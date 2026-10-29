//! Test-only feasibility primitive. No Runtime option or client API imports it.
use std::{
    ffi::c_void,
    fs, io,
    mem::{size_of, size_of_val},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HANDLE},
    Security::{
        Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW,
            SDDL_REVISION_1, SE_FILE_OBJECT,
        },
        FreeSid, GetSecurityDescriptorDacl, GetSecurityDescriptorSacl,
        Isolation::{CreateAppContainerProfile, DeleteAppContainerProfile},
        TokenAppContainerSid, TokenCapabilities, TokenIsAppContainer, TokenUser,
        DACL_SECURITY_INFORMATION, LABEL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES, TOKEN_APPCONTAINER_INFORMATION, TOKEN_USER,
    },
    Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
            JobObjectCpuRateControlInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_CPU_RATE_CONTROL_INFORMATION,
            JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_CPU_RATE_CONTROL_ENABLE, JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
            JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOB_OBJECT_LIMIT_PROCESS_MEMORY,
        },
        Registry::{
            RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, HKEY_CURRENT_USER, KEY_ALL_ACCESS,
            REG_CREATED_NEW_KEY, REG_OPTION_NON_VOLATILE,
        },
        Threading::{
            CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess, GetExitCodeProcess,
            GetProcessMitigationPolicy, InitializeProcThreadAttributeList,
            ProcessChildProcessPolicy, ResumeThread, TerminateProcess, UpdateProcThreadAttribute,
            WaitForSingleObject, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, DETACHED_PROCESS,
            EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION,
            PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY,
            PROC_THREAD_ATTRIBUTE_CHILD_PROCESS_POLICY, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            STARTF_USESTDHANDLES, STARTUPINFOEXW,
        },
        WindowsProgramming::{
            PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT,
            PROCESS_CREATION_CHILD_PROCESS_RESTRICTED,
        },
    },
};

#[path = "token.rs"]
mod token_evidence;
pub use token_evidence::{sid_text, token, token_lpac, token_u32, wide};
fn last() -> io::Error {
    io::Error::last_os_error()
}
fn stage(name: &str) -> io::Error {
    io::Error::other(format!("{name}: {}", last()))
}
fn local_app_data() -> io::Result<String> {
    // Turbo deliberately strips LOCALAPPDATA. Resolve the current native token's
    // known folder without inheriting more Host environment into child processes.
    unsafe {
        let mut path = ptr::null_mut();
        let status = windows_sys::Win32::UI::Shell::SHGetKnownFolderPath(
            &windows_sys::Win32::UI::Shell::FOLDERID_LocalAppData,
            0,
            ptr::null_mut(),
            &mut path,
        );
        if status < 0 || path.is_null() {
            windows_sys::Win32::System::Com::CoTaskMemFree(path.cast());
            return Err(io::Error::other(format!(
                "LocalAppData known folder: {status:#x}"
            )));
        }
        let mut length = 0;
        while length < 32768 && *path.add(length) != 0 {
            length += 1;
        }
        let result = if length == 32768 {
            Err(io::Error::other("known folder path budget"))
        } else {
            String::from_utf16(std::slice::from_raw_parts(path, length))
                .map_err(|_| io::Error::other("known folder UTF16"))
                .and_then(|text| {
                    if Path::new(&text).is_absolute() {
                        Ok(text)
                    } else {
                        Err(io::Error::other("known folder must be absolute"))
                    }
                })
        };
        windows_sys::Win32::System::Com::CoTaskMemFree(path.cast());
        result
    }
}
pub struct Handle(pub HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub fn user_sid() -> io::Result<String> {
    let data = token(unsafe { GetCurrentProcess() }, TokenUser)?;
    sid_text(unsafe { (*(data.as_ptr() as *const TOKEN_USER)).User.Sid })
}
pub struct Container {
    name: Vec<u16>,
    pub sid: *mut c_void,
    pub text: String,
}
// Fixed OS bootstrap capability, never accepted from an operation/package.
struct RegistryReadCapability(*mut c_void);
impl RegistryReadCapability {
    fn new() -> io::Result<Self> {
        unsafe {
            let (mut groups, mut caps) = (ptr::null_mut(), ptr::null_mut());
            let (mut ng, mut nc) = (0, 0);
            if windows_sys::Win32::Security::DeriveCapabilitySidsFromName(
                wide("registryRead").as_ptr(),
                &mut groups,
                &mut ng,
                &mut caps,
                &mut nc,
            ) == 0
            {
                return Err(last());
            }
            for index in 0..ng {
                LocalFree((*groups.add(index as usize)).cast());
            }
            LocalFree(groups.cast());
            if nc != 1 {
                for index in 0..nc {
                    LocalFree((*caps.add(index as usize)).cast());
                }
                LocalFree(caps.cast());
                return Err(io::Error::other("unexpected registry capability count"));
            }
            let sid = *caps;
            LocalFree(caps.cast());
            Ok(Self(sid))
        }
    }
}
impl Drop for RegistryReadCapability {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0.cast());
        }
    }
}
impl Container {
    pub fn new() -> io::Result<Self> {
        let name = wide(format!(
            "flowtools.validation.{}",
            uuid::Uuid::new_v4().simple()
        ));
        let mut sid = ptr::null_mut();
        let status = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                ptr::null(),
                0,
                &mut sid,
            )
        };
        if status < 0 {
            return Err(io::Error::other(format!(
                "CreateAppContainerProfile: {status:#x}"
            )));
        }
        let mut profile = Self {
            name,
            sid,
            text: String::new(),
        };
        profile.text = sid_text(sid)?;
        Ok(profile)
    }
}
impl Drop for Container {
    fn drop(&mut self) {
        let result = unsafe { DeleteAppContainerProfile(self.name.as_ptr()) };
        unsafe {
            FreeSid(self.sid);
        }
        assert!(
            result >= 0,
            "owned AppContainer cleanup failed: {result:#x}"
        );
    }
}

pub fn protect(path: &Path, sddl: &str, low: bool) -> io::Result<()> {
    unsafe {
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide(sddl).as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err(last());
        }
        let result = (|| {
            let (mut present, mut defaulted, mut dacl, mut sacl) =
                (0, 0, ptr::null_mut(), ptr::null_mut());
            if GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) == 0 {
                return Err(last());
            }
            let mut flags = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
            if low {
                if GetSecurityDescriptorSacl(descriptor, &mut present, &mut sacl, &mut defaulted)
                    == 0
                {
                    return Err(last());
                }
                flags |= LABEL_SECURITY_INFORMATION;
            }
            let error = SetNamedSecurityInfoW(
                wide(path).as_ptr(),
                SE_FILE_OBJECT,
                flags,
                ptr::null_mut(),
                ptr::null_mut(),
                dacl,
                sacl,
            );
            if error != 0 {
                Err(io::Error::from_raw_os_error(error as i32))
            } else {
                Ok(())
            }
        })();
        LocalFree(descriptor);
        result
    }
}
fn private_pipe(name: &str, user: &str) -> io::Result<Handle> {
    unsafe {
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide(format!("D:P(A;;GA;;;{user})")).as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err(last());
        }
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let raw = windows_sys::Win32::System::Pipes::CreateNamedPipeW(
            wide(name).as_ptr(),
            windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX
                | windows_sys::Win32::Storage::FileSystem::FILE_FLAG_FIRST_PIPE_INSTANCE,
            windows_sys::Win32::System::Pipes::PIPE_REJECT_REMOTE_CLIENTS,
            1,
            4096,
            4096,
            0,
            &security,
        );
        let error = last();
        LocalFree(descriptor);
        if raw == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            Err(error)
        } else {
            Ok(Handle(raw))
        }
    }
}
// Only a new, current-user-owned fixture key. The child opens HKEY_USERS with
// the actual user SID so AppContainer HKCU redirection cannot fake this test.
struct PrivateRegistry {
    name: Vec<u16>,
    read_path: String,
}
impl PrivateRegistry {
    fn new(user: &str) -> io::Result<Self> {
        let name = format!("Software\\FlowToolsIsolation-{}", uuid::Uuid::new_v4());
        let wide_name = wide(&name);
        unsafe {
            let mut descriptor = ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(format!("D:P(A;;KA;;;{user})")).as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            ) == 0
            {
                return Err(last());
            }
            let security = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            };
            let (mut key, mut disposition) = (ptr::null_mut(), 0);
            let status = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide_name.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_ALL_ACCESS,
                &security,
                &mut key,
                &mut disposition,
            );
            LocalFree(descriptor);
            if status != 0 {
                return Err(io::Error::from_raw_os_error(status as i32));
            }
            RegCloseKey(key);
            if disposition != REG_CREATED_NEW_KEY {
                return Err(io::Error::other("refuse pre-existing registry fixture"));
            }
        }
        Ok(Self {
            name: wide_name,
            read_path: format!("{user}\\{name}"),
        })
    }
}
impl Drop for PrivateRegistry {
    fn drop(&mut self) {
        let status = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, self.name.as_ptr()) };
        assert_eq!(status, 0, "remove only the owned registry fixture");
    }
}
pub struct Fixture {
    pub root: PathBuf,
    pub program: PathBuf,
    pub work: PathBuf,
    pub host: PathBuf,
    pub container: Container,
    registry: PrivateRegistry,
    _other_container: Container,
    private_file: fs::File,
    _pipe: Handle,
    pipe_name: String,
    _program_file: fs::File,
    _cleanup: Tree,
}
impl Fixture {
    pub fn new() -> io::Result<Self> {
        let current = std::env::current_exe()?;
        let compiled = current
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| io::Error::other("test binary directory"))?
            .join("flowtools-isolation-probe.exe");
        Self::from_program_bytes(&fs::read(compiled)?)
    }
    // Only this test module and the separate build-only media acceptance crate
    // use these bytes. No production Runtime/IPC option imports this harness.
    pub fn from_program_bytes(bytes: &[u8]) -> io::Result<Self> {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("node_modules/.tmp");
        fs::create_dir_all(&parent)?;
        let root = parent.join(format!("native-isolation-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root)?;
        let cleanup = Tree(root.clone());
        let user = user_sid()?;
        protect(&root, &format!("D:P(A;OICI;FA;;;{user})"), false)?;
        let container = Container::new()?;
        protect(
            &root,
            &format!("D:P(A;OICI;FA;;;{user})(A;;FX;;;{})", container.text),
            false,
        )?;
        let program_dir = root.join("program");
        let work = root.join("work");
        let host = root.join("host");
        for dir in [&program_dir, &work, &host] {
            fs::create_dir(dir)?;
        }
        protect(
            &program_dir,
            &format!("D:P(A;OICI;FA;;;{user})(A;OICI;FRFX;;;{})", container.text),
            false,
        )?;
        protect(
            &work,
            &format!(
                "D:P(A;OICI;FA;;;{user})(A;OICI;FA;;;{})S:(ML;OICI;NW;;;LW)",
                container.text
            ),
            true,
        )?;
        let program = program_dir.join("probe.exe");
        fs::write(&program, bytes)?;
        protect(
            &program,
            &format!("D:P(A;;FA;;;{user})(A;;FRFX;;;{})", container.text),
            false,
        )?;
        let _program_file = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&program)?;
        fs::write(host.join("private.txt"), b"HOST_PRIVATE_CANARY")?;
        let private_file = fs::File::open(host.join("private.txt"))?;
        if unsafe {
            windows_sys::Win32::Foundation::SetHandleInformation(private_file.as_raw_handle(), 1, 1)
        } == 0
        {
            return Err(last());
        }
        let registry = PrivateRegistry::new(&user)?;
        let pipe_name = format!("\\\\.\\pipe\\flowtools-isolation-{}", uuid::Uuid::new_v4());
        let pipe = private_pipe(&pipe_name, &user)?;
        let other_container = Container::new()?;
        let other = root.join("other-task");
        fs::create_dir(&other)?;
        protect(
            &other,
            &format!(
                "D:P(A;OICI;FA;;;{user})(A;OICI;FA;;;{})S:(ML;OICI;NW;;;LW)",
                other_container.text
            ),
            true,
        )?;
        fs::write(other.join("private.txt"), b"OTHER_TASK_CANARY")?;
        fs::copy(
            Path::new(
                &std::env::var_os("SystemRoot")
                    .ok_or_else(|| io::Error::other("missing SystemRoot"))?,
            )
            .join("System32/version.dll"),
            work.join("low-image.dll"),
        )?;
        protect(
            &work.join("low-image.dll"),
            &format!(
                "D:P(A;;FA;;;{user})(A;;FRFX;;;{})S:(ML;;NW;;;LW)",
                container.text
            ),
            true,
        )?;
        Ok(Self {
            root,
            program,
            work,
            host,
            container,
            registry,
            _other_container: other_container,
            private_file,
            _pipe: pipe,
            pipe_name,
            _program_file,
            _cleanup: cleanup,
        })
    }
    /// Uncontained is a test-only positive control, never a runtime selector.
    pub fn launch(
        &self,
        isolated: bool,
        case: &str,
        port: u16,
        memory: usize,
        args: &[&'static str],
    ) -> io::Result<Child> {
        self.launch_checked(isolated, case, port, memory, false, args)
    }
    /// A suspended, owned process whose policy check refuses before ResumeThread.
    pub fn refused_launch(&self) -> io::Result<Child> {
        self.launch_checked(true, "spin", 1, 128 * 1024 * 1024, true, &[])
    }
    fn launch_checked(
        &self,
        isolated: bool,
        case: &str,
        port: u16,
        memory: usize,
        refuse_before_resume: bool,
        args: &[&'static str],
    ) -> io::Result<Child> {
        unsafe {
            let job = Handle(CreateJobObjectW(ptr::null(), ptr::null()));
            if job.0.is_null() {
                return Err(last());
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_PROCESS_MEMORY
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            // Child-policy controls both allow a second process in the Job.
            limits.BasicLimitInformation.ActiveProcessLimit = if case == "child" { 2 } else { 1 };
            limits.ProcessMemoryLimit = memory;
            if SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            ) == 0
            {
                return Err(last());
            }
            let mut actual_limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            if QueryInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&mut actual_limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&actual_limits) as u32,
                ptr::null_mut(),
            ) == 0
            {
                return Err(stage("Job limit readback"));
            }
            if actual_limits.BasicLimitInformation.LimitFlags
                != limits.BasicLimitInformation.LimitFlags
                || actual_limits.BasicLimitInformation.ActiveProcessLimit
                    != limits.BasicLimitInformation.ActiveProcessLimit
                || actual_limits.ProcessMemoryLimit != limits.ProcessMemoryLimit
            {
                return Err(io::Error::other("unexpected actual Job limits"));
            }
            let cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
                ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE
                    | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
                Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 { CpuRate: 2500 },
            };
            if SetInformationJobObject(
                job.0,
                JobObjectCpuRateControlInformation,
                (&cpu as *const JOBOBJECT_CPU_RATE_CONTROL_INFORMATION).cast(),
                size_of_val(&cpu) as u32,
            ) == 0
            {
                return Err(stage("CPU hard cap"));
            }
            let mut actual_cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION::default();
            if QueryInformationJobObject(
                job.0,
                JobObjectCpuRateControlInformation,
                (&mut actual_cpu as *mut JOBOBJECT_CPU_RATE_CONTROL_INFORMATION).cast(),
                size_of_val(&actual_cpu) as u32,
                ptr::null_mut(),
            ) == 0
            {
                return Err(stage("CPU cap readback"));
            }
            if actual_cpu.ControlFlags != cpu.ControlFlags || actual_cpu.Anonymous.CpuRate != 2500 {
                return Err(io::Error::other("unexpected CPU limit"));
            }
            let attributes = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: ptr::null_mut(),
                bInheritHandle: 1,
            };
            let nul = Handle(CreateFileW(
                wide("NUL").as_ptr(),
                0x80000000 | 0x40000000,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &attributes,
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            ));
            if nul.0 == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                return Err(last());
            }
            let make_pipe = || -> io::Result<(Handle, Handle)> {
                let (mut read, mut write) = (ptr::null_mut(), ptr::null_mut());
                if windows_sys::Win32::System::Pipes::CreatePipe(
                    &mut read,
                    &mut write,
                    &attributes,
                    4096,
                ) == 0
                {
                    return Err(last());
                }
                let pair = (Handle(read), Handle(write));
                if windows_sys::Win32::Foundation::SetHandleInformation(pair.0 .0, 1, 0) == 0 {
                    return Err(last());
                }
                Ok(pair)
            };
            let (out_read, out_write) = make_pipe()?;
            let (err_read, err_write) = make_pipe()?;
            let mut handles = vec![nul.0, out_write.0, err_write.0];
            if !isolated {
                handles.push(self.private_file.as_raw_handle());
            }
            let registry = RegistryReadCapability::new()?;
            let mut capability = windows_sys::Win32::Security::SID_AND_ATTRIBUTES {
                Sid: registry.0,
                Attributes: 4,
            };
            let capabilities = SECURITY_CAPABILITIES {
                AppContainerSid: self.container.sid,
                Capabilities: &mut capability,
                CapabilityCount: 1,
                Reserved: 0,
            };
            let opt_out = PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT;
            let restricted = PROCESS_CREATION_CHILD_PROCESS_RESTRICTED;
            // Official Win32 process mitigation constants: no remote/Low image,
            // prefer System32. Kept local because windows-sys omits these macros.
            let mitigations: u64 = (1u64 << 52) | (1u64 << 56) | (1u64 << 60);
            let count = if isolated { 5 } else { 1 };
            let mut bytes = 0;
            InitializeProcThreadAttributeList(ptr::null_mut(), count, 0, &mut bytes);
            let mut buffer = vec![0usize; bytes.div_ceil(size_of::<usize>())];
            let list = buffer.as_mut_ptr().cast();
            if InitializeProcThreadAttributeList(list, count, 0, &mut bytes) == 0 {
                return Err(last());
            }
            struct Attributes(windows_sys::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST);
            impl Drop for Attributes {
                fn drop(&mut self) {
                    unsafe {
                        DeleteProcThreadAttributeList(self.0);
                    }
                }
            }
            let list = Attributes(list);
            let set = |key: u32, value: *const c_void, size: usize| -> io::Result<()> {
                if UpdateProcThreadAttribute(
                    list.0,
                    0,
                    key as usize,
                    value,
                    size,
                    ptr::null_mut(),
                    ptr::null(),
                ) == 0
                {
                    Err(stage(&format!("attribute {key}")))
                } else {
                    Ok(())
                }
            };
            set(
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                handles.as_ptr().cast(),
                size_of_val(handles.as_slice()),
            )?;
            if isolated {
                set(
                    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
                    (&capabilities as *const SECURITY_CAPABILITIES).cast(),
                    size_of_val(&capabilities),
                )?;
                set(
                    PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY,
                    (&opt_out as *const u32).cast(),
                    size_of_val(&opt_out),
                )?;
                set(
                    PROC_THREAD_ATTRIBUTE_CHILD_PROCESS_POLICY,
                    (&restricted as *const u32).cast(),
                    size_of_val(&restricted),
                )?;
                set(
                    PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY,
                    (&mitigations as *const u64).cast(),
                    size_of_val(&mitigations),
                )?;
            }
            let desktop = wide("WinSta0\\Default");
            let mut startup = STARTUPINFOEXW::default();
            startup.StartupInfo.lpDesktop = desktop.as_ptr().cast_mut();
            startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
            startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            startup.StartupInfo.hStdInput = nul.0;
            startup.StartupInfo.hStdOutput = out_write.0;
            startup.StartupInfo.hStdError = err_write.0;
            startup.lpAttributeList = list.0;
            let mut env = vec![
                format!(
                    "SystemRoot={}",
                    std::env::var("SystemRoot")
                        .map_err(|_| io::Error::other("missing SystemRoot"))?
                ),
                format!(
                    "SystemDrive={}",
                    std::env::var("SystemDrive")
                        .map_err(|_| io::Error::other("missing system drive"))?
                ),
                format!("TEMP={}", self.work.display()),
                format!("TMP={}", self.work.display()),
                format!("LOCALAPPDATA={}", local_app_data()?),
                format!("FLOWTOOLS_ISOLATION_ROOT={}", self.root.display()),
                format!("FLOWTOOLS_ISOLATION_CASE={case}"),
                format!("FLOWTOOLS_ISOLATION_SID={}", self.container.text),
                format!("FLOWTOOLS_ISOLATION_PORT={port}"),
                format!("FLOWTOOLS_ISOLATION_REGISTRY={}", self.registry.read_path),
                format!("FLOWTOOLS_ISOLATION_PIPE={}", self.pipe_name),
            ];
            env.sort_by_key(|v| v.to_ascii_uppercase());
            let mut environment: Vec<u16> = env.into_iter().flat_map(|v| wide(&v)).collect();
            environment.push(0);
            let mut command = wide(format!("\"{}\"", self.program.display()));
            // All arguments come from static Rust recipes in test code. Keep the
            // basic test quoting deliberately closed rather than accept paths or
            // arbitrary command lines from a runtime caller.
            command.pop();
            for arg in args {
                if arg.contains(['"', '\0', '\r', '\n']) || arg.ends_with('\\') {
                    return Err(io::Error::other("unsupported fixed test argument"));
                }
                command.extend(format!(" \"{arg}\"").encode_utf16());
            }
            command.push(0);
            let mut info = PROCESS_INFORMATION::default();
            if CreateProcessW(
                wide(&self.program).as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                CREATE_SUSPENDED
                    | DETACHED_PROCESS
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT,
                environment.as_ptr().cast(),
                wide(&self.work).as_ptr(),
                &startup.StartupInfo,
                &mut info,
            ) == 0
            {
                return Err(stage("CreateProcessW"));
            }
            let mut child = Child {
                process: Handle(info.hProcess),
                thread: Handle(info.hThread),
                job: Some(job),
                pub_pid: info.dwProcessId,
                started: Instant::now(),
                stdout: out_read,
                stderr: err_read,
                private_inherited: false,
                child_policy: 0,
                active_limit: actual_limits.BasicLimitInformation.ActiveProcessLimit,
            };
            let mut child_policy = windows_sys::Win32::System::SystemServices::PROCESS_MITIGATION_CHILD_PROCESS_POLICY::default();
            if GetProcessMitigationPolicy(child.process.0, ProcessChildProcessPolicy,
                (&mut child_policy as *mut windows_sys::Win32::System::SystemServices::PROCESS_MITIGATION_CHILD_PROCESS_POLICY).cast(),
                size_of_val(&child_policy)) == 0 {
                return Err(stage("child policy readback"));
            }
            child.child_policy = child_policy.Anonymous.Flags;
            if child.child_policy != u32::from(isolated) {
                return Err(io::Error::other("unexpected actual child creation policy"));
            }
            let mut duplicated = ptr::null_mut();
            if windows_sys::Win32::Foundation::DuplicateHandle(
                child.process.0,
                self.private_file.as_raw_handle(),
                GetCurrentProcess(),
                &mut duplicated,
                0,
                0,
                windows_sys::Win32::Foundation::DUPLICATE_SAME_ACCESS,
            ) != 0
            {
                let duplicated = Handle(duplicated);
                child.private_inherited = windows_sys::Win32::Foundation::CompareObjectHandles(
                    self.private_file.as_raw_handle(),
                    duplicated.0,
                ) != 0;
            } else if last().raw_os_error() != Some(6) {
                return Err(stage("inherited handle readback"));
            }
            if child.private_inherited == isolated {
                return Err(io::Error::other("unexpected inherited private object"));
            }
            if isolated {
                if token_u32(child.process.0, TokenIsAppContainer)? != 1
                    || !token_lpac(child.process.0)?
                    || token_u32(child.process.0, TokenCapabilities)? != 1
                {
                    return Err(io::Error::other("unexpected isolation token"));
                }
                let sid = token(child.process.0, TokenAppContainerSid)?;
                let actual = sid_text(
                    (*(sid.as_ptr() as *const TOKEN_APPCONTAINER_INFORMATION)).TokenAppContainer,
                )?;
                if actual != self.container.text {
                    return Err(io::Error::other("unexpected package SID"));
                }
                let caps = token(child.process.0, TokenCapabilities)?;
                if size_of_val(caps.as_slice())
                    < size_of::<windows_sys::Win32::Security::TOKEN_GROUPS>()
                {
                    return Err(io::Error::other("invalid capability group size"));
                }
                let groups = &*caps
                    .as_ptr()
                    .cast::<windows_sys::Win32::Security::TOKEN_GROUPS>();
                if groups.GroupCount != 1
                    || groups.Groups[0].Attributes & 4 == 0
                    || groups.Groups[0].Attributes & 16 != 0
                    || sid_text(groups.Groups[0].Sid)? != sid_text(registry.0)?
                {
                    return Err(io::Error::other("unexpected enabled capability identity"));
                }
            }
            if AssignProcessToJobObject(child.job.as_ref().unwrap().0, child.process.0) == 0 {
                return Err(stage("AssignProcessToJobObject"));
            }
            if refuse_before_resume {
                child.terminate()?;
                child.wait(Duration::from_millis(500))?;
                child.wait_empty(Duration::from_millis(500))?;
                return Ok(child);
            }
            if ResumeThread(child.thread.0) == u32::MAX {
                return Err(stage("ResumeThread"));
            }
            Ok(child)
        }
    }
}
struct Tree(PathBuf);
impl Drop for Tree {
    fn drop(&mut self) {
        assert_eq!(
            self.0.parent().unwrap(),
            Path::new(env!("CARGO_MANIFEST_DIR")).join("node_modules/.tmp")
        );
        assert!(self
            .0
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("native-isolation-")
            .is_some_and(|v| uuid::Uuid::parse_str(v).is_ok()));
        fs::remove_dir_all(&self.0).expect("remove only the owned fixture tree");
    }
}
pub struct Child {
    process: Handle,
    thread: Handle,
    pub job: Option<Handle>,
    pub pub_pid: u32,
    pub started: Instant,
    pub private_inherited: bool,
    pub child_policy: u32,
    pub active_limit: u32,
    stdout: Handle,
    stderr: Handle,
}
impl Child {
    pub fn output(&self, bound: Duration, limit: usize) -> io::Result<(u32, Vec<u8>, Vec<u8>)> {
        let end = Instant::now() + bound;
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut total = 0usize;
        loop {
            for (handle, target) in [(&self.stdout, &mut out), (&self.stderr, &mut err)] {
                let mut available = 0;
                if unsafe {
                    windows_sys::Win32::System::Pipes::PeekNamedPipe(
                        handle.0,
                        ptr::null_mut(),
                        0,
                        ptr::null_mut(),
                        &mut available,
                        ptr::null_mut(),
                    )
                } == 0
                {
                    if last().raw_os_error() == Some(109) {
                        continue;
                    }
                    return Err(last());
                }
                if available > 0 {
                    let mut bytes = [0u8; 8192];
                    let mut read = 0;
                    if unsafe {
                        windows_sys::Win32::Storage::FileSystem::ReadFile(
                            handle.0,
                            bytes.as_mut_ptr(),
                            available.min(bytes.len() as u32),
                            &mut read,
                            ptr::null_mut(),
                        )
                    } == 0
                    {
                        return Err(last());
                    }
                    if total + read as usize > limit {
                        self.terminate()?;
                        self.wait(Duration::from_millis(500))?;
                        self.wait_empty(Duration::from_millis(500))?;
                        return Err(io::Error::other("output budget exceeded"));
                    }
                    total += read as usize;
                    target.extend_from_slice(&bytes[..read as usize]);
                }
            }
            if self.exited()? {
                // Read all already-buffered output before reporting exit.
                let mut remaining = 0;
                for handle in [&self.stdout, &self.stderr] {
                    let mut n = 0;
                    let ok = unsafe {
                        windows_sys::Win32::System::Pipes::PeekNamedPipe(
                            handle.0,
                            ptr::null_mut(),
                            0,
                            ptr::null_mut(),
                            &mut n,
                            ptr::null_mut(),
                        )
                    };
                    if ok == 0 && last().raw_os_error() != Some(109) {
                        return Err(last());
                    }
                    remaining += n;
                }
                if remaining == 0 {
                    return Ok((self.wait(Duration::from_millis(500))?, out, err));
                }
            }
            if Instant::now() >= end {
                self.terminate()?;
                self.wait(Duration::from_millis(500))?;
                return Err(io::Error::other("output deadline exceeded"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    pub fn exited(&self) -> io::Result<bool> {
        let r = unsafe { WaitForSingleObject(self.process.0, 0) };
        if r == 0 {
            Ok(true)
        } else if r == 258 {
            Ok(false)
        } else {
            Err(last())
        }
    }
    pub fn wait(&self, bound: Duration) -> io::Result<u32> {
        let result = unsafe {
            WaitForSingleObject(
                self.process.0,
                bound.as_millis().try_into().unwrap_or(u32::MAX - 1),
            )
        };
        if result != 0 {
            return Err(io::Error::other(format!("wait did not exit: {result}")));
        }
        let mut code = 0;
        if unsafe { GetExitCodeProcess(self.process.0, &mut code) } == 0 {
            return Err(last());
        }
        Ok(code)
    }
    pub fn active(&self) -> io::Result<u32> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        if unsafe {
            QueryInformationJobObject(
                self.job.as_ref().unwrap().0,
                JobObjectBasicAccountingInformation,
                (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of_val(&info) as u32,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(last());
        }
        Ok(info.ActiveProcesses)
    }
    pub fn wait_empty(&self, bound: Duration) -> io::Result<()> {
        let end = Instant::now() + bound;
        loop {
            if self.active()? == 0 {
                return Ok(());
            }
            if Instant::now() >= end {
                return Err(io::Error::other("job accounting did not drain"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    pub fn terminate(&self) -> io::Result<()> {
        if unsafe { TerminateJobObject(self.job.as_ref().unwrap().0, 71) } == 0 {
            Err(last())
        } else {
            Ok(())
        }
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        unsafe {
            if let Some(job) = self.job.take() {
                TerminateJobObject(job.0, 72);
                drop(job);
            }
            if !self.exited().unwrap_or(false) {
                TerminateProcess(self.process.0, 72);
                WaitForSingleObject(self.process.0, 5000);
            }
        }
    }
}
