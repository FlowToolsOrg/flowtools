//! T1 lifetime containment only; not an OS privilege sandbox.
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
        JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
        TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    },
};

pub struct ProcessJob(HANDLE);
// The uniquely owned kernel handle has no thread affinity, and is never exposed.
unsafe impl Send for ProcessJob {}
impl ProcessJob {
    /// Reap the complete owned group before releasing a provider/version lease.
    pub fn active_processes(&self) -> std::io::Result<u32> {
        let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe {
            QueryInformationJobObject(
                self.0,
                JobObjectBasicAccountingInformation,
                (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                std::mem::size_of_val(&info) as u32,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(info.ActiveProcesses)
    }
    pub fn drain(&self) -> std::io::Result<()> {
        unsafe {
            if TerminateJobObject(self.0, 1) == 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if self.active_processes()? == 0 {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(std::io::Error::other("Owned process drain incomplete"));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    pub fn bind(child: &tokio::process::Child) -> std::io::Result<Self> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let job = Self(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
                || AssignProcessToJobObject(
                    handle,
                    child
                        .raw_handle()
                        .ok_or_else(|| std::io::Error::other("Child handle unavailable"))?
                        as HANDLE,
                ) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(job)
        }
    }
}
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn last_handle_close_terminates_owned_runner() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../.generated/runner.json")).unwrap();
        let mut child = tokio::process::Command::new(config["bun"]["path"].as_str().unwrap())
            .arg(config["runner"]["path"].as_str().unwrap())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let job = ProcessJob::bind(&child).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(child.try_wait().unwrap().is_none());
        drop(job);
        let _status = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap();
    }
}
