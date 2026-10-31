//! Fixture creation only: select the T0 owner without replacing inherited ACLs.
use super::*;
use std::{ffi::c_void, ptr};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::{
            ConvertSecurityDescriptorToStringSecurityDescriptorW,
            ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
            SetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
        },
        GetSecurityDescriptorOwner, DACL_SECURITY_INFORMATION, LABEL_SECURITY_INFORMATION,
        OWNER_SECURITY_INFORMATION,
    },
};
struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0) };
    }
}
fn snapshot(path: &Path) -> (String, String) {
    let (mut descriptor, mut owner) = (ptr::null_mut(), ptr::null_mut());
    let policy = DACL_SECURITY_INFORMATION | LABEL_SECURITY_INFORMATION;
    assert_eq!(
        unsafe {
            GetNamedSecurityInfoW(
                harness::wide(path).as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | policy,
                &mut owner,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut descriptor,
            )
        },
        0
    );
    let descriptor = Local(descriptor);
    let owner = harness::sid_text(owner).unwrap();
    let (mut output, mut length) = (ptr::null_mut(), 0);
    assert_ne!(
        unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor.0,
                SDDL_REVISION_1,
                policy,
                &mut output,
                &mut length,
            )
        },
        0
    );
    let _output = Local(output.cast());
    assert!((1..=32768).contains(&length));
    let policy = unsafe { std::slice::from_raw_parts(output, length as usize - 1) };
    (owner, String::from_utf16(policy).unwrap())
}
pub(super) fn assign_current_user(path: &Path) {
    let user = harness::user_sid().unwrap();
    let before = snapshot(path);
    let mut descriptor = ptr::null_mut();
    assert_ne!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                harness::wide(format!("O:{user}")).as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            )
        },
        0
    );
    let descriptor = Local(descriptor);
    let (mut owner, mut defaulted) = (ptr::null_mut(), 0);
    assert_ne!(
        unsafe { GetSecurityDescriptorOwner(descriptor.0, &mut owner, &mut defaulted) },
        0
    );
    assert_eq!(
        unsafe {
            SetNamedSecurityInfoW(
                harness::wide(path).as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                owner,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        },
        0
    );
    let after = snapshot(path);
    assert_eq!(after.0, user, "fixture owner readback: {path:?}");
    assert_eq!(
        before.1, after.1,
        "owner-only write changed ACL/label: {path:?}"
    );
}
