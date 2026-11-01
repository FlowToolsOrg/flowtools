//! Closed Windows/x64 LPAC kernel for ffprobe inspection. No uncontained branch,
//! caller argv/environment, PID reopening or public executable/handle getter.
use super::token::{sid_text, token, token_lpac, token_u32, wide};
use crate::packages::tools::media::{self, MediaActionV1};
use std::{
    ffi::c_void,
    io,
    mem::{size_of, size_of_val},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{LocalFree, HANDLE, INVALID_HANDLE_VALUE},
    Security::{
        FreeSid, GetLengthSid,
        Isolation::{CreateAppContainerProfile, DeleteAppContainerProfile},
        TokenAppContainerSid, TokenCapabilities, TokenIsAppContainer, SECURITY_ATTRIBUTES,
        SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES, TOKEN_APPCONTAINER_INFORMATION, TOKEN_GROUPS,
    },
    Storage::FileSystem::{
        CreateFileW, ReadFile, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    },
    System::{
        JobObjects::*,
        Pipes::{CreatePipe, PeekNamedPipe},
        SystemInformation::GetWindowsDirectoryW,
        Threading::*,
        WindowsProgramming::{
            PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT,
            PROCESS_CREATION_CHILD_PROCESS_RESTRICTED,
        },
    },
};

fn last() -> io::Error {
    io::Error::last_os_error()
}
fn owned(raw: HANDLE) -> io::Result<OwnedHandle> {
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        return Err(last());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}
fn copied_sid(raw: *mut c_void) -> io::Result<Vec<usize>> {
    let length = unsafe { GetLengthSid(raw) } as usize;
    if !(8..=512).contains(&length) {
        return Err(io::Error::other("SID size refused"));
    }
    let mut bytes = vec![0usize; length.div_ceil(size_of::<usize>())];
    unsafe { ptr::copy_nonoverlapping(raw.cast::<u8>(), bytes.as_mut_ptr().cast::<u8>(), length) };
    Ok(bytes)
}
pub(super) struct Container {
    name: Vec<u16>,
    sid: Vec<usize>,
    text: String,
    closed: bool,
}
impl Container {
    pub(super) fn new(profile: &str, id: &str) -> io::Result<Self> {
        if profile.len() != 64
            || !profile
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || uuid::Uuid::parse_str(id).is_err()
        {
            return Err(io::Error::other("Native call identity refused"));
        }
        let name = wide(format!(
            "FlowTools.Media.{}.{}",
            &profile[..12],
            uuid::Uuid::parse_str(id).unwrap().simple()
        ));
        let mut raw = ptr::null_mut();
        let status = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                ptr::null(),
                0,
                &mut raw,
            )
        };
        if status < 0 || raw.is_null() {
            return Err(io::Error::other("Unique AppContainer creation refused"));
        }
        let sid = copied_sid(raw);
        let text = sid_text(raw);
        unsafe { FreeSid(raw) };
        match (sid, text) {
            (Ok(sid), Ok(text)) => Ok(Self {
                name,
                sid,
                text,
                closed: false,
            }),
            _ => {
                unsafe { DeleteAppContainerProfile(name.as_ptr()) };
                Err(io::Error::other("Created AppContainer identity refused"))
            }
        }
    }
    pub(super) fn text(&self) -> &str {
        &self.text
    }
    fn sid(&self) -> *mut c_void {
        self.sid.as_ptr().cast_mut().cast()
    }
    pub(super) fn close(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        if unsafe { DeleteAppContainerProfile(self.name.as_ptr()) } < 0 {
            return Err(io::Error::other("Owned AppContainer cleanup failed"));
        }
        self.closed = true;
        Ok(())
    }
}
impl Drop for Container {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
struct Capability(Vec<usize>);
impl Capability {
    fn derive(name: &str) -> io::Result<Self> {
        let (mut groups, mut caps) = (ptr::null_mut(), ptr::null_mut());
        let (mut ng, mut nc) = (0, 0);
        if unsafe {
            windows_sys::Win32::Security::DeriveCapabilitySidsFromName(
                wide(name).as_ptr(),
                &mut groups,
                &mut ng,
                &mut caps,
                &mut nc,
            )
        } == 0
        {
            return Err(last());
        }
        let result = if nc == 1 && !caps.is_null() {
            copied_sid(unsafe { *caps }).map(Self)
        } else {
            Err(io::Error::other("Exact capability count refused"))
        };
        unsafe {
            for i in 0..ng {
                LocalFree((*groups.add(i as usize)).cast());
            }
            LocalFree(groups.cast());
            for i in 0..nc {
                LocalFree((*caps.add(i as usize)).cast());
            }
            LocalFree(caps.cast());
        }
        result
    }
    fn sid(&self) -> *mut c_void {
        self.0.as_ptr().cast_mut().cast()
    }
}
pub(super) struct Process {
    retention: Option<std::sync::Arc<super::Retention>>,
    process: OwnedHandle,
    thread: OwnedHandle,
    job: OwnedHandle,
    stdout: OwnedHandle,
    stderr: OwnedHandle,
    pid: u32,
    resumed: Option<Instant>,
    finished: bool,
    bytes: usize,
    out: Vec<u8>,
}
pub(super) enum Poll {
    Running,
    Exited { code: u32, stdout: Vec<u8> },
}
impl Process {
    #[cfg(test)]
    pub(super) fn snapshot(&self) -> io::Result<(u32, u32, bool, String, bool)> {
        Ok((
            self.pid,
            self.active()?,
            self.exited()?,
            self.retention.as_ref().unwrap().container.text().to_owned(),
            self.resumed.is_some(),
        ))
    }
    pub(super) fn prepare(
        image: &Path,
        work: &Path,
        container: &Container,
        artifact: &str,
        retention: std::sync::Arc<super::Retention>,
    ) -> io::Result<Self> {
        unsafe {
            let job = owned(CreateJobObjectW(ptr::null(), ptr::null()))?;
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_PROCESS_MEMORY
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            limits.BasicLimitInformation.ActiveProcessLimit = 1;
            limits.ProcessMemoryLimit = 512 * 1024 * 1024;
            if SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            ) == 0
            {
                return Err(last());
            }
            let mut readback = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            if QueryInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&mut readback as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&readback) as u32,
                ptr::null_mut(),
            ) == 0
                || readback.BasicLimitInformation.LimitFlags
                    != limits.BasicLimitInformation.LimitFlags
                || readback.BasicLimitInformation.ActiveProcessLimit != 1
                || readback.ProcessMemoryLimit != limits.ProcessMemoryLimit
            {
                return Err(io::Error::other("Native Job limits refused"));
            }
            let cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
                ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE
                    | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
                Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 { CpuRate: 2500 },
            };
            if SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectCpuRateControlInformation,
                (&cpu as *const JOBOBJECT_CPU_RATE_CONTROL_INFORMATION).cast(),
                size_of_val(&cpu) as u32,
            ) == 0
            {
                return Err(last());
            }
            let mut actual_cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION::default();
            if QueryInformationJobObject(
                job.as_raw_handle(),
                JobObjectCpuRateControlInformation,
                (&mut actual_cpu as *mut JOBOBJECT_CPU_RATE_CONTROL_INFORMATION).cast(),
                size_of_val(&actual_cpu) as u32,
                ptr::null_mut(),
            ) == 0
                || actual_cpu.ControlFlags != cpu.ControlFlags
                || actual_cpu.Anonymous.CpuRate != 2500
            {
                return Err(io::Error::other("Native CPU limit refused"));
            }
            let sa = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: ptr::null_mut(),
                bInheritHandle: 1,
            };
            let nul = owned(CreateFileW(
                wide("NUL").as_ptr(),
                0x80000000 | 0x40000000,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &sa,
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            ))?;
            let pipe = || -> io::Result<(OwnedHandle, OwnedHandle)> {
                let (mut read, mut write) = (ptr::null_mut(), ptr::null_mut());
                if CreatePipe(&mut read, &mut write, &sa, 4096) == 0 {
                    return Err(last());
                }
                let pair = (owned(read)?, owned(write)?);
                if windows_sys::Win32::Foundation::SetHandleInformation(
                    pair.0.as_raw_handle(),
                    1,
                    0,
                ) == 0
                {
                    return Err(last());
                }
                Ok(pair)
            };
            let (out_read, out_write) = pipe()?;
            let (err_read, err_write) = pipe()?;
            // Intentionally inheritable, but excluded from the exact stdio
            // allowlist. Read back actual object identity before any user code.
            let private_event = owned(CreateEventW(&sa, 1, 0, ptr::null()))?;
            let registry = Capability::derive("registryRead")?;
            let image_cap = Capability::derive(artifact)?;
            let mut caps = [
                SID_AND_ATTRIBUTES {
                    Sid: registry.sid(),
                    Attributes: 4,
                },
                SID_AND_ATTRIBUTES {
                    Sid: image_cap.sid(),
                    Attributes: 4,
                },
            ];
            let capabilities = SECURITY_CAPABILITIES {
                AppContainerSid: container.sid(),
                Capabilities: caps.as_mut_ptr(),
                CapabilityCount: 2,
                Reserved: 0,
            };
            let handles = [
                nul.as_raw_handle(),
                out_write.as_raw_handle(),
                err_write.as_raw_handle(),
            ];
            let opt_out = PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT;
            let child = PROCESS_CREATION_CHILD_PROCESS_RESTRICTED;
            let mitigation: u64 = (1u64 << 52) | (1u64 << 56) | (1u64 << 60);
            let mut length = 0;
            InitializeProcThreadAttributeList(ptr::null_mut(), 5, 0, &mut length);
            let mut data = vec![0usize; length.div_ceil(size_of::<usize>())];
            let attributes = data.as_mut_ptr().cast();
            if InitializeProcThreadAttributeList(attributes, 5, 0, &mut length) == 0 {
                return Err(last());
            }
            struct Attr(LPPROC_THREAD_ATTRIBUTE_LIST);
            impl Drop for Attr {
                fn drop(&mut self) {
                    unsafe { DeleteProcThreadAttributeList(self.0) }
                }
            }
            let attributes = Attr(attributes);
            let set = |key: u32, value: *const c_void, size: usize| -> io::Result<()> {
                if UpdateProcThreadAttribute(
                    attributes.0,
                    0,
                    key as usize,
                    value,
                    size,
                    ptr::null_mut(),
                    ptr::null(),
                ) == 0
                {
                    Err(last())
                } else {
                    Ok(())
                }
            };
            set(
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                handles.as_ptr().cast(),
                size_of_val(&handles),
            )?;
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
                (&child as *const u32).cast(),
                size_of_val(&child),
            )?;
            set(
                PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY,
                (&mitigation as *const u64).cast(),
                size_of_val(&mitigation),
            )?;
            let mut startup = STARTUPINFOEXW::default();
            startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
            startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            startup.StartupInfo.hStdInput = nul.as_raw_handle();
            startup.StartupInfo.hStdOutput = out_write.as_raw_handle();
            startup.StartupInfo.hStdError = err_write.as_raw_handle();
            startup.lpAttributeList = attributes.0;
            let mut windows = vec![0u16; 32768];
            let count = GetWindowsDirectoryW(windows.as_mut_ptr(), windows.len() as u32) as usize;
            if count == 0 || count >= windows.len() {
                return Err(io::Error::other("Native Windows directory refused"));
            }
            let system = String::from_utf16(&windows[..count])
                .map_err(|_| io::Error::other("Native Windows directory UTF16 refused"))?;
            if system.len() < 3
                || !system.as_bytes()[0].is_ascii_alphabetic()
                || system.as_bytes()[1] != b':'
                || !Path::new(&system).is_absolute()
            {
                return Err(io::Error::other("Native Windows root refused"));
            }
            let mut entries = vec![
                format!("SystemRoot={system}"),
                format!("SystemDrive={}", &system[..2]),
                format!("TEMP={}", work.display()),
                format!("TMP={}", work.display()),
                format!("LOCALAPPDATA={}", work.display()),
            ];
            entries.sort_by_key(|entry| entry.to_ascii_uppercase());
            let mut environment: Vec<u16> = entries.into_iter().flat_map(wide).collect();
            environment.push(0);
            let mut command = wide(format!("\"{}\"", image.display()));
            command.pop();
            for arg in media::recipe(MediaActionV1::Inspect).arguments() {
                command.extend(format!(" \"{arg}\"").encode_utf16());
            }
            command.push(0);
            let mut info = PROCESS_INFORMATION::default();
            if CreateProcessW(
                wide(image).as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                CREATE_SUSPENDED
                    | DETACHED_PROCESS
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT,
                environment.as_ptr().cast(),
                wide(work).as_ptr(),
                &startup.StartupInfo,
                &mut info,
            ) == 0
            {
                return Err(last());
            }
            let process = Self {
                retention: Some(retention),
                process: owned(info.hProcess)?,
                thread: owned(info.hThread)?,
                job,
                stdout: out_read,
                stderr: err_read,
                pid: info.dwProcessId,
                resumed: None,
                finished: false,
                bytes: 0,
                out: Vec::new(),
            };
            process.validate(container, &caps, image, private_event.as_raw_handle())?;
            if AssignProcessToJobObject(
                process.job.as_raw_handle(),
                process.process.as_raw_handle(),
            ) == 0
            {
                return Err(last());
            }
            if process.active()? != 1 {
                return Err(io::Error::other("Owned suspended process absent from Job"));
            }
            Ok(process)
        }
    }
    fn validate(
        &self,
        container: &Container,
        expected: &[SID_AND_ATTRIBUTES],
        image: &Path,
        private_event: HANDLE,
    ) -> io::Result<()> {
        unsafe {
            if GetProcessId(self.process.as_raw_handle()) != self.pid
                || self.pid == 0
                || token_u32(self.process.as_raw_handle(), TokenIsAppContainer)? != 1
                || !token_lpac(self.process.as_raw_handle())?
            {
                return Err(io::Error::other("Actual native process identity refused"));
            }
            let sid = token(self.process.as_raw_handle(), TokenAppContainerSid)?;
            if sid_text(
                (*(sid.as_ptr().cast::<TOKEN_APPCONTAINER_INFORMATION>())).TokenAppContainer,
            )? != container.text
            {
                return Err(io::Error::other("Actual native AppContainer SID refused"));
            }
            let bytes = token(self.process.as_raw_handle(), TokenCapabilities)?;
            if size_of_val(bytes.as_slice())
                < std::mem::offset_of!(TOKEN_GROUPS, Groups) + size_of_val(expected)
            {
                return Err(io::Error::other("Native capability buffer refused"));
            }
            let groups = &*bytes.as_ptr().cast::<TOKEN_GROUPS>();
            if groups.GroupCount as usize != expected.len() {
                return Err(io::Error::other("Native capability count refused"));
            }
            let actual =
                std::slice::from_raw_parts(groups.Groups.as_ptr(), groups.GroupCount as usize);
            let mut wanted = expected
                .iter()
                .map(|group| sid_text(group.Sid))
                .collect::<io::Result<Vec<_>>>()?;
            let mut observed = actual
                .iter()
                .map(|group| sid_text(group.Sid))
                .collect::<io::Result<Vec<_>>>()?;
            wanted.sort();
            observed.sort();
            if wanted != observed
                || actual
                    .iter()
                    .any(|group| group.Attributes & 4 == 0 || group.Attributes & 16 != 0)
            {
                return Err(io::Error::other(
                    "Actual enabled native capabilities refused",
                ));
            }
            let mut child=windows_sys::Win32::System::SystemServices::PROCESS_MITIGATION_CHILD_PROCESS_POLICY::default();
            if GetProcessMitigationPolicy(self.process.as_raw_handle(),ProcessChildProcessPolicy,(&mut child as *mut windows_sys::Win32::System::SystemServices::PROCESS_MITIGATION_CHILD_PROCESS_POLICY).cast(),size_of_val(&child))==0||child.Anonymous.Flags!=1{return Err(io::Error::other("Actual native child policy refused"));}
            let mut loader=windows_sys::Win32::System::SystemServices::PROCESS_MITIGATION_IMAGE_LOAD_POLICY::default();
            if GetProcessMitigationPolicy(self.process.as_raw_handle(),ProcessImageLoadPolicy,(&mut loader as *mut windows_sys::Win32::System::SystemServices::PROCESS_MITIGATION_IMAGE_LOAD_POLICY).cast(),size_of_val(&loader))==0||loader.Anonymous.Flags&7!=7{return Err(io::Error::other("Actual native loader policy refused"));}
            let mut name = vec![0u16; 32768];
            let mut length = name.len() as u32;
            if QueryFullProcessImageNameW(
                self.process.as_raw_handle(),
                0,
                name.as_mut_ptr(),
                &mut length,
            ) == 0
                || length == 0
                || length as usize >= name.len()
            {
                return Err(io::Error::other("Actual native image refused"));
            }
            let actual = PathBuf::from(
                String::from_utf16(&name[..length as usize])
                    .map_err(|_| io::Error::other("Actual image UTF16 refused"))?,
            );
            if std::fs::canonicalize(actual)? != std::fs::canonicalize(image)? {
                return Err(io::Error::other("Actual native image mismatch"));
            }
            let mut duplicate = ptr::null_mut();
            if windows_sys::Win32::Foundation::DuplicateHandle(
                self.process.as_raw_handle(),
                private_event,
                GetCurrentProcess(),
                &mut duplicate,
                0,
                0,
                windows_sys::Win32::Foundation::DUPLICATE_SAME_ACCESS,
            ) != 0
            {
                let duplicate = owned(duplicate)?;
                if windows_sys::Win32::Foundation::CompareObjectHandles(
                    private_event,
                    duplicate.as_raw_handle(),
                ) != 0
                {
                    return Err(io::Error::other("Private native object was inherited"));
                }
            } else if last().raw_os_error() != Some(6) {
                return Err(io::Error::other(
                    "Native inherited-object observation failed",
                ));
            }
            Ok(())
        }
    }
    fn active(&self) -> io::Result<u32> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        if unsafe {
            QueryInformationJobObject(
                self.job.as_raw_handle(),
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
    fn exited(&self) -> io::Result<bool> {
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            0 => Ok(true),
            258 => Ok(false),
            _ => Err(last()),
        }
    }
    pub(super) fn resume(&mut self) -> io::Result<()> {
        if self.resumed.is_some() || self.finished || self.active()? != 1 {
            return Err(io::Error::other(
                "Native process is not suspended and owned",
            ));
        }
        if unsafe { ResumeThread(self.thread.as_raw_handle()) } != 1 {
            return Err(io::Error::other("Native thread resume refused"));
        }
        self.resumed = Some(Instant::now());
        Ok(())
    }
    pub(super) fn drained(&self) -> io::Result<bool> {
        Ok(self.exited()? && self.active()? == 0)
    }
    pub(super) fn cancel(&mut self) -> io::Result<()> {
        if self.drained()? {
            self.finished = true;
            return Ok(());
        }
        if unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
            return Err(last());
        }
        // Admission can fail before assignment; this is still our exact
        // suspended process handle, never a reopened PID.
        if !self.exited()? {
            unsafe { TerminateProcess(self.process.as_raw_handle(), 1) };
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            if self.drained()? {
                self.finished = true;
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(io::Error::other("Owned native Job did not drain"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    pub(super) fn poll(&mut self) -> io::Result<Poll> {
        if self.finished {
            return Err(io::Error::other("Native process already consumed"));
        }
        let started = self
            .resumed
            .ok_or_else(|| io::Error::other("Native process not resumed"))?;
        if started.elapsed() >= Duration::from_secs(5) {
            self.cancel()?;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Native inspection deadline",
            ));
        }
        let mut buffered = 0;
        let exited_before_read = self.exited()?;
        for stdout in [true, false] {
            let pipe = if stdout { &self.stdout } else { &self.stderr };
            let mut available = 0;
            if unsafe {
                PeekNamedPipe(
                    pipe.as_raw_handle(),
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
                    ReadFile(
                        pipe.as_raw_handle(),
                        bytes.as_mut_ptr(),
                        available.min(bytes.len() as u32),
                        &mut read,
                        ptr::null_mut(),
                    )
                } == 0
                {
                    return Err(last());
                }
                self.bytes = self
                    .bytes
                    .checked_add(read as usize)
                    .ok_or_else(|| io::Error::other("Native output size overflow"))?;
                if self.bytes > 65_536 {
                    self.cancel()?;
                    return Err(io::Error::other("Native output budget exceeded"));
                }
                if stdout {
                    self.out.extend_from_slice(&bytes[..read as usize]);
                }
                buffered += available.saturating_sub(read);
            }
        }
        if exited_before_read && buffered == 0 {
            if self.active()? != 0 {
                return Ok(Poll::Running);
            }
            let mut code = 0;
            if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) } == 0 {
                return Err(last());
            }
            self.finished = true;
            return Ok(Poll::Exited {
                code,
                stdout: std::mem::take(&mut self.out),
            });
        }
        Ok(Poll::Running)
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if self.cancel().is_err() {
            // The outer capsule must retain its source/workspace on this error;
            // closing the owned Job still forces termination, never a fallback.
            unsafe {
                TerminateProcess(self.process.as_raw_handle(), 1);
            }
            if !matches!(self.drained(), Ok(true)) {
                // A failed observation cannot authorize releasing an executable
                // or its private input. Preserve the guards for process lifetime.
                if let Some(retention) = self.retention.take() {
                    std::mem::forget(retention);
                }
            }
        }
    }
}
