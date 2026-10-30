//! Fixed inert blob names, create-only writes and handle-held directory guards.
mod rename;
use super::{InstallError, Journal, Record};
use crate::packages::PackagePayload;

pub(crate) trait BlobRecord {
    fn payload(&self) -> &PackagePayload;
    fn artifact_id(&self) -> &str;
}
pub(crate) trait BlobJournal {
    fn record(&self) -> &dyn BlobRecord;
    fn id(&self) -> &str;
    fn new_artifact(&self) -> bool;
    fn prepared(&self) -> bool;
}
impl BlobRecord for Record {
    fn payload(&self) -> &PackagePayload {
        &self.payload
    }
    fn artifact_id(&self) -> &str {
        &self.artifact_id
    }
}
impl BlobJournal for Journal {
    fn record(&self) -> &dyn BlobRecord {
        &self.record
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn new_artifact(&self) -> bool {
        self.new_artifact
    }
    fn prepared(&self) -> bool {
        self.prepared
    }
}

use crate::data::{safe_path, DataStore};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

pub(crate) const MAX_DISK_BYTES: u64 = 268_435_456;
pub(crate) const MAX_GROUPS: usize = 384;
#[derive(Default)]
pub(crate) struct DiskUsage {
    pub bytes: u64,
    pub groups: usize,
}
impl DiskUsage {
    pub(crate) fn combine(self, other: Self) -> Result<Self, InstallError> {
        Ok(Self {
            bytes: self
                .bytes
                .checked_add(other.bytes)
                .ok_or(InstallError::BudgetExceeded)?,
            groups: self
                .groups
                .checked_add(other.groups)
                .ok_or(InstallError::BudgetExceeded)?,
        })
    }
}
// A materialization transaction keeps the source directory and all exact blob
// files open until its final commit checks; ordinary inert reads stay unchanged.
pub(crate) struct HeldBlob {
    pub manifest: Vec<u8>,
    pub archive: Vec<u8>,
    pub guards: Vec<File>,
}
pub(crate) fn move_group_held(
    source_parent: &File,
    source_name: &str,
    target_parent: &File,
    target_name: &str,
    validate: impl FnOnce() -> Result<(), InstallError>,
) -> Result<(), InstallError> {
    rename::move_directory(
        source_parent,
        source_name,
        target_parent,
        target_name,
        validate,
    )
}
const NAMES: [&str; 3] = ["manifest.json", "package.zip", "seal.json"];
fn seal(record: &dyn BlobRecord) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"formatVersion":1,"artifactId":record.artifact_id(),"zipDigest":record.payload().archive.sha256,"manifestDigest":record.payload().manifest.sha256})).expect("bounded seal")
}
pub(crate) struct Layout {
    pub root: PathBuf,
    _guards: Vec<File>,
    parents: BTreeMap<&'static str, File>,
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
    directory_shared(path, FILE_SHARE_READ)
}
fn parent_directory(path: &Path) -> Result<File, InstallError> {
    // Native rename needs parent entry-write sharing. Keep delete sharing closed
    // for the entire operation, including source validation and publication.
    directory_shared(path, FILE_SHARE_READ | FILE_SHARE_WRITE)
}
fn directory_shared(path: &Path, sharing: u32) -> Result<File, InstallError> {
    safe_path(path).map_err(|_| InstallError::PathRejected)?;
    let file = OpenOptions::new()
        .read(true)
        .share_mode(sharing)
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
    // Root/profile handles exclude write/delete sharing; internal parent handles
    // permit entry-write sharing but exclude delete throughout. No client path.
    pub fn open(store: &DataStore, create: bool) -> Result<Option<Self>, InstallError> {
        Self::open_named(store, create, "package-store-v1")
    }
    pub(crate) fn open_for_tools(
        store: &DataStore,
        create: bool,
    ) -> Result<Option<Self>, InstallError> {
        Self::open_named(store, create, "tool-package-store-v1")
    }
    fn open_named(
        store: &DataStore,
        create: bool,
        name: &str,
    ) -> Result<Option<Self>, InstallError> {
        if std::env::consts::ARCH != "x86_64" {
            return Err(InstallError::Unsupported);
        }
        let profile = store.profile().ok_or(InstallError::ProfileRequired)?;
        let mut guards = vec![directory(profile)?];
        let root = profile.join(name);
        if !exists(&root)? {
            if !create {
                return Ok(None);
            }
            std::fs::create_dir(&root).map_err(|_| InstallError::Storage)?;
        }
        guards.push(directory(&root)?);
        let mut parents = BTreeMap::new();
        for name in ["artifacts", "staging", "quarantine"] {
            let path = root.join(name);
            if !exists(&path)? {
                if !create {
                    continue;
                }
                std::fs::create_dir(&path).map_err(|_| InstallError::Storage)?;
            }
            parents.insert(name, parent_directory(&path)?);
        }
        Ok(Some(Self {
            root,
            _guards: guards,
            parents,
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
        records: &[impl BlobRecord],
        journal: Option<&dyn BlobJournal>,
    ) -> Result<(), InstallError> {
        self.assert_owned_with_orphans(records, journal, &[])
    }
    pub(crate) fn assert_owned_with_orphans(
        &self,
        records: &[impl BlobRecord],
        journal: Option<&dyn BlobJournal>,
        orphans: &[super::reconciliation::Orphan],
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
                            && (records.iter().any(|r| r.payload().archive.sha256 == name)
                                || orphans.iter().any(|o| {
                                    o.kind == super::reconciliation::Kind::Artifact
                                        && o.source == name
                                })
                                || journal.is_some_and(|j| {
                                    j.new_artifact()
                                        && j.prepared()
                                        && j.record().payload().archive.sha256 == name
                                }))
                    }
                    "staging" => {
                        journal.is_some_and(|j| j.new_artifact() && j.id() == name)
                            || orphans.iter().any(|o| {
                                o.kind == super::reconciliation::Kind::Stage && o.source == name
                            })
                    }
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
    pub fn has_artifact(&self, record: &dyn BlobRecord) -> Result<bool, InstallError> {
        exists(&self.artifact(&record.payload().archive.sha256))
    }
    pub fn read(&self, record: &dyn BlobRecord) -> Result<(Vec<u8>, Vec<u8>), InstallError> {
        self.read_at(&self.artifact(&record.payload().archive.sha256), record)
    }
    pub(crate) fn hold(&self, record: &dyn BlobRecord) -> Result<HeldBlob, InstallError> {
        let path = self.artifact(&record.payload().archive.sha256);
        let mut guards = vec![directory(&path)?];
        for (name, limit) in [
            ("manifest.json", record.payload().manifest.size),
            ("package.zip", record.payload().archive.size),
            ("seal.json", 512),
        ] {
            guards.push(regular(&path.join(name), limit, true)?);
        }
        let (manifest, archive) = self.read_held(&path, record)?;
        Ok(HeldBlob {
            manifest,
            archive,
            guards,
        })
    }
    fn read_at(
        &self,
        path: &Path,
        record: &dyn BlobRecord,
    ) -> Result<(Vec<u8>, Vec<u8>), InstallError> {
        let _guard = directory(path)?;
        self.read_held(path, record)
    }
    fn read_held(
        &self,
        path: &Path,
        record: &dyn BlobRecord,
    ) -> Result<(Vec<u8>, Vec<u8>), InstallError> {
        let mut result = Vec::new();
        for (name, expected) in [
            ("manifest.json", &record.payload().manifest),
            (
                "package.zip",
                &super::super::BlobDescriptor {
                    size: record.payload().archive.size,
                    sha256: record.payload().archive.sha256.clone(),
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
        self.partial_held(path, record)?;
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
    fn partial(&self, path: &Path, record: &dyn BlobRecord) -> Result<(), InstallError> {
        let _guard = directory(path)?;
        self.partial_held(path, record)
    }
    fn partial_held(&self, path: &Path, record: &dyn BlobRecord) -> Result<(), InstallError> {
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
                "manifest.json" => record.payload().manifest.size,
                "package.zip" => record.payload().archive.size,
                "seal.json" => 512,
                _ => return Err(InstallError::PathRejected),
            };
            regular(&entry.path(), limit, false)?;
        }
        Ok(())
    }
    pub fn stage_bytes(
        &self,
        journal: &dyn BlobJournal,
        manifest: &[u8],
        archive: &[u8],
    ) -> Result<(), InstallError> {
        let path = self.stage(journal.id());
        std::fs::create_dir(&path).map_err(|_| InstallError::Storage)?;
        let _guard = directory(&path)?;
        let seal = seal(journal.record());
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
        self.read_at(&path, journal.record())?;
        Ok(())
    }
    pub fn publish(&mut self, journal: &dyn BlobJournal) -> Result<(), InstallError> {
        self.move_owned(
            &self.stage(journal.id()),
            &self.artifact(&journal.record().payload().archive.sha256),
            journal.record(),
            true,
        )
    }
    pub fn quarantine(&mut self, journal: &dyn BlobJournal) -> Result<(), InstallError> {
        for (kind, source) in [
            ("stage", self.stage(journal.id())),
            (
                "artifact",
                self.artifact(&journal.record().payload().archive.sha256),
            ),
        ] {
            if kind == "artifact" && !journal.new_artifact() {
                continue;
            }
            let destination = self
                .root
                .join("quarantine")
                .join(format!("{}-{kind}", journal.id()));
            let present = exists(&source)?;
            let quarantined = exists(&destination)?;
            if present && quarantined {
                return Err(InstallError::StateInvalid);
            }
            let selected = if present { &source } else { &destination };
            if present || quarantined {
                if kind == "artifact" {
                    if !journal.prepared() {
                        return Err(InstallError::StateInvalid);
                    }
                    self.read_at(selected, journal.record())?;
                } else {
                    self.partial(selected, journal.record())?;
                }
            }
            if present {
                self.move_owned(&source, &destination, journal.record(), kind == "artifact")?;
            }
        }
        Ok(())
    }
    fn move_owned(
        &self,
        source: &Path,
        destination: &Path,
        record: &dyn BlobRecord,
        complete: bool,
    ) -> Result<(), InstallError> {
        self.move_validated(source, destination, || {
            if complete {
                self.read_held(source, record)?;
            } else {
                self.partial_held(source, record)?;
            }
            Ok(())
        })
    }
    fn move_validated(
        &self,
        source: &Path,
        destination: &Path,
        validate: impl FnOnce() -> Result<(), InstallError>,
    ) -> Result<(), InstallError> {
        let parent = |path: &Path| {
            self.parents
                .iter()
                .find(|(name, _)| path.parent() == Some(self.root.join(name).as_path()))
                .map(|(_, handle)| handle)
                .ok_or(InstallError::PathRejected)
        };
        let name = |path: &Path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
                .ok_or(InstallError::PathRejected)
        };
        rename::move_directory(
            parent(source)?,
            &name(source)?,
            parent(destination)?,
            &name(destination)?,
            validate,
        )
    }
    pub(crate) fn discover_orphans(
        &self,
        records: &[impl BlobRecord],
    ) -> Result<Vec<super::reconciliation::Orphan>, InstallError> {
        use super::reconciliation::{Kind, Orphan};
        let mut orphans = Vec::new();
        for kind in [Kind::Artifact, Kind::Stage] {
            let parent = self.root.join(kind.parent());
            if !exists(&parent)? {
                continue;
            }
            for entry in std::fs::read_dir(&parent).map_err(|_| InstallError::Storage)? {
                if orphans.len() >= MAX_GROUPS {
                    return Err(InstallError::BudgetExceeded);
                }
                let entry = entry.map_err(|_| InstallError::Storage)?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| InstallError::PathRejected)?;
                kind.validate_source(&name)?;
                if kind == Kind::Artifact
                    && records.iter().any(|r| r.payload().archive.sha256 == name)
                {
                    continue;
                }
                orphans.push(Orphan {
                    kind,
                    source: name,
                    destination_id: uuid::Uuid::new_v4().to_string(),
                    files: self.fingerprint(&entry.path())?,
                });
            }
        }
        orphans.sort_by(|a, b| (a.kind, &a.source).cmp(&(b.kind, &b.source)));
        self.assert_owned_with_orphans(records, None, &orphans)?;
        if !orphans.is_empty() {
            self.require_quarantine()?;
        }
        Ok(orphans)
    }
    pub fn require_quarantine(&self) -> Result<(), InstallError> {
        if !self.parents.contains_key("quarantine") {
            return Err(InstallError::ArtifactInvalid);
        }
        Ok(())
    }
    fn fingerprint(
        &self,
        path: &Path,
    ) -> Result<BTreeMap<String, super::super::BlobDescriptor>, InstallError> {
        let _guard = directory(path)?;
        self.fingerprint_held(path)
    }
    fn fingerprint_held(
        &self,
        path: &Path,
    ) -> Result<BTreeMap<String, super::super::BlobDescriptor>, InstallError> {
        let mut result = BTreeMap::new();
        for entry in std::fs::read_dir(path).map_err(|_| InstallError::Storage)? {
            if result.len() >= NAMES.len() {
                return Err(InstallError::BudgetExceeded);
            }
            let entry = entry.map_err(|_| InstallError::Storage)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| InstallError::PathRejected)?;
            let limit = match name.as_str() {
                "manifest.json" => super::super::MAX_PAYLOAD_BYTES as u64,
                "package.zip" => super::super::MAX_ARCHIVE_BYTES,
                "seal.json" => 512,
                _ => return Err(InstallError::PathRejected),
            };
            let mut file = regular(&entry.path(), limit, false)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|_| InstallError::Storage)?;
            result.insert(
                name,
                super::super::BlobDescriptor {
                    sha256: super::super::hash(&bytes),
                    size: bytes.len() as u64,
                },
            );
        }
        Ok(result)
    }
    pub(crate) fn check_orphan(
        &self,
        orphan: &super::reconciliation::Orphan,
    ) -> Result<bool, InstallError> {
        let source = self.root.join(orphan.kind.parent()).join(&orphan.source);
        let destination = self.root.join("quarantine").join(orphan.destination());
        let present = exists(&source)?;
        let quarantined = exists(&destination)?;
        if present == quarantined {
            return Err(InstallError::StateInvalid);
        }
        let selected = if present { &source } else { &destination };
        if self.fingerprint(selected)? != orphan.files {
            return Err(InstallError::ArtifactInvalid);
        }
        Ok(present)
    }
    pub(crate) fn quarantine_orphan(
        &self,
        orphan: &super::reconciliation::Orphan,
    ) -> Result<(), InstallError> {
        if !self.check_orphan(orphan)? {
            return Ok(());
        }
        let source = self.root.join(orphan.kind.parent()).join(&orphan.source);
        let destination = self.root.join("quarantine").join(orphan.destination());
        self.move_validated(&source, &destination, || {
            if self.fingerprint_held(&source)? != orphan.files {
                return Err(InstallError::ArtifactInvalid);
            }
            Ok(())
        })?;
        if self.check_orphan(orphan)? {
            return Err(InstallError::StateInvalid);
        }
        Ok(())
    }
    pub fn disk_bytes(&self) -> Result<u64, InstallError> {
        self.disk_bytes_reserving(0)
    }
    pub fn disk_bytes_reserving(&self, additional_groups: usize) -> Result<u64, InstallError> {
        Ok(self.disk_usage_reserving(additional_groups)?.bytes)
    }
    pub(crate) fn disk_usage_reserving(
        &self,
        additional_groups: usize,
    ) -> Result<DiskUsage, InstallError> {
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
        Ok(DiskUsage {
            bytes: total,
            groups,
        })
    }
}
