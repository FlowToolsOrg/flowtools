use std::{io, ptr};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1,
        },
        GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

pub fn current_user_sid() -> io::Result<String> {
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let result = (|| {
            let mut needed = 0;
            GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut needed);
            let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let user = &*(buffer.as_ptr() as *const TOKEN_USER);
            let mut text = ptr::null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut length = 0;
            while *text.add(length) != 0 {
                length += 1;
            }
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
            LocalFree(text.cast());
            Ok(sid)
        })();
        CloseHandle(token);
        result
    }
}

pub fn create_pipe(name: &str, sid: &str, first: bool) -> io::Result<NamedPipeServer> {
    // Protected DACL: only the current token user, no Everyone/network ACE.
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
            return Err(io::Error::last_os_error());
        }
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let result = ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .max_instances(16)
            .create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            );
        LocalFree(descriptor);
        result
    }
}

pub fn create_private_directory(path: &std::path::Path, sid: &str) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Security::{
            Authorization::{SetNamedSecurityInfoW, SE_FILE_OBJECT},
            GetSecurityDescriptorDacl, DACL_SECURITY_INFORMATION,
            PROTECTED_DACL_SECURITY_INFORMATION,
        },
        Storage::FileSystem::CreateDirectoryW,
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let sddl: Vec<u16> = format!("D:P(A;OICI;FA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let result = if path.exists() {
            let mut present = 0;
            let mut defaulted = 0;
            let mut dacl = ptr::null_mut();
            if GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) == 0 {
                Err(io::Error::last_os_error())
            } else {
                let error = SetNamedSecurityInfoW(
                    name.as_mut_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    dacl,
                    ptr::null_mut(),
                );
                if error == 0 {
                    Ok(())
                } else {
                    Err(io::Error::from_raw_os_error(error as i32))
                }
            }
        } else if CreateDirectoryW(name.as_ptr(), &attributes) == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        };
        LocalFree(descriptor);
        result?;
    }
    verify_private_path(path, sid, true)
}

pub fn verify_private_path(path: &std::path::Path, sid: &str, protected: bool) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::{
        Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT},
        GetAce, GetSecurityDescriptorControl, ACCESS_ALLOWED_ACE, DACL_SECURITY_INFORMATION,
    };
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut descriptor = ptr::null_mut();
        let mut dacl = ptr::null_mut();
        let error = GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        );
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        let result = (|| {
            if dacl.is_null() || (*dacl).AceCount != 1 {
                return Err(io::Error::other("PROFILE_ACL_INVALID"));
            }
            let mut control = 0;
            let mut revision = 0;
            if GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) == 0
                || (protected && control & 0x1000 == 0)
            {
                return Err(io::Error::other("PROFILE_ACL_INVALID"));
            }
            let mut ace = ptr::null_mut();
            if GetAce(dacl, 0, &mut ace) == 0 {
                return Err(io::Error::last_os_error());
            }
            let ace = ace as *const ACCESS_ALLOWED_ACE;
            if (*ace).Header.AceType != 0 {
                return Err(io::Error::other("PROFILE_ACL_INVALID"));
            }
            let mut text = ptr::null_mut();
            if ConvertSidToStringSidW(ptr::addr_of!((*ace).SidStart).cast_mut().cast(), &mut text)
                == 0
            {
                return Err(io::Error::last_os_error());
            }
            let mut len = 0;
            while *text.add(len) != 0 {
                len += 1;
            }
            let found = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            if found != sid {
                return Err(io::Error::other("PROFILE_ACL_INVALID"));
            }
            Ok(())
        })();
        LocalFree(descriptor);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_profile_and_inherited_files_have_only_current_user_acl() {
        let path =
            std::env::temp_dir().join(format!("flowtools-validation-acl-{}", uuid::Uuid::new_v4()));
        let sid = current_user_sid().unwrap();
        create_private_directory(&path, &sid).unwrap();
        let file = path.join("private-fixture");
        std::fs::write(&file, b"fixture").unwrap();
        verify_private_path(&path, &sid, true).unwrap();
        verify_private_path(&file, &sid, false).unwrap();
        assert!(verify_private_path(&path, "S-1-5-18", true).is_err());
    }
    #[tokio::test]
    async fn current_user_acl_and_first_instance_refuse_duplicate_listener() {
        let sid = current_user_sid().unwrap();
        assert!(sid.starts_with("S-1-"));
        let name = format!(r"\\.\pipe\flowtools-security-test-{}", uuid::Uuid::new_v4());
        let _pipe = create_pipe(&name, &sid, true).unwrap();
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Security::{
            Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT},
            GetAce, GetSecurityDescriptorControl, ACCESS_ALLOWED_ACE, DACL_SECURITY_INFORMATION,
        };
        unsafe {
            let mut descriptor = ptr::null_mut();
            let mut dacl = ptr::null_mut();
            assert_eq!(
                GetSecurityInfo(
                    _pipe.as_raw_handle(),
                    SE_KERNEL_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut dacl,
                    ptr::null_mut(),
                    &mut descriptor
                ),
                0
            );
            assert!(!dacl.is_null());
            assert_eq!((*dacl).AceCount, 1);
            let mut control = 0;
            let mut revision = 0;
            assert_ne!(
                GetSecurityDescriptorControl(descriptor, &mut control, &mut revision),
                0
            );
            assert_ne!(control & 0x1000, 0); // SE_DACL_PROTECTED
            let mut ace = ptr::null_mut();
            assert_ne!(GetAce(dacl, 0, &mut ace), 0);
            let ace = ace as *const ACCESS_ALLOWED_ACE;
            assert_eq!((*ace).Header.AceType, 0); // ACCESS_ALLOWED_ACE_TYPE
            let mut text = ptr::null_mut();
            assert_ne!(
                ConvertSidToStringSidW(ptr::addr_of!((*ace).SidStart).cast_mut().cast(), &mut text),
                0
            );
            let mut length = 0;
            while *text.add(length) != 0 {
                length += 1;
            }
            let allowed_sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
            LocalFree(text.cast());
            LocalFree(descriptor);
            assert_eq!(allowed_sid, sid);
        }
        assert!(create_pipe(&name, &sid, true).is_err());
    }
}
