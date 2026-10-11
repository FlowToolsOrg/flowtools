//! Fixed inert blob names, create-only writes and handle-held directory guards.
use super::{InstallError, Journal, Record};
use crate::data::{safe_path, DataStore};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileInformationByHandle, MoveFileExW, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, MOVEFILE_WRITE_THROUGH,
};

pub(super) const MAX_DISK_BYTES: u64 = 268_435_456;
const MAX_GROUPS: usize = 384;
const NAMES: [&str; 3] = ["manifest.json", "package.zip", "seal.json"];
fn seal(record: &Record) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"formatVersion":1,"artifactId":record.artifact_id,"zipDigest":record.payload.archive.sha256,"manifestDigest":record.payload.manifest.sha256})).expect("bounded seal")
}
pub(super) struct Layout {
    pub root: PathBuf,
    _guards: Vec<File>,
}
fn information(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION, InstallError> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(InstallError::Storage);
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(InstallError::PathRejected);
    }
    Ok(info)
}
fn directory(path: &Path) -> Result<File, InstallError> {
    safe_path(path).map_err(|_| InstallError::PathRejected)?;
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| InstallError::Storage)?;
    if information(&file)?.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(InstallError::PathRejected);
    }
    Ok(file)
}
fn regular(path: &Path, limit: u64, readonly: bool) -> Result<File, InstallError> {
    safe_path(path).map_err(|_| InstallError::PathRejected)?;
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| InstallError::Storage)?;
    let info = information(&file)?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0
        || info.nNumberOfLinks != 1
        || (readonly && info.dwFileAttributes & FILE_ATTRIBUTE_READONLY == 0)
        || file.metadata().map_err(|_| InstallError::Storage)?.len() > limit
    {
        return Err(InstallError::PathRejected);
    }
    Ok(file)
}
fn exists(path: &Path) -> Result<bool, InstallError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(InstallError::Storage),
    }
}
impl Layout {
    // The native Host owns/reserves/protects this profile before entry. Directory
    // handles exclude write/delete sharing during this operation. No client path.
    pub fn open(store: &DataStore, create: bool) -> Result<Option<Self>, InstallError> {
        if std::env::consts::ARCH != "x86_64" {
            return Err(InstallError::Unsupported);
        }
        let profile = store.profile().ok_or(InstallError::ProfileRequired)?;
        let mut guards = vec![directory(profile)?];
        let root = profile.join("package-store-v1");
        if !exists(&root)? {
            if !create {
                return Ok(None);
            }
            std::fs::create_dir(&root).map_err(|_| InstallError::Storage)?;
        }
        guards.push(directory(&root)?);
        for name in ["artifacts", "staging", "quarantine"] {
            let path = root.join(name);
            if !exists(&path)? {
                if !create {
                    continue;
                }
                std::fs::create_dir(&path).map_err(|_| InstallError::Storage)?;
            }
            guards.push(directory(&path)?);
        }
        Ok(Some(Self {
            root,
            _guards: guards,
        }))
    }
    pub fn artifact(&self, digest: &str) -> PathBuf {
        self.root.join("artifacts").join(digest)
    }
    pub fn stage(&self, id: &str) -> PathBuf {
        self.root.join("staging").join(id)
    }
    pub fn assert_owned(
        &self,
        records: &[Record],
        journal: Option<&Journal>,
    ) -> Result<(), InstallError> {
        let mut groups = 0;
        for kind in ["artifacts", "staging", "quarantine"] {
            let path = self.root.join(kind);
            if !exists(&path)? {
                continue;
            }
            for entry in std::fs::read_dir(path).map_err(|_| InstallError::Storage)? {
                groups += 1;
                if groups > MAX_GROUPS {
                    return Err(InstallError::BudgetExceeded);
                }
                let entry = entry.map_err(|_| InstallError::Storage)?;
                let name = entry.file_name();
                let name = name.to_str().ok_or(InstallError::PathRejected)?;
                let owned = match kind {
                    "artifacts" => {
                        super::super::valid_digest(name)
                            && (records.iter().any(|r| r.payload.archive.sha256 == name)
                                || journal.is_some_and(|j| {
                                    j.new_artifact
                                        && j.prepared
                                        && j.record.payload.archive.sha256 == name
                                }))
                    }
                    "staging" => journal.is_some_and(|j| j.new_artifact && j.id == name),
                    _ => ["-stage", "-artifact"].iter().any(|suffix| {
                        name.strip_suffix(suffix).is_some_and(|id| {
                            uuid::Uuid::parse_str(id).is_ok_and(|v| v.to_string() == id)
                        })
                    }),
                };
                if !owned {
                    return Err(InstallError::StateInvalid);
                }
            }
        }
        Ok(())
    }
    pub fn has_artifact(&self, record: &Record) -> Result<bool, InstallError> {
        exists(&self.artifact(&record.payload.archive.sha256))
    }
    pub fn read(&self, record: &Record) -> Result<(Vec<u8>, Vec<u8>), InstallError> {
        self.read_at(&self.artifact(&record.payload.archive.sha256), record)
    }
    fn read_at(&self, path: &Path, record: &Record) -> Result<(Vec<u8>, Vec<u8>), InstallError> {
        let _guard = directory(path)?;
        let mut result = Vec::new();
        for (name, expected) in [
            ("manifest.json", &record.payload.manifest),
            (
                "package.zip",
                &super::super::BlobDescriptor {
                    size: record.payload.archive.size,
                    sha256: record.payload.archive.sha256.clone(),
                },
            ),
        ] {
            let mut file = regular(&path.join(name), expected.size, true)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|_| InstallError::Storage)?;
            if bytes.len() as u64 != expected.size || super::super::hash(&bytes) != expected.sha256
            {
                return Err(InstallError::ArtifactInvalid);
            }
            result.push(bytes);
        }
        self.partial(path, record)?;
        let mut file = regular(&path.join("seal.json"), 512, true)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|_| InstallError::Storage)?;
        if bytes != seal(record) {
            return Err(InstallError::ArtifactInvalid);
        }
        Ok((result.remove(0), result.remove(0)))
    }
    // A crash can leave an empty or partially written stage. It stays inert;
    // abort validates names/types/limits without interpreting or executing it.
    fn partial(&self, path: &Path, record: &Record) -> Result<(), InstallError> {
        let _guard = directory(path)?;
        let mut count = 0;
        for entry in std::fs::read_dir(path).map_err(|_| InstallError::Storage)? {
            count += 1;
            if count > NAMES.len() {
                return Err(InstallError::BudgetExceeded);
            }
            let entry = entry.map_err(|_| InstallError::Storage)?;
            let name = entry.file_name();
            let name = name.to_str().ok_or(InstallError::PathRejected)?;
            let limit = match name {
                "manifest.json" => record.payload.manifest.size,
                "package.zip" => record.payload.archive.size,
                "seal.json" => 512,
                _ => return Err(InstallError::PathRejected),
            };
            regular(&entry.path(), limit, false)?;
        }
        Ok(())
    }
    pub fn stage_bytes(
        &self,
        journal: &Journal,
        manifest: &[u8],
        archive: &[u8],
    ) -> Result<(), InstallError> {
        let path = self.stage(&journal.id);
        std::fs::create_dir(&path).map_err(|_| InstallError::Storage)?;
        let _guard = directory(&path)?;
        let seal = seal(&journal.record);
        for (name, bytes) in [
            ("manifest.json", manifest),
            ("package.zip", archive),
            ("seal.json", seal.as_slice()),
        ] {
            let destination = path.join(name);
            safe_path(&destination).map_err(|_| InstallError::PathRejected)?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .share_mode(0)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                .open(&destination)
                .map_err(|_| InstallError::Storage)?;
            file.write_all(bytes).map_err(|_| InstallError::Storage)?;
            file.sync_all().map_err(|_| InstallError::Storage)?;
            let mut permissions = file
                .metadata()
                .map_err(|_| InstallError::Storage)?
                .permissions();
            permissions.set_readonly(true);
            file.set_permissions(permissions)
                .map_err(|_| InstallError::Storage)?;
            file.sync_all().map_err(|_| InstallError::Storage)?;
        }
        self.read_at(&path, &journal.record)?;
        Ok(())
    }
    pub fn publish(&mut self, journal: &Journal) -> Result<(), InstallError> {
        self.move_owned(
            &self.stage(&journal.id),
            &self.artifact(&journal.record.payload.archive.sha256),
        )
    }
    pub fn quarantine(&mut self, journal: &Journal) -> Result<(), InstallError> {
        for (kind, source) in [
            ("stage", self.stage(&journal.id)),
            (
                "artifact",
                self.artifact(&journal.record.payload.archive.sha256),
            ),
        ] {
            if kind == "artifact" && !journal.new_artifact {
                continue;
            }
            let destination = self
                .root
                .join("quarantine")
                .join(format!("{}-{kind}", journal.id));
            let present = exists(&source)?;
            let quarantined = exists(&destination)?;
            if present && quarantined {
                return Err(InstallError::StateInvalid);
            }
            let selected = if present { &source } else { &destination };
            if present || quarantined {
                if kind == "artifact" {
                    if !journal.prepared {
                        return Err(InstallError::StateInvalid);
                    }
                    self.read_at(selected, &journal.record)?;
                } else {
                    self.partial(selected, &journal.record)?;
                }
            }
            if present {
                self.move_owned(&source, &destination)?;
            }
        }
        Ok(())
    }
    fn move_owned(&mut self, source: &Path, destination: &Path) -> Result<(), InstallError> {
        // Windows publication needs parent-directory access incompatible with
        // the inner guards. Keep profile/root handles held, then reacquire the
        // three internal guards. The native Host's private profile is required;
        // this is not a handle-relative OS sandbox or a hostile same-user claim.
        self._guards.truncate(2);
        let moved = rename_new(source, destination);
        let guarded = (|| {
            for name in ["artifacts", "staging", "quarantine"] {
                self._guards.push(directory(&self.root.join(name))?);
            }
            Ok(())
        })();
        moved?;
        guarded
    }
    pub fn disk_bytes(&self) -> Result<u64, InstallError> {
        self.disk_bytes_reserving(0)
    }
    pub fn disk_bytes_reserving(&self, additional_groups: usize) -> Result<u64, InstallError> {
        let mut total = 0u64;
        let mut groups = additional_groups;
        let names = std::fs::read_dir(&self.root).map_err(|_| InstallError::Storage)?;
        for entry in names {
            let entry = entry.map_err(|_| InstallError::Storage)?;
            if !["artifacts", "staging", "quarantine"]
                .iter()
                .any(|n| entry.file_name() == *n)
            {
                return Err(InstallError::PathRejected);
            }
            for child in std::fs::read_dir(entry.path()).map_err(|_| InstallError::Storage)? {
                groups += 1;
                if groups > MAX_GROUPS {
                    return Err(InstallError::BudgetExceeded);
                }
                let child = child.map_err(|_| InstallError::Storage)?;
                let _guard = directory(&child.path())?;
                let mut files = 0;
                for file in std::fs::read_dir(child.path()).map_err(|_| InstallError::Storage)? {
                    files += 1;
                    if files > NAMES.len() {
                        return Err(InstallError::BudgetExceeded);
                    }
                    let file = file.map_err(|_| InstallError::Storage)?;
                    if !NAMES.iter().any(|n| file.file_name() == *n) {
                        return Err(InstallError::PathRejected);
                    }
                    let limit = if file.file_name() == "seal.json" {
                        512
                    } else if file.file_name() == "manifest.json" {
                        super::super::MAX_PAYLOAD_BYTES as u64
                    } else {
                        super::super::MAX_ARCHIVE_BYTES
                    };
                    total = total
                        .checked_add(
                            regular(&file.path(), limit, false)?
                                .metadata()
                                .map_err(|_| InstallError::Storage)?
                                .len(),
                        )
                        .ok_or(InstallError::BudgetExceeded)?;
                    if total > MAX_DISK_BYTES {
                        return Err(InstallError::BudgetExceeded);
                    }
                }
            }
        }
        Ok(total)
    }
}
fn rename_new(source: &Path, destination: &Path) -> Result<(), InstallError> {
    use std::os::windows::ffi::OsStrExt;
    for path in [source, destination] {
        safe_path(path).map_err(|_| InstallError::PathRejected)?;
    }
    let source: Vec<_> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<_> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // No REPLACE_EXISTING or COPY_ALLOWED: same-volume, create-only publication.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(InstallError::Storage);
    }
    Ok(())
}
