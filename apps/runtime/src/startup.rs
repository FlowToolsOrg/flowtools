use crate::{profile, security};
use std::{
    io::{BufRead, BufReader, Read},
    os::windows::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    ptr,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
    Security::{
        Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1},
        SECURITY_ATTRIBUTES,
    },
    System::{
        Pipes::WaitNamedPipeW,
        Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
    },
};

struct Lock(HANDLE);
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}
fn lock(pipe: &str, sid: &str) -> Result<Lock, &'static str> {
    lock_for(pipe, sid, 30000)
}
fn lock_for(pipe: &str, sid: &str, milliseconds: u32) -> Result<Lock, &'static str> {
    let name: Vec<u16> = format!(
        "Global\\{}-startup",
        pipe.rsplit('\\').next().ok_or("SETUP_REQUIRED")?
    )
    .encode_utf16()
    .chain(Some(0))
    .collect();
    let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err("STARTUP_LOCK_FAILED");
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let handle = CreateMutexW(&attributes, 0, name.as_ptr());
        LocalFree(descriptor);
        if handle.is_null() {
            return Err("STARTUP_LOCK_FAILED");
        }
        match WaitForSingleObject(handle, milliseconds) {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Lock(handle)),
            _ => {
                CloseHandle(handle);
                Err("STARTUP_TIMEOUT")
            }
        }
    }
}

fn present(pipe: &str) -> bool {
    let name: Vec<u16> = pipe.encode_utf16().chain(Some(0)).collect();
    unsafe {
        WaitNamedPipeW(name.as_ptr(), 1) != 0
            || matches!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(121 | 231)
            )
    }
}

// The named mutex is acquired and released on this synchronous OS thread.
pub fn dispatch(args: &[String]) -> Option<Result<(), &'static str>> {
    if args.first().is_some_and(|arg| arg == "--storage") {
        return Some(storage(args));
    }
    if args.len() != 2
        || !matches!(
            args[0].as_str(),
            "--ensure-runtime" | "--endpoint" | "--desktop-bootstrap" | "--manager-bootstrap"
        )
    {
        return None;
    }
    Some((|| {
        let (path, bootstrap) = profile::load(Path::new(&args[1])).map_err(|_| "SETUP_REQUIRED")?;
        let sid = security::current_user_sid().map_err(|_| "IPC_ACCESS_FAILED")?;
        let pipe = profile::pipe(&path, &sid);
        if matches!(
            args[0].as_str(),
            "--desktop-bootstrap" | "--manager-bootstrap"
        ) {
            // Local T0 native client bootstrap; captured by native Host code, never IPC/JS.
            println!(
                "{}",
                serde_json::json!({"pipe":pipe, "token":if args[0] == "--desktop-bootstrap" {bootstrap.desktop_token} else {bootstrap.management_token}})
            );
            return Ok(());
        }
        if args[0] == "--ensure-runtime" {
            let _lock = lock(&pipe, &sid)?;
            if !present(&pipe) {
                let exe = std::env::current_exe().map_err(|_| "SETUP_REQUIRED")?;
                let mut command = Command::new(exe);
                command
                    .args([
                        "--profile",
                        path.to_str().ok_or("SETUP_REQUIRED")?,
                        "--cold-start",
                    ])
                    .creation_flags(0x08000000)
                    .env_clear()
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                if let Some(root) = std::env::var_os("SystemRoot") {
                    command.env("SystemRoot", root);
                }
                let mut child = command.spawn().map_err(|_| "STARTUP_FAILED")?;
                let stdout = child.stdout.take().ok_or("STARTUP_FAILED")?;
                let stderr = child.stderr.take().ok_or("STARTUP_FAILED")?;
                let (sender, receiver) = std::sync::mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    let mut line = String::new();
                    let result = BufReader::new(stdout)
                        .take(2048)
                        .read_line(&mut line)
                        .map(|_| line);
                    let _ = sender.send(result);
                });
                let ready = receiver
                    .recv_timeout(Duration::from_secs(10))
                    .ok()
                    .and_then(Result::ok)
                    .and_then(|line| serde_json::from_str::<serde_json::Value>(&line).ok())
                    .is_some_and(|v| {
                        v["type"] == "ready" && v["protocolMajor"] == 1 && v["pipe"] == pipe
                    });
                if !ready {
                    let _ = child.kill();
                    let _ = child.wait();
                    let mut code = String::new();
                    let _ = stderr.take(4096).read_to_string(&mut code);
                    if code.trim() == "COLD_START_DENIED" {
                        return Err("COLD_START_DENIED");
                    }
                    if code.trim() == "BUNDLE_INTEGRITY_FAILED" {
                        return Err("BUNDLE_INTEGRITY_FAILED");
                    }
                    match code.trim() {
                        "RECOVERY_PENDING" => return Err("RECOVERY_PENDING"),
                        "STORE_CORRUPT" => return Err("STORE_CORRUPT"),
                        "STORE_BUSY" => return Err("STORE_BUSY"),
                        "SCHEMA_UNSUPPORTED" => return Err("SCHEMA_UNSUPPORTED"),
                        "STORAGE_FAILED" => return Err("STORAGE_FAILED"),
                        _ => {}
                    }
                    return Err("STARTUP_DENIED_OR_FAILED");
                }
                // Readiness reader may drop its pipe; the Host emits no further stdout.
                drop(child);
            }
        }
        println!(
            "{}",
            serde_json::json!({"type":"endpoint","pipe":pipe,"protocolMajor":1})
        );
        Ok(())
    })())
}

fn storage(args: &[String]) -> Result<(), &'static str> {
    use flowtools_runtime_core::{protocol::StorageAction, recovery};
    let result = (|| {
        if args.len() != 3 || args[2].len() > 8192 {
            return Err(flowtools_runtime_core::protocol::ErrorCode::InvalidRequest);
        }
        let action: StorageAction = serde_json::from_str(&args[2])
            .map_err(|_| flowtools_runtime_core::protocol::ErrorCode::InvalidRequest)?;
        let (path, _) = profile::load(Path::new(&args[1]))?;
        let sid = security::current_user_sid()
            .map_err(|_| flowtools_runtime_core::protocol::ErrorCode::StorageFailed)?;
        let pipe = profile::pipe(&path, &sid);
        let _lock = lock(&pipe, &sid)
            .map_err(|_| flowtools_runtime_core::protocol::ErrorCode::StoreBusy)?;
        // Reserve the same first pipe as online startup before opening SQLite.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| flowtools_runtime_core::protocol::ErrorCode::StorageFailed)?;
        let _enter = runtime.enter();
        let _reservation = security::create_pipe(&pipe, &sid, true)
            .map_err(|_| flowtools_runtime_core::protocol::ErrorCode::StoreBusy)?;
        recovery::manage(&path, action)
    })();
    match result {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string(&report).map_err(|_| "INVALID_RESPONSE")?
            );
            Ok(())
        }
        Err(code) => {
            println!("{}", serde_json::json!({"error":{"code":code}}));
            Err("STORAGE_OPERATION_FAILED")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_lock_wait_and_owner_release_are_real_os_operations() {
        let pipe = format!(r"\\.\pipe\flowtools-managed-{}", uuid::Uuid::new_v4());
        let sid = security::current_user_sid().unwrap();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let name = pipe.clone();
        let user = sid.clone();
        let owner = std::thread::spawn(move || {
            let _guard = lock_for(&name, &user, 1000).unwrap();
            ready_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        ready_rx.recv().unwrap();
        assert!(matches!(lock_for(&pipe, &sid, 5), Err("STARTUP_TIMEOUT")));
        release_tx.send(()).unwrap();
        owner.join().unwrap();
        assert!(lock_for(&pipe, &sid, 1000).is_ok());
    }
}
