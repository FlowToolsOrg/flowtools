//! Same-volume create-only moves between held T0 directory handles.
use super::{information, InstallError};
use std::{
    fs::File,
    mem::{align_of, size_of},
    os::windows::io::{AsRawHandle, FromRawHandle},
    ptr,
};
use windows_sys::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FileRenameInformation, NtCreateFile, NtSetInformationFile, FILE_DIRECTORY_FILE,
            FILE_OPEN, FILE_OPEN_REPARSE_POINT, FILE_RENAME_INFORMATION,
            FILE_SYNCHRONOUS_IO_NONALERT,
        },
    },
    Win32::{
        Foundation::{
            INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, UNICODE_STRING,
        },
        Storage::FileSystem::{
            DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ, SYNCHRONIZE,
        },
        System::IO::IO_STATUS_BLOCK,
    },
};

fn component(value: &str) -> Result<Vec<u16>, InstallError> {
    if value.is_empty()
        || value.len() > 80
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(InstallError::PathRejected);
    }
    Ok(value.encode_utf16().collect())
}

fn source(parent: &File, name: &[u16]) -> Result<File, InstallError> {
    let name = UNICODE_STRING {
        Length: (name.len() * 2) as u16,
        MaximumLength: (name.len() * 2) as u16,
        Buffer: name.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &name,
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        ..Default::default()
    };
    let mut handle = ptr::null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    // OPEN, never CREATE. Native lookup is one component beneath a held parent;
    // do not inherit the handle or follow a reparse point, including the leaf.
    let result = unsafe {
        NtCreateFile(
            &mut handle,
            DELETE | FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            &attributes,
            &mut status,
            ptr::null(),
            0,
            FILE_SHARE_READ,
            FILE_OPEN,
            FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            ptr::null(),
            0,
        )
    };
    if result != 0 || handle.is_null() || handle == INVALID_HANDLE_VALUE {
        #[cfg(test)]
        eprintln!("held source open NTSTATUS={result:#x}");
        return Err(InstallError::Storage);
    }
    // A successful synchronous open transfers exactly one owned handle to File.
    let file = unsafe { File::from_raw_handle(handle) };
    if unsafe { status.Anonymous.Status } != 0 {
        return Err(InstallError::Storage);
    }
    if information(&file)?.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(InstallError::PathRejected);
    }
    Ok(file)
}

pub(super) fn move_directory(
    source_parent: &File,
    source_name: &str,
    destination_parent: &File,
    destination_name: &str,
    validate_held: impl FnOnce() -> Result<(), InstallError>,
) -> Result<(), InstallError> {
    let source_name = component(source_name)?;
    let destination_name = component(destination_name)?;
    for parent in [source_parent, destination_parent] {
        if information(parent)?.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(InstallError::PathRejected);
        }
    }
    let source = source(source_parent, &source_name)?;
    if information(&source)?.dwVolumeSerialNumber
        != information(destination_parent)?.dwVolumeSerialNumber
    {
        return Err(InstallError::PathRejected);
    }
    // Content validation runs with the exact source directory held against
    // replacement. Neither parent guard is dropped for the native move.
    validate_held()?;
    for parent in [source_parent, destination_parent] {
        information(parent)?;
    }
    let length = size_of::<FILE_RENAME_INFORMATION>() + destination_name.len() * 2;
    assert!(align_of::<FILE_RENAME_INFORMATION>() <= align_of::<u64>());
    let mut buffer = vec![0u64; length.div_ceil(size_of::<u64>())];
    let header = buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
    let mut status = IO_STATUS_BLOCK::default();
    let result = unsafe {
        (*header).RootDirectory = destination_parent.as_raw_handle();
        (*header).FileNameLength = (destination_name.len() * 2) as u32;
        // The zeroed ReplaceIfExists field remains false. Use the generated
        // Windows ABI layout and aligned, bounded storage for its trailing name.
        ptr::copy_nonoverlapping(
            destination_name.as_ptr(),
            ptr::addr_of_mut!((*header).FileName).cast::<u16>(),
            destination_name.len(),
        );
        NtSetInformationFile(
            source.as_raw_handle(),
            &mut status,
            buffer.as_ptr().cast(),
            length as u32,
            FileRenameInformation,
        )
    };
    // No asynchronous lifetime, POSIX replacement, copy, pathname fallback or
    // retry. Any unsupported filesystem/status refuses and retains the journal.
    if result != 0 || unsafe { status.Anonymous.Status } != 0 {
        #[cfg(test)]
        eprintln!("held directory rename NTSTATUS={result:#x}");
        return Err(InstallError::Storage);
    }
    for parent in [source_parent, destination_parent] {
        information(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, os::windows::fs::MetadataExt, path::PathBuf, process::Command};

    struct Fixture {
        root: PathBuf,
        left: PathBuf,
        right: PathBuf,
        name: String,
        target: String,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "flowtools-validation-held-move-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir(&root).unwrap();
            let left = root.join("staging");
            let right = root.join("artifacts");
            std::fs::create_dir(&left).unwrap();
            std::fs::create_dir(&right).unwrap();
            let name = uuid::Uuid::new_v4().to_string();
            let target = "a".repeat(64);
            Self {
                root,
                left,
                right,
                name,
                target,
            }
        }
        fn source(&self) -> PathBuf {
            self.left.join(&self.name)
        }
        fn destination(&self) -> PathBuf {
            self.right.join(&self.target)
        }
        fn prepare_source(&self) {
            std::fs::create_dir(self.source()).unwrap();
            std::fs::write(self.source().join("canary"), b"UNCHANGED").unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            assert_eq!(self.root.parent().unwrap(), std::env::temp_dir());
            let name = self.root.file_name().unwrap().to_str().unwrap();
            let id = name
                .strip_prefix("flowtools-validation-held-move-")
                .unwrap();
            assert_eq!(uuid::Uuid::parse_str(id).unwrap().to_string(), id);
            fn clean(path: &std::path::Path) {
                let meta = std::fs::symlink_metadata(path).unwrap();
                if meta.file_attributes() & super::super::FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                    std::fs::remove_dir(path).unwrap();
                } else if meta.is_dir() {
                    for entry in std::fs::read_dir(path).unwrap() {
                        clean(&entry.unwrap().path());
                    }
                    std::fs::remove_dir(path).unwrap();
                } else {
                    std::fs::remove_file(path).unwrap();
                }
            }
            clean(&self.root);
        }
    }

    #[test]
    fn native_move_retains_both_parent_guards_and_pins_the_source() {
        let fixture = Fixture::new();
        fixture.prepare_source();
        let source = fixture.source();
        {
            let left_guard = super::super::parent_directory(&fixture.left).unwrap();
            let right_guard = super::super::parent_directory(&fixture.right).unwrap();
            move_directory(
                &left_guard,
                &fixture.name,
                &right_guard,
                &fixture.target,
                || {
                    for path in [&fixture.left, &fixture.right, &source] {
                        assert_eq!(
                            std::fs::rename(path, fixture.root.join("replaced"))
                                .unwrap_err()
                                .raw_os_error(),
                            Some(32)
                        );
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert!(!source.exists());
            assert_eq!(
                std::fs::read(fixture.destination().join("canary")).unwrap(),
                b"UNCHANGED"
            );
            assert!(std::fs::rename(&fixture.right, fixture.root.join("replaced")).is_err());
        }
    }

    #[test]
    fn existing_directory_or_file_is_never_replaced_on_native_move_failure() {
        for is_directory in [true, false] {
            let fixture = Fixture::new();
            fixture.prepare_source();
            let target = if is_directory {
                std::fs::create_dir(fixture.destination()).unwrap();
                fixture.destination().join("keep")
            } else {
                fixture.destination()
            };
            std::fs::write(&target, b"KEEP").unwrap();
            let left = super::super::parent_directory(&fixture.left).unwrap();
            let right = super::super::parent_directory(&fixture.right).unwrap();
            assert_eq!(
                move_directory(&left, &fixture.name, &right, &fixture.target, || Ok(())),
                Err(InstallError::Storage)
            );
            assert_eq!(std::fs::read(target).unwrap(), b"KEEP");
            assert_eq!(
                std::fs::read(fixture.source().join("canary")).unwrap(),
                b"UNCHANGED"
            );
            assert!(std::fs::rename(&fixture.right, fixture.root.join("replaced")).is_err());
        }
    }

    #[test]
    fn source_junction_is_refused_before_validation_without_touching_its_target() {
        let fixture = Fixture::new();
        let outside = fixture.root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("canary"), b"UNCHANGED").unwrap();
        let command = Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(fixture.source())
            .arg(&outside)
            .output()
            .unwrap();
        assert!(command.status.success());
        let left = super::super::parent_directory(&fixture.left).unwrap();
        let right = super::super::parent_directory(&fixture.right).unwrap();
        let called = Cell::new(false);
        assert!(
            move_directory(&left, &fixture.name, &right, &fixture.target, || {
                called.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!called.get());
        assert!(!fixture.destination().exists());
        assert_eq!(std::fs::read(outside.join("canary")).unwrap(), b"UNCHANGED");
    }

    #[test]
    fn escaping_names_and_failed_held_validation_never_publish() {
        let fixture = Fixture::new();
        fixture.prepare_source();
        let left = super::super::parent_directory(&fixture.left).unwrap();
        let right = super::super::parent_directory(&fixture.right).unwrap();
        for hostile in [
            "",
            "../outside",
            "a/b",
            "a\\b",
            "c:stream",
            "A",
            "a\0b",
            &"a".repeat(81),
        ] {
            let called = Cell::new(false);
            for (source, destination) in [
                (hostile, fixture.target.as_str()),
                (fixture.name.as_str(), hostile),
            ] {
                assert_eq!(
                    move_directory(&left, source, &right, destination, || {
                        called.set(true);
                        Ok(())
                    }),
                    Err(InstallError::PathRejected)
                );
            }
            assert!(!called.get());
        }
        assert_eq!(
            move_directory(&left, &fixture.name, &right, &fixture.target, || Err(
                InstallError::ArtifactInvalid
            )),
            Err(InstallError::ArtifactInvalid)
        );
        assert!(!fixture.destination().exists());
        assert_eq!(
            std::fs::read(fixture.source().join("canary")).unwrap(),
            b"UNCHANGED"
        );
    }

    #[test]
    fn parent_changed_to_reparse_during_held_validation_refuses_publication() {
        use std::{fs::OpenOptions, mem::offset_of, os::windows::fs::OpenOptionsExt};
        use windows_sys::{
            Wdk::Storage::FileSystem::{REPARSE_DATA_BUFFER, REPARSE_DATA_BUFFER_0_1},
            Win32::{
                Storage::FileSystem::{
                    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_WRITE,
                },
                System::{
                    Ioctl::{FSCTL_DELETE_REPARSE_POINT, FSCTL_SET_REPARSE_POINT},
                    SystemServices::IO_REPARSE_TAG_MOUNT_POINT,
                    IO::DeviceIoControl,
                },
            },
        };
        let fixture = Fixture::new();
        fixture.prepare_source();
        let outside = fixture.root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("canary"), b"UNCHANGED").unwrap();
        let left = super::super::parent_directory(&fixture.left).unwrap();
        let right = super::super::parent_directory(&fixture.right).unwrap();
        let writer = OpenOptions::new()
            .write(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&fixture.right)
            .unwrap();
        let name: Vec<_> = format!("\\??\\{}", outside.display())
            .encode_utf16()
            .collect();
        let bytes = name.len() * 2;
        let mut terminated = name;
        terminated.extend([0, 0]);
        let data_offset = offset_of!(REPARSE_DATA_BUFFER, Anonymous);
        let name_offset = data_offset + offset_of!(REPARSE_DATA_BUFFER_0_1, PathBuffer);
        let length = name_offset + terminated.len() * 2;
        let mut buffer = vec![0u64; length.div_ceil(8)];
        let header = buffer.as_mut_ptr().cast::<REPARSE_DATA_BUFFER>();
        unsafe {
            (*header).ReparseTag = IO_REPARSE_TAG_MOUNT_POINT;
            (*header).ReparseDataLength = (length - data_offset) as u16;
            (*header)
                .Anonymous
                .MountPointReparseBuffer
                .SubstituteNameLength = bytes as u16;
            (*header).Anonymous.MountPointReparseBuffer.PrintNameOffset = (bytes + 2) as u16;
            ptr::copy_nonoverlapping(
                terminated.as_ptr(),
                ptr::addr_of_mut!((*header).Anonymous.MountPointReparseBuffer.PathBuffer)
                    .cast::<u16>(),
                terminated.len(),
            );
        }
        let mut transferred = 0;
        let result = move_directory(&left, &fixture.name, &right, &fixture.target, || {
            assert_ne!(
                unsafe {
                    DeviceIoControl(
                        writer.as_raw_handle(),
                        FSCTL_SET_REPARSE_POINT,
                        buffer.as_ptr().cast(),
                        length as u32,
                        ptr::null_mut(),
                        0,
                        &mut transferred,
                        ptr::null_mut(),
                    )
                },
                0,
                "{}",
                std::io::Error::last_os_error()
            );
            Ok(())
        });
        let tag = IO_REPARSE_TAG_MOUNT_POINT as u64;
        assert_ne!(
            unsafe {
                DeviceIoControl(
                    writer.as_raw_handle(),
                    FSCTL_DELETE_REPARSE_POINT,
                    ptr::addr_of!(tag).cast(),
                    data_offset as u32,
                    ptr::null_mut(),
                    0,
                    &mut transferred,
                    ptr::null_mut(),
                )
            },
            0
        );
        assert_eq!(result, Err(InstallError::PathRejected));
        assert!(!fixture.destination().exists());
        assert_eq!(
            std::fs::read(fixture.source().join("canary")).unwrap(),
            b"UNCHANGED"
        );
        assert_eq!(std::fs::read(outside.join("canary")).unwrap(), b"UNCHANGED");
    }
}
