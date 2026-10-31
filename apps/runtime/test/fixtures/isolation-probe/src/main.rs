#[cfg(not(windows))]
fn main() {
    panic!("Windows-only fixture");
}
#[cfg(windows)]
#[path = "../../../../tests/isolation/token.rs"]
mod evidence;
#[cfg(windows)]
use evidence::*;
#[cfg(windows)]
use std::{
    fs,
    io::Write,
    net::{SocketAddr, TcpStream},
    time::Duration,
};
#[cfg(windows)]
use windows_sys::Win32::Foundation::FreeLibrary;
#[cfg(windows)]
use windows_sys::Win32::Security::{
    TokenAppContainerSid, TokenCapabilities, TokenIsAppContainer, TOKEN_APPCONTAINER_INFORMATION,
};
#[cfg(windows)]
use windows_sys::Win32::System::{
    LibraryLoader::{GetModuleFileNameW, LoadLibraryW},
    Memory::{VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE},
    Registry::{RegCloseKey, RegOpenKeyExW, HKEY_USERS, KEY_READ},
    Threading::{GetCurrentProcess, GetCurrentProcessId},
};

#[cfg(windows)]
fn main() {
    let root = std::env::var_os("FLOWTOOLS_ISOLATION_ROOT").expect("test-only fixture root");
    let root = std::path::PathBuf::from(root);
    let work = root.join("work");
    let case = std::env::var("FLOWTOOLS_ISOLATION_CASE").unwrap();
    if case == "grandchild" {
        fs::write(work.join("grandchild-ran"), b"child").unwrap();
        return;
    }
    if case == "spin" {
        fs::write(work.join("ready"), b"ready").unwrap();
        loop {
            std::hint::spin_loop();
        }
    }
    if case == "flood" {
        fs::write(work.join("ready"), b"ready").unwrap();
        let mut output = std::io::stdout().lock();
        let block = [b'x'; 8192];
        loop {
            output.write_all(&block).unwrap();
        }
    }
    if case == "child" {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .env("FLOWTOOLS_ISOLATION_CASE", "grandchild")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        let report = serde_json::json!({"pid":unsafe {GetCurrentProcessId()},
            "childCreated":result.is_ok(), "childError":result.err().and_then(|e| e.raw_os_error())});
        fs::write(
            work.join("report.json"),
            serde_json::to_vec(&report).unwrap(),
        )
        .unwrap();
        return;
    }
    if case == "tool-files" {
        tool_files(&work);
        return;
    }
    let process = unsafe { GetCurrentProcess() };
    let is_container = token_u32(process, TokenIsAppContainer).unwrap();
    let is_lpac = token_lpac(process).unwrap();
    let capabilities = if is_container == 1 {
        token_u32(process, TokenCapabilities).unwrap()
    } else {
        0
    };
    let actual_sid = if is_container == 1 {
        let buffer = token(process, TokenAppContainerSid).unwrap();
        sid_text(unsafe {
            (*buffer.as_ptr().cast::<TOKEN_APPCONTAINER_INFORMATION>()).TokenAppContainer
        })
        .unwrap()
    } else {
        String::new()
    };
    let mut key = std::ptr::null_mut();
    let registry_status = unsafe {
        RegOpenKeyExW(
            HKEY_USERS,
            wide(std::env::var("FLOWTOOLS_ISOLATION_REGISTRY").unwrap()).as_ptr(),
            0,
            KEY_READ,
            &mut key,
        )
    };
    if registry_status == 0 {
        unsafe {
            RegCloseKey(key);
        }
    }
    let other_read = fs::read(root.join("other-task/private.txt")).is_ok();
    let other_write = fs::write(root.join("other-task/escape.txt"), b"escape").is_ok();
    let junction_read = fs::read(work.join("redirect/private.txt")).is_ok();
    let junction_write = fs::write(work.join("redirect/redirected.txt"), b"escape").is_ok();
    let ipc = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(std::env::var_os("FLOWTOOLS_ISOLATION_PIPE").unwrap());
    let ipc_error = ipc.as_ref().err().and_then(|e| e.raw_os_error());
    let host_read = fs::read(root.join("host/private.txt")).ok();
    let host_write = fs::write(root.join("host/escape.txt"), b"escape").is_ok();
    let self_write = fs::write(work.join("allowed.txt"), b"allowed").is_ok();
    let port: u16 = std::env::var("FLOWTOOLS_ISOLATION_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let network = TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(500),
    );
    let network_error = network.as_ref().err().and_then(|e| e.raw_os_error());
    let memory = unsafe {
        VirtualAlloc(
            std::ptr::null(),
            256 * 1024 * 1024,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    let memory_allowed = !memory.is_null();
    if memory_allowed {
        unsafe {
            VirtualFree(memory, 0, MEM_RELEASE);
        }
    }
    let low_dll = unsafe { LoadLibraryW(wide(work.join("low-image.dll")).as_ptr()) };
    let low_dll_allowed = !low_dll.is_null();
    if low_dll_allowed {
        unsafe {
            FreeLibrary(low_dll);
        }
    }
    let sys_dll = unsafe { LoadLibraryW(wide("version.dll").as_ptr()) };
    let mut path = [0u16; 32768];
    let len = if sys_dll.is_null() {
        0
    } else {
        unsafe { GetModuleFileNameW(sys_dll, path.as_mut_ptr(), path.len() as u32) }
    } as usize;
    let system_dll = String::from_utf16_lossy(&path[..len])
        .to_ascii_lowercase()
        .contains("\\system32\\");
    if !sys_dll.is_null() {
        unsafe {
            FreeLibrary(sys_dll);
        }
    }
    let report = serde_json::json!({
        "pid":unsafe{GetCurrentProcessId()}, "container":is_container,
        "lpac":is_lpac, "capabilities":capabilities, "sid":actual_sid,
        "registryStatus":registry_status,
        "otherRead":other_read, "otherWrite":other_write,
        "junctionRead":junction_read, "junctionWrite":junction_write,
        "ipc":ipc.is_ok(), "ipcError":ipc_error,
        "hostRead":host_read.is_some(),"hostWrite":host_write,
        "workspaceWrite":self_write,"network":network.is_ok(),"networkError":network_error,
        "memoryAllowed":memory_allowed,"lowDllAllowed":low_dll_allowed,"systemDll":system_dll,
        "hostEnvironment":std::env::var_os("USERPROFILE").is_some()||std::env::var_os("PATH").is_some()
    });
    fs::write(
        work.join("report.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
}

#[cfg(windows)]
fn tool_files(work: &std::path::Path) {
    use std::{mem::size_of, os::windows::io::AsRawHandle, ptr};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW,
                SDDL_REVISION_1, SE_FILE_OBJECT,
            },
            GetSecurityDescriptorSacl, LABEL_SECURITY_INFORMATION,
        },
        Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION},
    };
    let root = std::path::PathBuf::from(std::env::var_os("FLOWTOOLS_TOOL_FIXTURE_ROOT").unwrap());
    let other = std::path::PathBuf::from(std::env::var_os("FLOWTOOLS_TOOL_FIXTURE_OTHER").unwrap());
    let path = root.join("bin/ffmpeg.exe");
    let opened = fs::File::open(&path);
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    let identity = opened.as_ref().ok().and_then(|file| {
        (unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } != 0).then(
            || {
                format!(
                    "{:08x}:{:08x}{:08x}",
                    information.dwVolumeSerialNumber,
                    information.nFileIndexHigh,
                    information.nFileIndexLow
                )
            },
        )
    });
    let read = fs::read(&path).ok();
    let write = fs::OpenOptions::new().write(true).open(&path).is_ok();
    let mut descriptor = ptr::null_mut();
    let mut relabel = false;
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide("S:(ML;;NW;;;LW)").as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        ) != 0
        {
            let (mut present, mut defaulted) = (0, 0);
            let mut label = ptr::null_mut();
            assert_ne!(
                GetSecurityDescriptorSacl(descriptor, &mut present, &mut label, &mut defaulted),
                0
            );
            relabel = SetNamedSecurityInfoW(
                wide(&path).as_ptr(),
                SE_FILE_OBJECT,
                LABEL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                label,
            ) == 0;
            LocalFree(descriptor);
        }
    }
    let process = unsafe { GetCurrentProcess() };
    let buffer = token(process, TokenAppContainerSid).unwrap();
    assert!(buffer.len() * size_of::<usize>() >= size_of::<TOKEN_APPCONTAINER_INFORMATION>());
    let sid = sid_text(unsafe {
        (*buffer.as_ptr().cast::<TOKEN_APPCONTAINER_INFORMATION>()).TokenAppContainer
    })
    .unwrap();
    let report = serde_json::json!({"pid":unsafe {GetCurrentProcessId()},"sid":sid,"lpac":token_lpac(process).unwrap(),"capabilities":token_u32(process,TokenCapabilities).unwrap(),"identity":identity,"bytes":read,"write":write,"relabel":relabel,"otherRead":fs::read(other.join("bin/ffmpeg.exe")).is_ok()});
    fs::write(
        work.join("report.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
}
