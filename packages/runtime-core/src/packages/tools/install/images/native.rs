//! One-component NT opens beneath a held directory. Never follows a client path,
//! reparse point, existing create target or alternate data stream.
use crate::packages::{install::InstallError, valid_path};
use std::{
    fs::File,
    mem::size_of,
    os::windows::io::{AsRawHandle, FromRawHandle},
    ptr,
};
use windows_sys::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            NtCreateFile, FILE_CREATE, FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN,
            FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
        },
    },
    Win32::{
        Foundation::{
            INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, UNICODE_STRING,
        },
        Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_DELETE,
            FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, READ_CONTROL, WRITE_DAC, WRITE_OWNER,
        },
        System::IO::IO_STATUS_BLOCK,
    },
};
type Result<T> = std::result::Result<T, InstallError>;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    File,
    Directory,
}
#[derive(Clone, Copy)]
pub(super) enum Open {
    Read,
    Parent,
    DuringMove,
    Create,
    Protect,
}
pub(super) fn info(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let mut data = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut data) } == 0 {
        return Err(InstallError::Storage);
    }
    if data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(InstallError::PathRejected);
    }
    Ok(data)
}
pub(super) fn child(parent: &File, name: &str, kind: Kind, mode: Open) -> Result<File> {
    if !valid_path(name) || name.contains('/') || name.len() > 240 {
        return Err(InstallError::PathRejected);
    }
    let before = info(parent)?;
    if before.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(InstallError::PathRejected);
    }
    let text: Vec<u16> = name.encode_utf16().collect();
    let name = UNICODE_STRING {
        Length: (text.len() * 2) as u16,
        MaximumLength: (text.len() * 2) as u16,
        Buffer: text.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &name,
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        ..Default::default()
    };
    let mut desired = FILE_GENERIC_READ | READ_CONTROL;
    if matches!(kind, Kind::Directory) {
        desired |= FILE_TRAVERSE;
    }
    if matches!(mode, Open::Create | Open::Protect) {
        desired |= WRITE_DAC | WRITE_OWNER;
        if matches!(kind, Kind::File) && matches!(mode, Open::Create) {
            desired |= FILE_GENERIC_WRITE;
        }
    }
    let sharing = match mode {
        Open::Read | Open::Protect => FILE_SHARE_READ,
        Open::Parent => FILE_SHARE_READ | FILE_SHARE_WRITE,
        Open::DuringMove => FILE_SHARE_READ | FILE_SHARE_DELETE,
        Open::Create => {
            if matches!(kind, Kind::File) {
                0
            } else {
                FILE_SHARE_READ
            }
        }
    };
    let options = FILE_OPEN_REPARSE_POINT
        | FILE_SYNCHRONOUS_IO_NONALERT
        | match kind {
            Kind::File => FILE_NON_DIRECTORY_FILE,
            Kind::Directory => FILE_DIRECTORY_FILE,
        };
    let mut handle = ptr::null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    let create = matches!(mode, Open::Create);
    let result = unsafe {
        NtCreateFile(
            &mut handle,
            desired,
            &attributes,
            &mut status,
            ptr::null(),
            0,
            sharing,
            if create { FILE_CREATE } else { FILE_OPEN },
            options,
            ptr::null(),
            0,
        )
    };
    if result != 0 || handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(InstallError::Storage);
    }
    let file = unsafe { File::from_raw_handle(handle) };
    if unsafe { status.Anonymous.Status } != 0 || status.Information != if create { 2 } else { 1 } {
        return Err(InstallError::Storage);
    }
    let after = info(parent)?;
    if before.dwVolumeSerialNumber != after.dwVolumeSerialNumber
        || before.nFileIndexHigh != after.nFileIndexHigh
        || before.nFileIndexLow != after.nFileIndexLow
    {
        return Err(InstallError::InventoryChanged);
    }
    let data = info(&file)?;
    if (data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != matches!(kind, Kind::Directory)
        || (matches!(kind, Kind::File) && data.nNumberOfLinks != 1)
    {
        return Err(InstallError::PathRejected);
    }
    Ok(file)
}
