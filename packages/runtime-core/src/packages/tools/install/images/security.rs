//! Private native security for inert materialized tool groups. Capability names
//! bind the T0 profile and original signed ZIP, never caller-selected policies.
use crate::packages::{install::InstallError, valid_digest};
use std::{ffi::c_void, fs::File, mem::size_of, os::windows::io::AsRawHandle, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree},
    Security::{
        Authorization::{
            ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
            ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SetSecurityInfo,
            SDDL_REVISION_1, SE_FILE_OBJECT,
        },
        DeriveCapabilitySidsFromName, GetSecurityDescriptorDacl, GetSecurityDescriptorLength,
        GetSecurityDescriptorOwner, GetSecurityDescriptorSacl, GetTokenInformation,
        IsValidSecurityDescriptor, TokenUser, DACL_SECURITY_INFORMATION,
        LABEL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};
type Result<T> = std::result::Result<T, InstallError>;
const INFORMATION: u32 =
    OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION | LABEL_SECURITY_INFORMATION;

struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn text(pointer: *const u16) -> Result<String> {
    if pointer.is_null() {
        return Err(InstallError::Storage);
    }
    let mut length = 0;
    unsafe {
        while length < 32768 && *pointer.add(length) != 0 {
            length += 1;
        }
        if length == 32768 {
            return Err(InstallError::BudgetExceeded);
        }
        String::from_utf16(std::slice::from_raw_parts(pointer, length))
            .map_err(|_| InstallError::StateInvalid)
    }
}
fn sid_text(sid: *mut c_void) -> Result<String> {
    let mut output = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut output) } == 0 {
        return Err(InstallError::Storage);
    }
    let _owned = Local(output.cast());
    text(output)
}
fn current_user() -> Result<String> {
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(InstallError::Storage);
    }
    struct Token(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let token = Token(token);
    let mut length = 0;
    unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut length);
    }
    if !(size_of::<TOKEN_USER>() as u32..=65536).contains(&length) {
        return Err(InstallError::StateInvalid);
    }
    let mut buffer = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err(InstallError::Storage);
    }
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    sid_text(user.User.Sid)
}
fn derive(name: &str) -> Result<String> {
    let (mut groups, mut caps) = (ptr::null_mut(), ptr::null_mut());
    let (mut ng, mut nc) = (0, 0);
    if unsafe {
        DeriveCapabilitySidsFromName(
            wide(name).as_ptr(),
            &mut groups,
            &mut ng,
            &mut caps,
            &mut nc,
        )
    } == 0
    {
        return Err(InstallError::Storage);
    }
    let result = if nc == 1 && !caps.is_null() {
        sid_text(unsafe { *caps })
    } else {
        Err(InstallError::StateInvalid)
    };
    unsafe {
        for index in 0..ng {
            LocalFree((*groups.add(index as usize)).cast());
        }
        LocalFree(groups.cast());
        for index in 0..nc {
            LocalFree((*caps.add(index as usize)).cast());
        }
        LocalFree(caps.cast());
    }
    result
}
fn descriptor(sddl: &str) -> Result<Local> {
    let mut output = ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide(sddl).as_ptr(),
            SDDL_REVISION_1,
            &mut output,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(InstallError::Storage);
    }
    Ok(Local(output))
}
fn canonical(descriptor: *mut c_void) -> Result<String> {
    if descriptor.is_null() || unsafe { IsValidSecurityDescriptor(descriptor) } == 0 {
        return Err(InstallError::StateInvalid);
    }
    let bytes = unsafe { GetSecurityDescriptorLength(descriptor) };
    if bytes == 0 || bytes > 65536 {
        return Err(InstallError::BudgetExceeded);
    }
    let mut output = ptr::null_mut();
    if unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            INFORMATION,
            &mut output,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(InstallError::Storage);
    }
    let _owned = Local(output.cast());
    text(output)
}
pub(super) struct Policy {
    file: Local,
    directory: Local,
    file_canonical: String,
    directory_canonical: String,
    private_alternative: Option<Box<Policy>>,
}
impl Policy {
    pub(super) fn new(profile_digest: &str, archive_digest: &str) -> Result<Self> {
        if !valid_digest(profile_digest) || !valid_digest(archive_digest) {
            return Err(InstallError::StateInvalid);
        }
        let capability = derive(&format!(
            "FlowTools.ToolImages.v1.{profile_digest}.{archive_digest}"
        ))?;
        Self::build(Some(capability))
    }
    pub(super) fn private() -> Result<Self> {
        Self::build(None)
    }
    pub(super) fn for_quarantine(profile: &str, archive: &str) -> Result<Self> {
        // A journal-owned quarantined group can have either exact policy while
        // an explicit abort is interrupted. Ordinary artifact reads never do.
        let mut policy = Self::new(profile, archive)?;
        policy.private_alternative = Some(Box::new(Self::private()?));
        Ok(policy)
    }
    fn build(capability: Option<String>) -> Result<Self> {
        let owner = current_user()?;
        let make = |directory| {
            let inherit = if directory { "OICI" } else { "" };
            let reader = capability
                .as_ref()
                .map(|sid| format!("(A;{inherit};FRFX;;;{sid})"))
                .unwrap_or_default();
            // Native SetSecurityInfo marks the materialized ACLs auto-inherited.
            // Require those observed bits too, while preserving the protected
            // DACL, exact owner/ACEs and explicit Medium no-write-up label.
            descriptor(&format!("O:{owner}D:PAI(A;{inherit};FA;;;{owner})(A;{inherit};FA;;;SY){reader}S:AI(ML;{inherit};NW;;;ME)"))
        };
        let file = make(false)?;
        let directory = make(true)?;
        Ok(Self {
            file_canonical: canonical(file.0)?,
            directory_canonical: canonical(directory.0)?,
            file,
            directory,
            private_alternative: None,
        })
    }
    pub(super) fn apply(&self, file: &File, directory: bool) -> Result<()> {
        let descriptor = if directory {
            self.directory.0
        } else {
            self.file.0
        };
        let (mut owner, mut dacl, mut sacl) = (ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
        let (mut defaulted, mut present) = (0, 0);
        unsafe {
            if GetSecurityDescriptorOwner(descriptor, &mut owner, &mut defaulted) == 0
                || owner.is_null()
            {
                return Err(InstallError::StateInvalid);
            }
            if GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) == 0
                || present == 0
                || dacl.is_null()
            {
                return Err(InstallError::StateInvalid);
            }
            if GetSecurityDescriptorSacl(descriptor, &mut present, &mut sacl, &mut defaulted) == 0
                || present == 0
                || sacl.is_null()
            {
                return Err(InstallError::StateInvalid);
            }
            if SetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                owner,
                ptr::null_mut(),
                dacl,
                sacl,
            ) != 0
            {
                return Err(InstallError::Storage);
            }
        }
        self.verify(file, directory)
    }
    pub(super) fn verify(&self, file: &File, directory: bool) -> Result<()> {
        let mut output = ptr::null_mut();
        if unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut output,
            )
        } != 0
        {
            return Err(InstallError::Storage);
        }
        let owned = Local(output);
        let expected = if directory {
            &self.directory_canonical
        } else {
            &self.file_canonical
        };
        let actual = canonical(owned.0)?;
        if actual != *expected {
            if let Some(private) = &self.private_alternative {
                return private.verify(file, directory);
            }
            #[cfg(test)]
            eprintln!("native image security mismatch: expected={expected}; actual={actual}");
            return Err(InstallError::ArtifactInvalid);
        }
        Ok(())
    }
}
