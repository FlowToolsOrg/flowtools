use crate::{INPUT_LIMIT, OUTPUT_LIMIT, TERMINATE_GRACE_MS, WALL_LIMIT_MS};
use serde_json::Value;
use std::{
    cell::Cell,
    ffi::c_void,
    fs::{self, File},
    io::{self, Read, Write},
    mem::{size_of, zeroed},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Mutex, MutexGuard,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, LocalFree, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    },
    Security::{
        AccessCheck,
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        },
        DuplicateToken, EqualSid, FreeSid, GetTokenInformation,
        Isolation::{
            CreateAppContainerProfile, DeleteAppContainerProfile, GetAppContainerFolderPath,
        },
        SecurityImpersonation, SetFileSecurityW, TokenAppContainerSid, TokenCapabilities,
        TokenIsAppContainer, TokenUser, DACL_SECURITY_INFORMATION, GENERIC_MAPPING, PRIVILEGE_SET,
        PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES, TOKEN_APPCONTAINER_INFORMATION,
        TOKEN_DUPLICATE, TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TOKEN_USER,
    },
    System::{
        Com::CoTaskMemFree,
        Diagnostics::Debug::{
            GetErrorMode, SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX,
            SEM_NOOPENFILEERRORBOX,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
            JobObjectCpuRateControlInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_CPU_RATE_CONTROL_INFORMATION,
            JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_CPU_RATE_CONTROL_ENABLE, JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
            JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOB_OBJECT_LIMIT_PROCESS_TIME,
        },
        Pipes::CreatePipe,
        SystemInformation::GetWindowsDirectoryW,
        Threading::{
            CreateEventW, CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess,
            GetExitCodeProcess, InitializeProcThreadAttributeList, OpenProcessToken, ResumeThread,
            TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject, CREATE_SUSPENDED,
            CREATE_UNICODE_ENVIRONMENT, DETACHED_PROCESS, EXTENDED_STARTUPINFO_PRESENT,
            PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY,
            PROC_THREAD_ATTRIBUTE_CHILD_PROCESS_POLICY, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            STARTF_USESTDHANDLES, STARTUPINFOEXW,
        },
        WindowsProgramming::{
            PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT,
            PROCESS_CREATION_CHILD_PROCESS_RESTRICTED,
        },
        IO::CancelSynchronousIo,
    },
};

fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

extern "C" {
    fn flowtools_p22_mitigation_policy() -> u64;
}

// Child processes inherit process error mode before their first instruction.
// Scope the parent setting to this harness's serialized CreateProcess call and
// restore it immediately; never edit machine/user registry or another process.
struct ErrorUiGuard {
    previous_mode: u32,
    restored: bool,
    _lock: MutexGuard<'static, ()>,
}
impl ErrorUiGuard {
    fn new() -> io::Result<Self> {
        static LOCK: Mutex<()> = Mutex::new(());
        let lock = LOCK
            .lock()
            .map_err(|_| io::Error::other("Error UI mode mutex poisoned"))?;
        let previous_mode = unsafe { GetErrorMode() };
        unsafe {
            SetErrorMode(
                previous_mode
                    | SEM_FAILCRITICALERRORS
                    | SEM_NOGPFAULTERRORBOX
                    | SEM_NOOPENFILEERRORBOX,
            );
        }
        let guard = Self {
            previous_mode,
            restored: false,
            _lock: lock,
        };
        Ok(guard)
    }
    fn restore(mut self) -> io::Result<()> {
        unsafe {
            SetErrorMode(self.previous_mode);
        }
        if unsafe { GetErrorMode() } != self.previous_mode {
            return Err(io::Error::other(
                "Parent error UI policy restoration refused",
            ));
        }
        self.restored = true;
        Ok(())
    }
}
impl Drop for ErrorUiGuard {
    fn drop(&mut self) {
        if !self.restored {
            unsafe {
                SetErrorMode(self.previous_mode);
            }
        }
    }
}

struct Handle(HANDLE);
impl Handle {
    fn new(value: HANDLE) -> io::Result<Self> {
        if value.is_null() || value as isize == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(value))
        }
    }
    fn into_file(self) -> File {
        let raw = self.0;
        std::mem::forget(self);
        unsafe { File::from_raw_handle(raw) }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct Sid(PSID);
impl Drop for Sid {
    fn drop(&mut self) {
        unsafe {
            FreeSid(self.0);
        }
    }
}

struct OwnedProfile {
    name: Vec<u16>,
    folder: PathBuf,
    removed: bool,
    preserve: bool,
}
impl OwnedProfile {
    fn create(name: &str) -> io::Result<(Self, Sid)> {
        let name = wide(name);
        let mut sid = null_mut();
        let result = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                null(),
                0,
                &mut sid,
            )
        };
        // ERROR_ALREADY_EXISTS is refusal, never derive or adopt an old profile.
        if result < 0 {
            return Err(io::Error::from_raw_os_error(result));
        }
        let sid = Sid(sid);
        let mut profile = Self {
            name,
            folder: PathBuf::new(),
            removed: false,
            preserve: false,
        };
        let mut folder = null_mut();
        let result =
            unsafe { GetAppContainerFolderPath(wide(sid_string(sid.0)?).as_ptr(), &mut folder) };
        if result < 0 {
            return Err(io::Error::from_raw_os_error(result));
        }
        unsafe {
            let mut len = 0;
            while *folder.add(len) != 0 {
                len += 1;
            }
            profile.folder = PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(
                folder, len,
            )));
            CoTaskMemFree(folder.cast());
        }
        Ok((profile, sid))
    }
    fn close(&mut self) -> io::Result<()> {
        if self.removed {
            return Ok(());
        }
        let result = unsafe { DeleteAppContainerProfile(self.name.as_ptr()) };
        if result < 0 {
            return Err(io::Error::from_raw_os_error(result));
        }
        self.removed = true;
        Ok(())
    }
}
impl Drop for OwnedProfile {
    fn drop(&mut self) {
        if !self.preserve {
            let _ = self.close();
        }
    }
}

struct AttributeList(Vec<usize>);
impl AttributeList {
    fn new(count: u32) -> io::Result<Self> {
        let mut bytes = 0;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), count, 0, &mut bytes);
        }
        if bytes == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut list = Self(vec![0; bytes.div_ceil(size_of::<usize>())]);
        if unsafe { InitializeProcThreadAttributeList(list.ptr(), count, 0, &mut bytes) } == 0 {
            // No initialized attribute list exists to delete on this path;
            // its allocation must still be freed.
            let error = io::Error::last_os_error();
            let _allocation = std::mem::take(&mut list.0);
            std::mem::forget(list);
            return Err(error);
        }
        Ok(list)
    }
    fn ptr(&mut self) -> *mut c_void {
        self.0.as_mut_ptr().cast()
    }
    fn set<T>(&mut self, attribute: u32, data: &T) -> io::Result<()> {
        if unsafe {
            UpdateProcThreadAttribute(
                self.ptr(),
                0,
                attribute as usize,
                (data as *const T).cast(),
                size_of::<T>(),
                null_mut(),
                null(),
            )
        } == 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
impl Drop for AttributeList {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.ptr());
        }
    }
}

fn token_buffer(token: HANDLE, class: TOKEN_INFORMATION_CLASS) -> io::Result<Vec<usize>> {
    let mut size = 0;
    unsafe {
        GetTokenInformation(token, class, null_mut(), 0, &mut size);
    }
    if size == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut data = vec![0_usize; (size as usize).div_ceil(size_of::<usize>())];
    if unsafe { GetTokenInformation(token, class, data.as_mut_ptr().cast(), size, &mut size) } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(data)
}

fn token(process: HANDLE) -> io::Result<Handle> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY | TOKEN_DUPLICATE, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Handle::new(token)
}

fn sid_string(sid: PSID) -> io::Result<String> {
    let mut value = null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut value) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut size = 0;
    unsafe {
        while *value.add(size) != 0 {
            size += 1;
        }
        let result = String::from_utf16_lossy(std::slice::from_raw_parts(value, size));
        LocalFree(value.cast());
        Ok(result)
    }
}

fn user_sid() -> io::Result<String> {
    let token = token(unsafe { GetCurrentProcess() })?;
    let buffer = token_buffer(token.0, TokenUser)?;
    sid_string(unsafe { (*(buffer.as_ptr().cast::<TOKEN_USER>())).User.Sid })
}

fn set_acl(path: &Path, sddl: &str) -> io::Result<()> {
    let sddl = wide(sddl);
    let mut descriptor = null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let ok =
        unsafe { SetFileSecurityW(wide(path).as_ptr(), DACL_SECURITY_INFORMATION, descriptor) };
    unsafe {
        LocalFree(descriptor);
    }
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn aap_access(token: HANDLE) -> io::Result<bool> {
    let mut impersonation = null_mut();
    if unsafe { DuplicateToken(token, SecurityImpersonation, &mut impersonation) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let impersonation = Handle::new(impersonation)?;
    let mut descriptor = null_mut();
    // User/Everyone permits the ordinary user pass; only AAP permits the
    // package pass. Test an ordinary AppContainer positive control separately.
    let sddl = wide("O:SYG:SYD:(A;;0x1;;;WD)(A;;0x1;;;S-1-15-2-1)");
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mapping = GENERIC_MAPPING {
        GenericRead: 1,
        GenericWrite: 2,
        GenericExecute: 4,
        GenericAll: 7,
    };
    let mut privileges = [0_usize; 128];
    let mut size = size_of_val(&privileges) as u32;
    let mut granted = 0;
    let mut allowed = 0;
    let success = unsafe {
        AccessCheck(
            descriptor,
            impersonation.0,
            1,
            &mapping,
            privileges.as_mut_ptr().cast::<PRIVILEGE_SET>(),
            &mut size,
            &mut granted,
            &mut allowed,
        )
    };
    let error = io::Error::last_os_error();
    unsafe {
        LocalFree(descriptor);
    }
    if success == 0 {
        return Err(error);
    }
    Ok(allowed != 0 && granted & 1 != 0)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenEvidence {
    pub app_container: bool,
    pub aap_access_denied: bool,
    pub capability_count: u32,
    pub sid_matches: bool,
}

fn verify_token(
    process: HANDLE,
    expected_sid: PSID,
    expect_aap_denial: bool,
) -> io::Result<TokenEvidence> {
    let token = token(process)?;
    let is_app = token_buffer(token.0, TokenIsAppContainer)
        .map_err(|e| io::Error::other(format!("TokenIsAppContainer: {e}")))?;
    let capabilities = token_buffer(token.0, TokenCapabilities)
        .map_err(|e| io::Error::other(format!("TokenCapabilities: {e}")))?;
    let sid = token_buffer(token.0, TokenAppContainerSid)
        .map_err(|e| io::Error::other(format!("TokenAppContainerSid: {e}")))?;
    let evidence = unsafe {
        TokenEvidence {
            app_container: *(is_app.as_ptr().cast::<u32>()) == 1,
            aap_access_denied: !aap_access(token.0)?,
            capability_count: *(capabilities.as_ptr().cast::<u32>()),
            sid_matches: EqualSid(
                (*(sid.as_ptr().cast::<TOKEN_APPCONTAINER_INFORMATION>())).TokenAppContainer,
                expected_sid,
            ) != 0,
        }
    };
    if !evidence.app_container
        || evidence.aap_access_denied != expect_aap_denial
        || evidence.capability_count != 0
        || !evidence.sid_matches
    {
        return Err(io::Error::other(format!(
            "LPAC token refusal: {evidence:?}"
        )));
    }
    Ok(evidence)
}

fn pipe() -> io::Result<(Handle, Handle)> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    let (mut read, mut write) = (null_mut(), null_mut());
    if unsafe { CreatePipe(&mut read, &mut write, &attributes, 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((Handle::new(read)?, Handle::new(write)?))
}

fn no_inherit(handle: &Handle) -> io::Result<()> {
    if unsafe { SetHandleInformation(handle.0, HANDLE_FLAG_INHERIT, 0) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

struct Job(Handle);
impl Job {
    fn new() -> io::Result<Self> {
        let job = Self(Handle::new(unsafe { CreateJobObjectW(null(), null()) })?);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_PROCESS_TIME;
        limits.BasicLimitInformation.ActiveProcessLimit = 1;
        limits.BasicLimitInformation.PerProcessUserTimeLimit = 10_000_000; // 1 CPU second.
        limits.ProcessMemoryLimit = 128 * 1024 * 1024;
        if unsafe {
            SetInformationJobObject(
                job.0 .0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
            ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
            Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 { CpuRate: 2000 },
        };
        if unsafe {
            SetInformationJobObject(
                job.0 .0,
                JobObjectCpuRateControlInformation,
                (&cpu as *const JOBOBJECT_CPU_RATE_CONTROL_INFORMATION).cast(),
                size_of_val(&cpu) as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }
    fn active(&self) -> io::Result<u32> {
        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
        if unsafe {
            QueryInformationJobObject(
                self.0 .0,
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of_val(&accounting) as u32,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(accounting.ActiveProcesses)
    }
    fn terminate(&self) -> io::Result<()> {
        if unsafe { TerminateJobObject(self.0 .0, 1) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    Deadline,
    OutputLimit,
    ProcessFailure,
    InvalidResult,
}

#[derive(Debug)]
pub struct Outcome {
    pub token: TokenEvidence,
    pub value: Option<Value>,
    pub rejection: Option<Rejection>,
    pub exit_code: u32,
    pub elapsed: Duration,
    pub active_processes: u32,
}

struct Drained {
    output: Vec<u8>,
    writer: io::Result<()>,
    exit_code: u32,
}

// Own every post-create path, including suspended setup refusal and unwinding.
// No caller can resume a process after this guard refuses setup.
struct Custody<'fixture> {
    uncertain: &'fixture Cell<bool>,
    child: Handle,
    job: Job,
    assigned: bool,
    cleanup_attempted: bool,
    verified_finished: bool,
    writer: Option<thread::JoinHandle<io::Result<()>>>,
    reader: Option<thread::JoinHandle<io::Result<Vec<u8>>>>,
}
impl Custody<'_> {
    fn finish(&mut self) -> io::Result<Drained> {
        self.cleanup_attempted = true;
        let end = Instant::now() + Duration::from_millis(TERMINATE_GRACE_MS);
        let process_result = (|| -> io::Result<u32> {
            let status = unsafe { WaitForSingleObject(self.child.0, 0) };
            if status != WAIT_OBJECT_0 {
                if self.assigned {
                    self.job.terminate()?;
                } else if unsafe { TerminateProcess(self.child.0, 1) } == 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            let remaining = end.saturating_duration_since(Instant::now()).as_millis() as u32;
            if unsafe { WaitForSingleObject(self.child.0, remaining) } != WAIT_OBJECT_0 {
                return Err(io::Error::other("LPAC child termination is uncertain"));
            }
            while self.job.active()? != 0 {
                if Instant::now() >= end {
                    return Err(io::Error::other("LPAC Job drain is uncertain"));
                }
                thread::sleep(Duration::from_millis(1));
            }
            let mut exit_code = 0;
            if unsafe { GetExitCodeProcess(self.child.0, &mut exit_code) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(exit_code)
        })();
        if process_result.is_err() {
            // Bounded best effort only. Never claim drain after an OS failure.
            // Cancel synchronous pipe workers without transferring/closing their
            // thread handles, then retain uncertain fixture state for diagnosis.
            if let Some(reader) = &self.reader {
                unsafe {
                    CancelSynchronousIo(reader.as_raw_handle());
                }
            }
            if let Some(writer) = &self.writer {
                unsafe {
                    CancelSynchronousIo(writer.as_raw_handle());
                }
            }
        }
        while !self
            .reader
            .as_ref()
            .is_none_or(thread::JoinHandle::is_finished)
            || !self
                .writer
                .as_ref()
                .is_none_or(thread::JoinHandle::is_finished)
        {
            if Instant::now() >= end {
                return Err(io::Error::other("LPAC pipe worker cleanup is uncertain"));
            }
            thread::sleep(Duration::from_millis(1));
        }
        let output_result = self.reader.take().map(|reader| {
            reader
                .join()
                .map_err(|_| io::Error::other("Output reader panicked"))
        });
        let writer_result = self.writer.take().map(|writer| {
            writer
                .join()
                .map_err(|_| io::Error::other("Input writer panicked"))
        });
        let exit_code = process_result?;
        let output = match output_result {
            Some(result) => result??,
            None => Vec::new(),
        };
        let writer = match writer_result {
            Some(result) => result?,
            None => Ok(()),
        };
        self.verified_finished = true;
        Ok(Drained {
            output,
            writer,
            exit_code,
        })
    }
}
impl Drop for Custody<'_> {
    fn drop(&mut self) {
        if !self.cleanup_attempted && !self.verified_finished {
            let _ = self.finish();
        }
        // This also covers early return/unwinding and a prior failed finish.
        // Never let Fixture::drop delete evidence or its OS profile unless
        // process exit, empty Job and pipe-worker joins were all verified.
        if !self.verified_finished {
            self.uncertain.set(true);
        }
    }
}

/// Disposable fixture state only. Not an installed plugin or execution grant.
pub struct Fixture {
    root: PathBuf,
    executable: PathBuf,
    canary: PathBuf,
    aap_canary: PathBuf,
    sid: Sid,
    profile: OwnedProfile,
    uncertain: Cell<bool>,
}
impl Fixture {
    pub fn new() -> io::Result<Self> {
        Self::with_binary(Path::new(env!("FLOWTOOLS_P22_CHILD_BINARY")))
    }
    fn with_binary(binary: &Path) -> io::Result<Self> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let unique = format!(
            "{}.{}.{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let root = std::env::temp_dir().join(format!("flowtools-p22-{unique}"));
        // create_dir is create-only; never adopt an existing test directory.
        fs::create_dir(&root)?;
        let (profile, sid) = match OwnedProfile::create(&format!("FlowTools.P22.{unique}")) {
            Ok(value) => value,
            Err(error) => {
                let _ = fs::remove_dir(&root);
                return Err(error);
            }
        };
        let fixture = Self {
            executable: root.join("runner").join("flowtools-p22-sandbox-child.exe"),
            canary: root.join("private").join("canary.txt"),
            aap_canary: root.join("aap-canary.txt"),
            root,
            sid,
            profile,
            uncertain: Cell::new(false),
        };
        let owner = user_sid()?;
        set_acl(
            &fixture.root,
            &format!(
                "D:P(A;OICI;FA;;;{owner})(A;;GX;;;{})",
                sid_string(fixture.sid.0)?
            ),
        )?;
        fs::create_dir(fixture.root.join("runner"))?;
        fs::create_dir(fixture.root.join("private"))?;
        fs::write(&fixture.canary, b"T0_PRIVATE_CANARY")?;
        fs::write(&fixture.aap_canary, b"AAP_ONLY_CANARY")?;
        set_acl(
            &fixture.aap_canary,
            &format!("D:P(A;;FA;;;{owner})(A;;GR;;;S-1-15-2-1)"),
        )?;
        fs::copy(binary, &fixture.executable)?;
        set_acl(
            &fixture.root.join("runner"),
            &format!(
                "D:P(A;OICI;FA;;;{owner})(A;OICI;GRGX;;;{})",
                sid_string(fixture.sid.0)?
            ),
        )?;
        // Existing file gets an explicit entry; directory inheritance alone is
        // not used as proof that Windows actually granted image read/execute.
        set_acl(
            &fixture.executable,
            &format!(
                "D:P(A;;FA;;;{owner})(A;;GRGX;;;{})",
                sid_string(fixture.sid.0)?
            ),
        )?;
        Ok(fixture)
    }
    pub fn canary_path(&self) -> &Path {
        &self.canary
    }
    pub fn aap_canary_path(&self) -> &Path {
        &self.aap_canary
    }
    pub fn root_path(&self) -> &Path {
        &self.root
    }
    pub fn executable_path(&self) -> &Path {
        &self.executable
    }
    pub fn os_profile_path(&self) -> &Path {
        &self.profile.folder
    }
    /// Explicit fixture teardown is checked in the native acceptance test.
    pub fn close(mut self) -> io::Result<()> {
        if self.uncertain.get() {
            return Err(io::Error::other(
                "Fixture cleanup is uncertain; owned artifacts retained",
            ));
        }
        fs::remove_dir_all(&self.root)?;
        self.profile.close()?;
        let mut folder = null_mut();
        let result = unsafe {
            GetAppContainerFolderPath(wide(sid_string(self.sid.0)?).as_ptr(), &mut folder)
        };
        if result >= 0 {
            unsafe {
                CoTaskMemFree(folder.cast());
            }
            return Err(io::Error::other(
                "Deleted OS AppContainer profile still resolves",
            ));
        }
        if self.profile.folder.exists() {
            return Err(io::Error::other(
                "Deleted OS AppContainer folder still exists",
            ));
        }
        Ok(())
    }
    pub fn execute(&self, source: &str, input: Value) -> io::Result<Outcome> {
        self.run("--js", serde_json::json!({"source":source,"input":input}))
    }
    pub fn native_probe(&self, request: Value) -> io::Result<Outcome> {
        self.run("--native-probe", request)
    }
    /// Deliberately inheritable private handle, omitted from HANDLE_LIST.
    /// This is a fixture-only attack probe, never a plugin-facing handle API.
    pub fn probe_unlisted_handle(&self) -> io::Result<Outcome> {
        self.probe_handle(false)
    }
    /// Positive control for exactly the same Event object and SetEvent probe.
    /// Only this fixture path deliberately adds an Event to HANDLE_LIST.
    pub fn probe_included_handle(&self) -> io::Result<Outcome> {
        self.probe_handle(true)
    }
    fn probe_handle(&self, include: bool) -> io::Result<Outcome> {
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        };
        let canary = Handle::new(unsafe { CreateEventW(&attributes, 1, 0, null()) })?;
        let mut outcome = self.run_inner_with_handle(
            "--native-probe",
            serde_json::json!({"operation":"handle","handle":canary.0 as usize}),
            true,
            self.sid.0,
            include.then_some(canary.0),
        )?;
        let status = unsafe { WaitForSingleObject(canary.0, 0) };
        if status != WAIT_TIMEOUT && status != WAIT_OBJECT_0 {
            return Err(io::Error::last_os_error());
        }
        outcome.value = Some(
            serde_json::json!({"probe":outcome.value,"parentCanarySignaled":status == WAIT_OBJECT_0}),
        );
        Ok(outcome)
    }
    fn run(&self, mode: &str, request: Value) -> io::Result<Outcome> {
        self.run_inner(mode, request, true, self.sid.0)
    }
    fn run_inner(
        &self,
        mode: &str,
        request: Value,
        deny_aap: bool,
        expected_sid: PSID,
    ) -> io::Result<Outcome> {
        self.run_inner_with_handle(mode, request, deny_aap, expected_sid, None)
    }
    fn run_inner_with_handle(
        &self,
        mode: &str,
        request: Value,
        deny_aap: bool,
        expected_sid: PSID,
        extra_handle: Option<HANDLE>,
    ) -> io::Result<Outcome> {
        if self.uncertain.get() {
            return Err(io::Error::other(
                "Fixture cleanup is uncertain; execution refused",
            ));
        }
        let input = serde_json::to_vec(&request)?;
        if input.len() > INPUT_LIMIT {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "INPUT_LIMIT"));
        }
        let (stdin_read, stdin_write) = pipe()?;
        let (stdout_read, stdout_write) = pipe()?;
        no_inherit(&stdin_write)?;
        no_inherit(&stdout_read)?;
        let mut attributes = AttributeList::new(5)?;
        let capabilities = SECURITY_CAPABILITIES {
            AppContainerSid: self.sid.0,
            Capabilities: null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        };
        let lpac = if deny_aap {
            PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT
        } else {
            0
        };
        let child_policy = PROCESS_CREATION_CHILD_PROCESS_RESTRICTED;
        let handles = [stdin_read.0, stdout_write.0];
        let control_handles = [
            stdin_read.0,
            stdout_write.0,
            extra_handle.unwrap_or(null_mut()),
        ];
        attributes.set(PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, &capabilities)?;
        attributes.set(PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY, &lpac)?;
        attributes.set(PROC_THREAD_ATTRIBUTE_CHILD_PROCESS_POLICY, &child_policy)?;
        if extra_handle.is_some() {
            attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &control_handles)?;
        } else {
            attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &handles)?;
        }
        let mitigations = unsafe { flowtools_p22_mitigation_policy() };
        attributes.set(PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY, &mitigations)?;
        let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = stdin_read.0;
        startup.StartupInfo.hStdOutput = stdout_write.0;
        startup.StartupInfo.hStdError = stdout_write.0;
        startup.lpAttributeList = attributes.ptr();
        let mut command = wide(format!("\"{}\" {mode}", self.executable.display()));
        let mut windows_directory = [0_u16; 32768];
        let directory_len = unsafe {
            GetWindowsDirectoryW(
                windows_directory.as_mut_ptr(),
                windows_directory.len() as u32,
            )
        } as usize;
        if directory_len == 0 || directory_len >= windows_directory.len() {
            return Err(io::Error::last_os_error());
        }
        // AppContainer initialization requires SystemRoot. Derive it from the
        // OS API, never inherit even this value from the parent environment.
        let local_data = self
            .profile
            .folder
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or_else(|| io::Error::other("OS AppContainer folder structure refused"))?;
        let mut environment = wide(format!(
            "LOCALAPPDATA={}\0SystemRoot={}",
            local_data.display(),
            String::from_utf16_lossy(&windows_directory[..directory_len])
        ));
        environment.push(0);
        let mut process: PROCESS_INFORMATION = unsafe { zeroed() };
        let job = Job::new()?;
        let error_ui = ErrorUiGuard::new()?;
        let created = {
            if unsafe {
                CreateProcessW(
                    wide(&self.executable).as_ptr(),
                    command.as_mut_ptr(),
                    null(),
                    null(),
                    1,
                    CREATE_SUSPENDED
                        | DETACHED_PROCESS
                        | CREATE_UNICODE_ENVIRONMENT
                        | EXTENDED_STARTUPINFO_PRESENT,
                    environment.as_ptr().cast(),
                    wide(self.executable.parent().unwrap()).as_ptr(),
                    &startup.StartupInfo,
                    &mut process,
                )
            } == 0
            {
                // Capture GetLastError before any policy restoration API call.
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        };
        if let Err(error) = created {
            error_ui.restore()?;
            return Err(io::Error::other(format!(
                "CreateProcessW LPAC refused: {error}"
            )));
        }
        let child_thread = Handle(process.hThread);
        let mut custody = Custody {
            uncertain: &self.uncertain,
            child: Handle(process.hProcess),
            job,
            assigned: false,
            cleanup_attempted: false,
            verified_finished: false,
            writer: None,
            reader: None,
        };
        // Child ownership exists before a restoration error can leave this
        // function. Custody then terminates the still-suspended child.
        error_ui.restore()?;
        // Close parent copies before any post-create failure can enter cleanup.
        drop(stdin_read);
        drop(stdout_write);
        let mut input_file = stdin_write.into_file();
        let mut output_file = stdout_read.into_file();
        let (limit_send, limit_receive) = mpsc::channel();
        let started = Instant::now();
        let execution = (|| -> io::Result<(TokenEvidence, Option<Rejection>)> {
            if unsafe { AssignProcessToJobObject(custody.job.0 .0, custody.child.0) } == 0 {
                return Err(io::Error::last_os_error());
            }
            custody.assigned = true;
            if custody.job.active()? != 1 {
                return Err(io::Error::other("LPAC child Job membership refusal"));
            }
            let evidence = verify_token(custody.child.0, expected_sid, deny_aap)?;
            custody.writer = Some(thread::spawn(move || input_file.write_all(&input)));
            custody.reader = Some(thread::spawn(move || -> io::Result<Vec<u8>> {
                let mut output = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let size = output_file.read(&mut buffer)?;
                    if size == 0 {
                        return Ok(output);
                    }
                    if output.len() + size > OUTPUT_LIMIT {
                        let _ = limit_send.send(());
                        return Ok(output);
                    }
                    output.extend_from_slice(&buffer[..size]);
                }
            }));
            if unsafe { ResumeThread(child_thread.0) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            loop {
                if limit_receive.try_recv().is_ok() {
                    return Ok((evidence, Some(Rejection::OutputLimit)));
                }
                let status = unsafe { WaitForSingleObject(custody.child.0, 5) };
                if status == WAIT_OBJECT_0 {
                    return Ok((evidence, None));
                }
                if status != WAIT_TIMEOUT {
                    return Err(io::Error::last_os_error());
                }
                if started.elapsed() >= Duration::from_millis(WALL_LIMIT_MS) {
                    return Ok((evidence, Some(Rejection::Deadline)));
                }
            }
        })();
        // Refusal paths and normal paths share exact same bounded custody exit.
        let drained = match custody.finish() {
            Ok(value) => value,
            Err(error) => {
                self.uncertain.set(true);
                return Err(error);
            }
        };
        let (evidence, mut rejection) = execution?;
        if limit_receive.try_recv().is_ok() {
            rejection = Some(Rejection::OutputLimit);
        }
        if drained.exit_code != 0 && rejection.is_none() {
            rejection = Some(Rejection::ProcessFailure);
        }
        if rejection.is_none() {
            drained.writer?;
        }
        let value = if rejection.is_none() {
            match serde_json::from_slice(&drained.output) {
                Ok(value) => Some(value),
                Err(_) => {
                    rejection = Some(Rejection::InvalidResult);
                    None
                }
            }
        } else {
            None
        };
        Ok(Outcome {
            token: evidence,
            value,
            rejection,
            exit_code: drained.exit_code,
            elapsed: started.elapsed(),
            active_processes: custody.job.active()?,
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Path is a create-only fixture owned by this instance under Temp.
        // No package path or user Runtime profile can enter this deletion.
        if self.uncertain.get() {
            self.profile.preserve = true;
            return;
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(test)]
mod acceptance_controls {
    use super::*;
    use serde_json::json;

    fn cleanup_failure_custody(
        uncertain: &Cell<bool>,
        attempted: bool,
        verified: bool,
    ) -> Custody<'_> {
        // A signaled wait object keeps this deterministic Drop-only control
        // free of child processes. The owned reader thread returns an actual
        // IO error through finish's worker-result path; no OS failure or
        // leaked process needs to be manufactured by the regression.
        let event = Handle::new(unsafe { CreateEventW(null(), 1, 1, null()) }).unwrap();
        Custody {
            uncertain,
            child: event,
            job: Job::new().unwrap(),
            assigned: false,
            cleanup_attempted: attempted,
            verified_finished: verified,
            reader: if !attempted && !verified {
                Some(thread::spawn(|| {
                    Err(io::Error::other("PIPE_WORKER_TEST_FAILURE"))
                }))
            } else {
                None
            },
            writer: None,
        }
    }

    #[test]
    fn custody_drop_propagates_failed_and_previously_attempted_cleanup() {
        let observed = Cell::new(false);
        let mut control = cleanup_failure_custody(&observed, false, false);
        let error = control
            .finish()
            .err()
            .expect("reader error must refuse verification");
        assert_eq!(error.to_string(), "PIPE_WORKER_TEST_FAILURE");
        drop(control);
        assert!(observed.get());
        for (attempted, verified, expected) in [
            (false, false, true),
            (true, false, true),
            (true, true, false),
        ] {
            let uncertain = Cell::new(false);
            let custody = cleanup_failure_custody(&uncertain, attempted, verified);
            drop(custody);
            assert_eq!(uncertain.get(), expected);
        }
    }

    #[test]
    fn failed_drop_cleanup_refuses_reuse_and_preserves_owned_fixture_profile() {
        let fixture = Fixture::new().unwrap();
        let root = fixture.root.clone();
        let profile_name = fixture.profile.name.clone();
        let profile_folder = fixture.profile.folder.clone();
        let original = fs::read(fixture.canary_path()).unwrap();
        drop(cleanup_failure_custody(&fixture.uncertain, false, false));
        assert!(fixture.uncertain.get());
        let error = fixture.execute("return 42", Value::Null).unwrap_err();
        assert!(
            error.to_string().contains("Fixture cleanup is uncertain"),
            "{error}"
        );
        assert_eq!(fs::read(fixture.canary_path()).unwrap(), original);
        let error = fixture.close().unwrap_err();
        assert!(
            error.to_string().contains("owned artifacts retained"),
            "{error}"
        );
        // This is an explicit test-only cleanup of the exact create-only
        // fixture whose synthetic control created no child. Production never
        // clears uncertainty or adopts/reaps a preserved profile automatically.
        let mut retained_profile = OwnedProfile {
            name: profile_name,
            folder: profile_folder,
            removed: false,
            preserve: false,
        };
        let retained = root.exists() && retained_profile.folder.exists();
        fs::remove_dir_all(&root).unwrap();
        retained_profile.close().unwrap();
        assert!(
            retained,
            "uncertain fixture evidence and OS profile must survive Drop"
        );
        assert!(!root.exists());
        assert!(!retained_profile.folder.exists());
    }

    fn current_error_policy() -> (u32, i32, u32) {
        let mut flags = 0;
        let result = unsafe {
            windows_sys::Win32::System::ErrorReporting::WerGetFlags(GetCurrentProcess(), &mut flags)
        };
        (unsafe { GetErrorMode() }, result, flags)
    }

    #[test]
    fn parent_error_policy_is_restored_after_success_and_create_refusal() {
        let before = current_error_policy();
        let fixture = Fixture::new().unwrap();
        let outcome = fixture.execute("return 42", Value::Null).unwrap();
        assert_eq!(outcome.rejection, None, "{outcome:?}");
        assert_eq!(current_error_policy(), before);
        fs::remove_file(fixture.executable_path()).unwrap();
        let error = fixture.execute("return 42", Value::Null).unwrap_err();
        assert!(
            error.to_string().contains("CreateProcessW LPAC refused"),
            "{error}"
        );
        assert!(error.to_string().contains("os error 2"), "{error}");
        assert_eq!(current_error_policy(), before);
        fixture.close().unwrap();
    }

    #[test]
    fn native_startup_diagnostic_uses_only_fixed_build_owned_image() {
        let fixture = Fixture::with_binary(Path::new(env!("FLOWTOOLS_P22_STARTUP_PROBE"))).unwrap();
        let outcome = fixture.execute("return 42", Value::Null).unwrap();
        assert_eq!(outcome.rejection, None, "{outcome:?}");
        assert_eq!(outcome.value, Some(json!({"value":42})));
        assert_eq!(outcome.active_processes, 0);
        fixture.close().unwrap();
    }

    #[test]
    fn ordinary_appcontainer_positive_control_and_lpac_aap_refusal() {
        let fixture = Fixture::new().unwrap();
        let request = json!({"operation":"read","path":fixture.aap_canary_path()});
        let ordinary = fixture
            .run_inner("--native-probe", request.clone(), false, fixture.sid.0)
            .unwrap();
        assert_eq!(ordinary.rejection, None, "{ordinary:?}");
        assert!(!ordinary.token.aap_access_denied);
        assert_eq!(ordinary.value.unwrap()["allowed"], true);
        assert_eq!(ordinary.active_processes, 0);
        let lpac = fixture.native_probe(request).unwrap();
        assert_eq!(lpac.rejection, None, "{lpac:?}");
        assert!(lpac.token.aap_access_denied);
        assert_eq!(lpac.value.unwrap()["allowed"], false);
        assert_eq!(lpac.active_processes, 0);
        fixture.close().unwrap();
    }

    #[test]
    fn wrong_bound_sid_refuses_before_resume_and_drains_suspended_child() {
        let fixture = Fixture::new().unwrap();
        let sibling = Fixture::new().unwrap();
        let started = Instant::now();
        let error = fixture
            .run_inner(
                "--js",
                json!({"source":"return 42","input":null}),
                true,
                sibling.sid.0,
            )
            .unwrap_err();
        assert!(error.to_string().contains("sid_matches: false"), "{error}");
        assert!(started.elapsed() < Duration::from_millis(TERMINATE_GRACE_MS * 2 + 100));
        // run_inner returns the setup error only after Custody::finish checks
        // the suspended process is signaled, Job empty and workers joined.
        let outcome = fixture.execute("return 42", Value::Null).unwrap();
        assert_eq!(outcome.rejection, None, "{outcome:?}");
        assert_eq!(outcome.active_processes, 0);
        fixture.close().unwrap();
        sibling.close().unwrap();
    }
}
