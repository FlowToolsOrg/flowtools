//! User-selected NTFS directories. Every subsequent lookup is handle-relative.
use super::{FilePin, RootPin, MAX_ENTRIES, MAX_FILE_BYTES};
use crate::{catalog::digest, protocol::ErrorCode};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    mem::{offset_of, size_of},
    os::windows::{
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::{Component, Path, Prefix},
    ptr,
};
use windows_sys::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            NtCreateFile, FILE_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
            FILE_SYNCHRONOUS_IO_NONALERT,
        },
    },
    Win32::{
        Foundation::{
            GetLastError, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE,
            OBJ_DONT_REPARSE, UNICODE_STRING,
        },
        Storage::FileSystem::{
            FileCaseSensitiveInfo, FileIdBothDirectoryInfo, FileIdBothDirectoryRestartInfo,
            GetFileInformationByHandle, GetFileInformationByHandleEx, GetFinalPathNameByHandleW,
            GetVolumeInformationByHandleW, BY_HANDLE_FILE_INFORMATION, DELETE,
            FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_CASE_SENSITIVE_INFO,
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_BOTH_DIR_INFO,
            FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_READ_DATA, FILE_SHARE_READ,
            FILE_SHARE_WRITE, SYNCHRONIZE, VOLUME_NAME_GUID,
        },
        System::IO::IO_STATUS_BLOCK,
    },
};
type Result<T> = std::result::Result<T, ErrorCode>;

fn nt_error(status: i32) -> ErrorCode {
    match status as u32 {
        0xc0000035 => ErrorCode::RevisionConflict, // Object name collision.
        0xc0000043 => ErrorCode::RuntimeBusy,      // An existing writer/deleter.
        0xc0000022 | 0xc000050b | 0xc0000275 => ErrorCode::ScopeDenied,
        _ => ErrorCode::StorageFailed,
    }
}
pub(super) fn component(value: &str) -> Result<Vec<u16>> {
    let units: Vec<u16> = value.encode_utf16().collect();
    if units.is_empty()
        || units.len() > 255
        || value == "."
        || value == ".."
        || value.ends_with(['.', ' '])
        || value.chars().any(|c| {
            c.is_control()
                || "\\/:*?\"<>|".contains(c)
                || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
    {
        return Err(ErrorCode::InputInvalid);
    }
    let base = value.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(
        base.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || (base.len() == 4
        && (base.starts_with("COM") || base.starts_with("LPT"))
        && matches!(base.as_bytes()[3], b'1'..=b'9'))
        || ["COM", "LPT"].iter().any(|prefix| {
            base.strip_prefix(prefix)
                .is_some_and(|suffix| matches!(suffix, "¹" | "²" | "³"))
        })
    {
        return Err(ErrorCode::InputInvalid);
    }
    Ok(units)
}
fn information(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(ErrorCode::StorageFailed);
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(ErrorCode::ScopeDenied);
    }
    Ok(info)
}
fn file_id(info: &BY_HANDLE_FILE_INFORMATION) -> String {
    format!("{:08x}{:08x}", info.nFileIndexHigh, info.nFileIndexLow)
}
fn time(high: u32, low: u32) -> String {
    format!("{high:08x}{low:08x}")
}
fn canonical(file: &File) -> Result<String> {
    let mut buffer = vec![0u16; 32768];
    let len = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            VOLUME_NAME_GUID,
        )
    } as usize;
    if len == 0 || len >= buffer.len() {
        return Err(ErrorCode::StorageFailed);
    }
    let path = String::from_utf16(&buffer[..len]).map_err(|_| ErrorCode::ScopeDenied)?;
    if !path.starts_with(r"\\?\Volume{") {
        return Err(ErrorCode::ScopeDenied);
    }
    Ok(path)
}
fn root_pin(file: &File) -> Result<RootPin> {
    let info = information(file)?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(ErrorCode::ScopeDenied);
    }
    let canonical = canonical(file)?;
    let end = canonical.find('}').ok_or(ErrorCode::ScopeDenied)?;
    Ok(RootPin {
        volume_id: digest(&serde_json::json!(canonical[..=end].to_ascii_lowercase())),
        file_id: file_id(&info),
        path_digest: digest(&serde_json::json!(canonical)),
    })
}
/// OPEN only, one component below a held parent, never following a reparse.
fn relative(
    parent: &File,
    name: &str,
    directory: Option<bool>,
    access: u32,
    sharing: u32,
) -> Result<Option<File>> {
    let units = component(name)?;
    let name = UNICODE_STRING {
        Length: (units.len() * 2) as u16,
        MaximumLength: (units.len() * 2) as u16,
        Buffer: units.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &name,
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        ..Default::default()
    };
    let mut handle = ptr::null_mut();
    let mut io = IO_STATUS_BLOCK::default();
    let kind = match directory {
        Some(true) => FILE_DIRECTORY_FILE,
        Some(false) => windows_sys::Wdk::Storage::FileSystem::FILE_NON_DIRECTORY_FILE,
        None => 0,
    };
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            &attributes,
            &mut io,
            ptr::null(),
            0,
            sharing,
            FILE_OPEN,
            kind | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            ptr::null(),
            0,
        )
    };
    if status as u32 == 0xc0000034 {
        return Ok(None);
    }
    if status != 0 {
        return Err(nt_error(status));
    }
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(ErrorCode::StorageFailed);
    }
    let file = unsafe { File::from_raw_handle(handle) };
    if unsafe { io.Anonymous.Status } != 0 {
        return Err(ErrorCode::StorageFailed);
    }
    information(&file)?;
    Ok(Some(file))
}

pub(super) struct Directory {
    guards: Vec<File>,
    pub pin: RootPin,
}
pub(super) struct HeldSource {
    file: File,
    pub pin: FilePin,
}
impl Directory {
    fn root(&self) -> &File {
        self.guards.last().expect("an admitted directory")
    }
    pub fn open(path: &Path, protected: Option<&Path>) -> Result<Self> {
        if !cfg!(target_arch = "x86_64") {
            return Err(ErrorCode::ScopeDenied);
        }
        let mut parts = path.components();
        let drive = match parts.next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
                _ => return Err(ErrorCode::ScopeDenied),
            },
            _ => return Err(ErrorCode::InputInvalid),
        };
        if !matches!(parts.next(), Some(Component::RootDir)) {
            return Err(ErrorCode::InputInvalid);
        }
        let names: Vec<String> = parts
            .map(|part| match part {
                Component::Normal(name) => name
                    .to_str()
                    .map(str::to_owned)
                    .ok_or(ErrorCode::ScopeDenied),
                _ => Err(ErrorCode::InputInvalid),
            })
            .collect::<Result<_>>()?;
        if names.is_empty() || names.len() > 64 {
            return Err(ErrorCode::ScopeDenied);
        }
        let drive_path = format!("{}:\\", char::from(drive));
        let root = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(drive_path)
            .map_err(|_| ErrorCode::StorageFailed)?;
        information(&root)?;
        let mut guards = vec![root];
        for (index, name) in names.iter().enumerate() {
            let sharing = if index + 1 == names.len() {
                FILE_SHARE_READ | FILE_SHARE_WRITE
            } else {
                FILE_SHARE_READ
            };
            let directory = relative(
                guards.last().unwrap(),
                name,
                Some(true),
                FILE_LIST_DIRECTORY,
                sharing,
            )?
            .ok_or(ErrorCode::RevisionConflict)?;
            guards.push(directory);
        }
        let root = guards.last().unwrap();
        let mut fs_name = [0u16; 32];
        if unsafe {
            GetVolumeInformationByHandleW(
                root.as_raw_handle(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                fs_name.as_mut_ptr(),
                fs_name.len() as u32,
            )
        } == 0
        {
            return Err(ErrorCode::ScopeDenied);
        }
        let length = fs_name
            .iter()
            .position(|unit| *unit == 0)
            .ok_or(ErrorCode::ScopeDenied)?;
        if String::from_utf16(&fs_name[..length]).map_err(|_| ErrorCode::ScopeDenied)? != "NTFS" {
            return Err(ErrorCode::ScopeDenied);
        }
        let mut case = FILE_CASE_SENSITIVE_INFO::default();
        if unsafe {
            GetFileInformationByHandleEx(
                root.as_raw_handle(),
                FileCaseSensitiveInfo,
                (&mut case as *mut FILE_CASE_SENSITIVE_INFO).cast(),
                size_of::<FILE_CASE_SENSITIVE_INFO>() as u32,
            )
        } == 0
            || case.Flags != 0
        {
            return Err(ErrorCode::ScopeDenied);
        }
        let result = Self {
            pin: root_pin(root)?,
            guards,
        };
        if let Some(protected) = protected {
            let protected = Self::open(protected, None)?;
            for guard in &result.guards {
                let pin = root_pin(guard)?;
                if pin.volume_id == protected.pin.volume_id && pin.file_id == protected.pin.file_id
                {
                    return Err(ErrorCode::ScopeDenied);
                }
            }
        }
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<()> {
        for guard in &self.guards {
            information(guard)?;
        }
        if root_pin(self.root())? != self.pin {
            return Err(ErrorCode::RevisionConflict);
        }
        Ok(())
    }
    pub fn entries(&self) -> Result<Vec<String>> {
        self.validate()?;
        let mut result = Vec::new();
        let mut restart = true;
        let mut count = 0usize;
        let mut buffer = vec![0u64; 8192];
        loop {
            buffer.fill(0);
            let class = if restart {
                FileIdBothDirectoryRestartInfo
            } else {
                FileIdBothDirectoryInfo
            };
            let success = unsafe {
                GetFileInformationByHandleEx(
                    self.root().as_raw_handle(),
                    class,
                    buffer.as_mut_ptr().cast(),
                    (buffer.len() * 8) as u32,
                )
            };
            if success == 0 {
                if unsafe { GetLastError() } == ERROR_NO_MORE_FILES {
                    break;
                }
                return Err(ErrorCode::StorageFailed);
            }
            restart = false;
            let mut offset = 0usize;
            loop {
                if offset + size_of::<FILE_ID_BOTH_DIR_INFO>() > buffer.len() * 8
                    || !offset.is_multiple_of(8)
                {
                    return Err(ErrorCode::StorageFailed);
                }
                let entry = unsafe {
                    &*buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset)
                        .cast::<FILE_ID_BOTH_DIR_INFO>()
                };
                let length = entry.FileNameLength as usize;
                let name_offset = offset + offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
                if !length.is_multiple_of(2)
                    || length > 510
                    || name_offset + length > buffer.len() * 8
                {
                    return Err(ErrorCode::ScopeDenied);
                }
                let units = unsafe {
                    std::slice::from_raw_parts(
                        buffer.as_ptr().cast::<u8>().add(name_offset).cast::<u16>(),
                        length / 2,
                    )
                };
                let name = String::from_utf16(units).map_err(|_| ErrorCode::ScopeDenied)?;
                if name != "." && name != ".." {
                    if entry.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                        return Err(ErrorCode::ScopeDenied);
                    }
                    count += 1;
                    if count > MAX_ENTRIES {
                        return Err(ErrorCode::BudgetExceeded);
                    }
                    if entry.FileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
                        component(&name)?;
                        result.push(name);
                    }
                }
                if entry.NextEntryOffset == 0 {
                    break;
                }
                if (entry.NextEntryOffset as usize)
                    < offset_of!(FILE_ID_BOTH_DIR_INFO, FileName) + length
                {
                    return Err(ErrorCode::StorageFailed);
                }
                offset = offset
                    .checked_add(entry.NextEntryOffset as usize)
                    .ok_or(ErrorCode::StorageFailed)?;
            }
        }
        self.validate()?;
        result.sort();
        if result.windows(2).any(|names| names[0] == names[1]) {
            return Err(ErrorCode::RevisionConflict);
        }
        Ok(result)
    }
    pub fn source(&self, name: &str) -> Result<Option<HeldSource>> {
        self.validate()?;
        let Some(mut file) = relative(
            self.root(),
            name,
            None,
            DELETE | FILE_READ_DATA,
            FILE_SHARE_READ,
        )?
        else {
            return Ok(None);
        };
        let before = information(&file)?;
        if before.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
            return Ok(None);
        }
        if before.nNumberOfLinks != 1 {
            return Err(ErrorCode::ScopeDenied);
        }
        let size = (u64::from(before.nFileSizeHigh) << 32) | u64::from(before.nFileSizeLow);
        if size > MAX_FILE_BYTES {
            return Err(ErrorCode::BudgetExceeded);
        }
        let mut hasher = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|_| ErrorCode::StorageFailed)?;
            if read == 0 {
                break;
            };
            bytes += read as u64;
            if bytes > MAX_FILE_BYTES {
                return Err(ErrorCode::BudgetExceeded);
            };
            hasher.update(&buffer[..read]);
        }
        let pin = FilePin {
            volume_id: self.pin.volume_id.clone(),
            file_id: file_id(&before),
            size,
            modified: time(
                before.ftLastWriteTime.dwHighDateTime,
                before.ftLastWriteTime.dwLowDateTime,
            ),
            created: time(
                before.ftCreationTime.dwHighDateTime,
                before.ftCreationTime.dwLowDateTime,
            ),
            sha256: format!("{:x}", hasher.finalize()),
        };
        let source = HeldSource { file, pin };
        if bytes != size {
            return Err(ErrorCode::RevisionConflict);
        }
        source.validate()?;
        self.validate()?;
        Ok(Some(source))
    }
    pub fn absent(&self, name: &str) -> Result<bool> {
        self.validate()?;
        Ok(relative(
            self.root(),
            name,
            None,
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
        )?
        .is_none())
    }
    #[cfg(test)]
    pub fn rename(&self, source: &HeldSource, target: &str) -> Result<FilePin> {
        use windows_sys::Wdk::Storage::FileSystem::{
            FileRenameInformation, NtSetInformationFile, FILE_RENAME_INFORMATION,
        };
        const _: () =
            assert!(std::mem::align_of::<FILE_RENAME_INFORMATION>() <= std::mem::align_of::<u64>());
        let target = component(target)?;
        self.validate()?;
        source.validate()?;
        if source.pin.volume_id != self.pin.volume_id {
            return Err(ErrorCode::ScopeDenied);
        }
        let length = size_of::<FILE_RENAME_INFORMATION>() + target.len() * 2;
        let mut buffer = vec![0u64; length.div_ceil(8)];
        let header = buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
        let mut io = IO_STATUS_BLOCK::default();
        let status = unsafe {
            (*header).RootDirectory = self.root().as_raw_handle();
            (*header).FileNameLength = (target.len() * 2) as u32;
            ptr::copy_nonoverlapping(
                target.as_ptr(),
                ptr::addr_of_mut!((*header).FileName).cast::<u16>(),
                target.len(),
            );
            NtSetInformationFile(
                source.file.as_raw_handle(),
                &mut io,
                buffer.as_ptr().cast(),
                length as u32,
                FileRenameInformation,
            )
        };
        if status != 0 {
            return Err(nt_error(status));
        }
        if unsafe { io.Anonymous.Status } != 0 {
            return Err(ErrorCode::StorageFailed);
        }
        self.validate()?;
        let after = source.observed()?;
        // NTFS name tunneling may inherit the creation time of a recently
        // deleted target. Keep every other identity/content field exact and
        // return the observed creation time for a future undo/retry receipt.
        let mut expected = source.pin.clone();
        expected.created = after.created.clone();
        if after != expected {
            return Err(ErrorCode::RevisionConflict);
        }
        Ok(after)
    }
}
impl HeldSource {
    pub fn validate(&self) -> Result<()> {
        if self.observed()? != self.pin {
            return Err(ErrorCode::RevisionConflict);
        }
        Ok(())
    }
    fn observed(&self) -> Result<FilePin> {
        let info = information(&self.file)?;
        let size = (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow);
        if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0 || info.nNumberOfLinks != 1 {
            return Err(ErrorCode::RevisionConflict);
        }
        Ok(FilePin {
            volume_id: self.pin.volume_id.clone(),
            file_id: file_id(&info),
            size,
            modified: time(
                info.ftLastWriteTime.dwHighDateTime,
                info.ftLastWriteTime.dwLowDateTime,
            ),
            created: time(
                info.ftCreationTime.dwHighDateTime,
                info.ftCreationTime.dwLowDateTime,
            ),
            sha256: self.pin.sha256.clone(),
        })
    }
}
