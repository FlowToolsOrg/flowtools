//! Read-only native token evidence shared by the test launcher and child.
use std::{ffi::c_void, io, mem::size_of, os::windows::ffi::OsStrExt, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HANDLE},
    Security::{Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY},
    System::Threading::OpenProcessToken,
};
fn last() -> io::Error {
    io::Error::last_os_error()
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
pub fn token(process: HANDLE, class: i32) -> io::Result<Vec<usize>> {
    unsafe {
        let mut raw = ptr::null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut raw) == 0 {
            return Err(last());
        }
        let token = Handle(raw);
        let mut needed = 0;
        GetTokenInformation(token.0, class, ptr::null_mut(), 0, &mut needed);
        if needed == 0 || needed > 65536 {
            return Err(io::Error::other(format!(
                "invalid token buffer for {class}: {}",
                last()
            )));
        }
        let mut data = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(
            token.0,
            class,
            data.as_mut_ptr().cast(),
            needed,
            &mut needed,
        ) == 0
        {
            return Err(last());
        }
        Ok(data)
    }
}
pub fn sid_text(sid: *mut c_void) -> io::Result<String> {
    unsafe {
        let mut text = ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut text) == 0 {
            return Err(last());
        }
        let mut len = 0;
        while len < 256 && *text.add(len) != 0 {
            len += 1;
        }
        let result = String::from_utf16(std::slice::from_raw_parts(text, len))
            .map_err(|_| io::Error::other("invalid SID"));
        LocalFree(text.cast());
        result
    }
}
pub fn token_u32(process: HANDLE, class: i32) -> io::Result<u32> {
    let value = token(process, class)?;
    Ok(unsafe { *value.as_ptr().cast::<u32>() })
}
// Native token-security-attribute layout (phnt nt seapi, original ABI).
// Class 46 is not implemented by this Windows kernel; inspect the actual
// WIN://NOALLAPPPKG attribute rather than trusting requested process flags.
pub fn token_lpac(process: HANDLE) -> io::Result<bool> {
    #[repr(C)]
    struct Attributes {
        version: u16,
        reserved: u16,
        count: u32,
        entries: *const Attribute,
    }
    #[repr(C)]
    struct Attribute {
        name: windows_sys::Win32::Foundation::UNICODE_STRING,
        kind: u16,
        reserved: u16,
        flags: u32,
        count: u32,
        values: *const u64,
    }
    let buffer = token(
        process,
        windows_sys::Win32::Security::TokenSecurityAttributes,
    )?;
    let begin = buffer.as_ptr() as usize;
    let bytes = std::mem::size_of_val(buffer.as_slice());
    let inside = |pointer: usize, length: usize, align: usize| {
        pointer.is_multiple_of(align)
            && pointer >= begin
            && pointer
                .checked_add(length)
                .is_some_and(|end| end <= begin + bytes)
    };
    if bytes < size_of::<Attributes>() {
        return Err(io::Error::other("invalid token attributes"));
    }
    let header = unsafe { &*buffer.as_ptr().cast::<Attributes>() };
    if header.version != 1
        || header.reserved != 0
        || header.count > 32
        || (header.count > 0
            && !inside(
                header.entries as usize,
                header.count as usize * size_of::<Attribute>(),
                std::mem::align_of::<Attribute>(),
            ))
    {
        return Err(io::Error::other("invalid token attribute header"));
    }
    let mut found = None;
    for index in 0..header.count as usize {
        let attribute = unsafe { &*header.entries.add(index) };
        let name = &attribute.name;
        if !name.Length.is_multiple_of(2)
            || name.Length > 512
            || name.Length > name.MaximumLength
            || !inside(name.Buffer as usize, name.Length as usize, 2)
        {
            return Err(io::Error::other("invalid token attribute name"));
        }
        let text = String::from_utf16(unsafe {
            std::slice::from_raw_parts(name.Buffer, name.Length as usize / 2)
        })
        .map_err(|_| io::Error::other("invalid token attribute UTF16"))?;
        if text == "WIN://NOALLAPPPKG" {
            if found.is_some()
                || !matches!(attribute.kind, 2 | 6)
                || attribute.count != 1
                || !inside(attribute.values as usize, 8, 8)
            {
                return Err(io::Error::other("invalid LPAC attribute"));
            }
            found = Some(attribute.flags & 0x14 == 0 && unsafe { *attribute.values } == 1);
        }
    }
    Ok(found.unwrap_or(false))
}
