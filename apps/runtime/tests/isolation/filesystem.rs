//! Native, test-owned junction; no administrator or shell invocation.
use super::harness::wide;
use std::{
    fs, io,
    mem::offset_of,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
    ptr,
};
use windows_sys::{
    Wdk::Storage::FileSystem::{REPARSE_DATA_BUFFER, REPARSE_DATA_BUFFER_0_1},
    Win32::{
        Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
            FILE_SHARE_WRITE,
        },
        System::{
            Ioctl::FSCTL_SET_REPARSE_POINT, SystemServices::IO_REPARSE_TAG_MOUNT_POINT,
            IO::DeviceIoControl,
        },
    },
};
pub fn junction(path: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir(path)?;
    let directory = fs::OpenOptions::new()
        .write(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    // NT substitute names do not normalize forward slashes like Win32 paths.
    let mut name = wide(format!(
        "\\??\\{}",
        target.display().to_string().replace('/', "\\")
    ));
    let name_bytes = (name.len() - 1) * 2;
    name.push(0);
    let data_offset = offset_of!(REPARSE_DATA_BUFFER, Anonymous);
    let name_offset = data_offset + offset_of!(REPARSE_DATA_BUFFER_0_1, PathBuffer);
    let length = name_offset + name.len() * 2;
    assert!(length <= 16384);
    let mut buffer = vec![0u64; length.div_ceil(8)];
    let header = buffer.as_mut_ptr().cast::<REPARSE_DATA_BUFFER>();
    unsafe {
        (*header).ReparseTag = IO_REPARSE_TAG_MOUNT_POINT;
        (*header).ReparseDataLength = (length - data_offset) as u16;
        (*header)
            .Anonymous
            .MountPointReparseBuffer
            .SubstituteNameLength = name_bytes as u16;
        (*header).Anonymous.MountPointReparseBuffer.PrintNameOffset = (name_bytes + 2) as u16;
        ptr::copy_nonoverlapping(
            name.as_ptr(),
            ptr::addr_of_mut!((*header).Anonymous.MountPointReparseBuffer.PathBuffer).cast::<u16>(),
            name.len(),
        );
        let mut transferred = 0;
        if DeviceIoControl(
            directory.as_raw_handle(),
            FSCTL_SET_REPARSE_POINT,
            buffer.as_ptr().cast(),
            length as u32,
            ptr::null_mut(),
            0,
            &mut transferred,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
