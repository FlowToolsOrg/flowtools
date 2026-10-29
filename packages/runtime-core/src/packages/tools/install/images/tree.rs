//! Fixed private namespace and handle-relative, create-only materialized files.
use super::{
    native::{self, Kind, Open},
    security::Policy,
    Content, DirectoryPin, FilePin, Identity, Journal, Record, MAX_CONTENT_FILES, MAX_DIRECTORIES,
    MAX_SEAL_BYTES,
};
use crate::{
    data::{safe_path, DataStore},
    packages::{
        self,
        install::{files, InstallError},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_READONLY,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, FILE_SHARE_READ,
    FILE_TRAVERSE,
};
type Result<T> = std::result::Result<T, InstallError>;
const NAMESPACE: &str = "tool-image-store-v1";
fn exists(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(InstallError::Storage),
    }
}
fn identity(info: &BY_HANDLE_FILE_INFORMATION) -> Identity {
    Identity {
        volume: info.dwVolumeSerialNumber,
        file_id: format!("{:08x}{:08x}", info.nFileIndexHigh, info.nFileIndexLow),
        creation: format!(
            "{:08x}{:08x}",
            info.ftCreationTime.dwHighDateTime, info.ftCreationTime.dwLowDateTime
        ),
    }
}
fn root_guard(path: &Path) -> Result<File> {
    safe_path(path).map_err(|_| InstallError::PathRejected)?;
    let file = OpenOptions::new()
        .access_mode(FILE_GENERIC_READ | FILE_TRAVERSE)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| InstallError::Storage)?;
    if native::info(&file)?.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(InstallError::PathRejected);
    }
    Ok(file)
}
pub(super) struct Layout {
    pub(super) root: PathBuf,
    _guards: Vec<File>,
    parents: BTreeMap<&'static str, File>,
}
impl Layout {
    pub(super) fn open(store: &DataStore, create: bool) -> Result<Option<Self>> {
        let profile = store.profile().ok_or(InstallError::ProfileRequired)?;
        let mut guards = vec![root_guard(profile)?];
        let root = profile.join(NAMESPACE);
        let private = Policy::private()?;
        let namespace = if exists(&root)? {
            let directory = native::child(&guards[0], NAMESPACE, Kind::Directory, Open::Read)?;
            private.verify(&directory, true)?;
            directory
        } else {
            if !create {
                return Ok(None);
            }
            let directory = native::child(&guards[0], NAMESPACE, Kind::Directory, Open::Create)?;
            private.apply(&directory, true)?;
            directory
        };
        guards.push(namespace);
        let mut parents = BTreeMap::new();
        for name in ["artifacts", "staging", "quarantine"] {
            let directory = if exists(&root.join(name))? {
                let directory = native::child(&guards[1], name, Kind::Directory, Open::Parent)?;
                private.verify(&directory, true)?;
                directory
            } else {
                if !create {
                    continue;
                }
                let directory = native::child(&guards[1], name, Kind::Directory, Open::Create)?;
                private.apply(&directory, true)?;
                drop(directory);
                native::child(&guards[1], name, Kind::Directory, Open::Parent)?
            };
            parents.insert(name, directory);
        }
        Ok(Some(Self {
            root,
            _guards: guards,
            parents,
        }))
    }
    fn parent(&self, kind: &str) -> Result<&File> {
        self.parents.get(kind).ok_or(InstallError::ArtifactInvalid)
    }
    fn group(&self, kind: &str, name: &str, mode: Open) -> Result<File> {
        native::child(self.parent(kind)?, name, Kind::Directory, mode)
    }
    fn directories(
        root: File,
        paths: impl Iterator<Item = String>,
        policy: &Policy,
        create: bool,
    ) -> Result<BTreeMap<String, File>> {
        let mut names = BTreeSet::from([String::new()]);
        for path in paths {
            if !packages::valid_path(&path) {
                return Err(InstallError::PathRejected);
            }
            let mut current = path.rsplit_once('/').map(|(p, _)| p);
            while let Some(directory) = current {
                names.insert(directory.to_owned());
                current = directory.rsplit_once('/').map(|(p, _)| p);
            }
        }
        if names.len() > MAX_DIRECTORIES {
            return Err(InstallError::BudgetExceeded);
        }
        let mut handles = BTreeMap::from([(String::new(), root)]);
        for path in names.into_iter().filter(|p| !p.is_empty()) {
            let (parent, leaf) = path.rsplit_once('/').unwrap_or(("", &path));
            let directory = native::child(
                &handles[parent],
                leaf,
                Kind::Directory,
                if create { Open::Create } else { Open::Read },
            )?;
            if create {
                policy.apply(&directory, true)?;
            } else {
                policy.verify(&directory, true)?;
            }
            handles.insert(path, directory);
        }
        Ok(handles)
    }
    fn content_parent<'a>(
        directories: &'a BTreeMap<String, File>,
        path: &'a str,
    ) -> Result<(&'a File, &'a str)> {
        if !packages::valid_path(path) {
            return Err(InstallError::PathRejected);
        }
        let (parent, leaf) = path.rsplit_once('/').unwrap_or(("", path));
        Ok((
            directories.get(parent).ok_or(InstallError::StateInvalid)?,
            leaf,
        ))
    }
    fn written(parent: &File, name: &str, bytes: &[u8], policy: &Policy) -> Result<FilePin> {
        let mut file = native::child(parent, name, Kind::File, Open::Create)?;
        file.write_all(bytes).map_err(|_| InstallError::Storage)?;
        file.sync_all().map_err(|_| InstallError::Storage)?;
        policy.apply(&file, false)?;
        let mut permission = file
            .metadata()
            .map_err(|_| InstallError::Storage)?
            .permissions();
        permission.set_readonly(true);
        file.set_permissions(permission)
            .map_err(|_| InstallError::Storage)?;
        file.sync_all().map_err(|_| InstallError::Storage)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| InstallError::Storage)?;
        let mut observed = Vec::new();
        (&mut file)
            .take(bytes.len() as u64 + 1)
            .read_to_end(&mut observed)
            .map_err(|_| InstallError::Storage)?;
        if observed != bytes {
            return Err(InstallError::ArtifactInvalid);
        }
        let info = native::info(&file)?;
        if info.nNumberOfLinks != 1 || info.dwFileAttributes & FILE_ATTRIBUTE_READONLY == 0 {
            return Err(InstallError::PathRejected);
        }
        Ok(FilePin {
            content: Content {
                path: name.into(),
                size: bytes.len() as u64,
                sha256: packages::hash(bytes),
            },
            identity: identity(&info),
        })
    }
    pub(super) fn stage(
        &self,
        pending: &Journal,
        contents: &BTreeMap<String, Vec<u8>>,
        profile: &str,
    ) -> Result<Record> {
        if contents.len() > MAX_CONTENT_FILES || contents.contains_key("seal.json") {
            return Err(InstallError::BudgetExceeded);
        }
        let policy = Policy::new(profile, &pending.record.pin.archive_digest)?;
        let root = self.group("staging", &pending.id, Open::Create)?;
        policy.apply(&root, true)?;
        let directories = Self::directories(root, contents.keys().cloned(), &policy, true)?;
        let mut record = pending.record.clone();
        for (path, bytes) in contents {
            let (parent, name) = Self::content_parent(&directories, path)?;
            let mut written = Self::written(parent, name, bytes, &policy)?;
            written.content.path = path.clone();
            record.files.push(written);
        }
        for (path, handle) in &directories {
            record.directories.push(DirectoryPin {
                path: path.clone(),
                identity: identity(&native::info(handle)?),
            });
        }
        let seal = super::seal(&record, profile)?;
        record.seal_file = Some(Self::written(
            &directories[""],
            "seal.json",
            &seal,
            &policy,
        )?);
        Ok(record)
    }
    fn read_file(
        parent: &File,
        name: &str,
        pin: &FilePin,
        policy: &Policy,
    ) -> Result<(File, Vec<u8>)> {
        let mut file = native::child(parent, name, Kind::File, Open::Read)?;
        let info = native::info(&file)?;
        if identity(&info) != pin.identity
            || info.nNumberOfLinks != 1
            || info.dwFileAttributes & FILE_ATTRIBUTE_READONLY == 0
        {
            return Err(InstallError::ArtifactInvalid);
        }
        policy.verify(&file, false)?;
        if file.metadata().map_err(|_| InstallError::Storage)?.len() != pin.content.size {
            return Err(InstallError::ArtifactInvalid);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(pin.content.size + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| InstallError::Storage)?;
        if bytes.len() as u64 != pin.content.size || packages::hash(&bytes) != pin.content.sha256 {
            return Err(InstallError::ArtifactInvalid);
        }
        Ok((file, bytes))
    }
    fn complete_inventory(
        &self,
        path: &Path,
        files: &BTreeSet<String>,
        directories: &BTreeSet<String>,
    ) -> Result<()> {
        let mut observed_files = BTreeSet::new();
        let mut observed_directories = BTreeSet::from([String::new()]);
        let mut queue = vec![String::new()];
        while let Some(relative) = queue.pop() {
            for item in
                std::fs::read_dir(path.join(&relative)).map_err(|_| InstallError::Storage)?
            {
                let item = item.map_err(|_| InstallError::Storage)?;
                let name = item
                    .file_name()
                    .into_string()
                    .map_err(|_| InstallError::PathRejected)?;
                let child = if relative.is_empty() {
                    name
                } else {
                    format!("{relative}/{name}")
                };
                if !packages::valid_path(&child) {
                    return Err(InstallError::PathRejected);
                }
                let metadata =
                    std::fs::symlink_metadata(item.path()).map_err(|_| InstallError::Storage)?;
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes()
                    & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                    != 0
                {
                    return Err(InstallError::PathRejected);
                }
                if metadata.is_dir() {
                    if !directories.contains(&child) || !observed_directories.insert(child.clone())
                    {
                        return Err(InstallError::ArtifactInvalid);
                    }
                    queue.push(child);
                } else {
                    if !metadata.is_file()
                        || !files.contains(&child)
                        || !observed_files.insert(child)
                    {
                        return Err(InstallError::ArtifactInvalid);
                    }
                }
                if observed_files.len() > MAX_CONTENT_FILES + 1
                    || observed_directories.len() > MAX_DIRECTORIES
                {
                    return Err(InstallError::BudgetExceeded);
                }
            }
        }
        if &observed_files != files || &observed_directories != directories {
            return Err(InstallError::ArtifactInvalid);
        }
        Ok(())
    }
    fn read_at(
        &self,
        kind: &str,
        name: &str,
        record: &Record,
        profile: &str,
        moving: bool,
    ) -> Result<Vec<File>> {
        let policy = if kind == "quarantine" {
            Policy::for_quarantine(profile, &record.pin.archive_digest)?
        } else {
            Policy::new(profile, &record.pin.archive_digest)?
        };
        let root = self.group(
            kind,
            name,
            if moving { Open::DuringMove } else { Open::Read },
        )?;
        policy.verify(&root, true)?;
        let directories = Self::directories(
            root,
            record.files.iter().map(|f| f.content.path.clone()),
            &policy,
            false,
        )?;
        if directories.len() != record.directories.len() {
            return Err(InstallError::ArtifactInvalid);
        }
        for pin in &record.directories {
            let directory = directories
                .get(&pin.path)
                .ok_or(InstallError::ArtifactInvalid)?;
            if identity(&native::info(directory)?) != pin.identity {
                return Err(InstallError::ArtifactInvalid);
            }
        }
        let mut held = Vec::new();
        for pin in &record.files {
            let (parent, name) = Self::content_parent(&directories, &pin.content.path)?;
            held.push(Self::read_file(parent, name, pin, &policy)?.0);
        }
        let pin = record
            .seal_file
            .as_ref()
            .ok_or(InstallError::StateInvalid)?;
        let (file, bytes) = Self::read_file(&directories[""], "seal.json", pin, &policy)?;
        if bytes != super::seal(record, profile)? {
            return Err(InstallError::ArtifactInvalid);
        }
        held.push(file);
        let mut files = record
            .files
            .iter()
            .map(|f| f.content.path.clone())
            .collect::<BTreeSet<_>>();
        files.insert("seal.json".into());
        self.complete_inventory(
            &self.root.join(kind).join(name),
            &files,
            &directories.keys().cloned().collect(),
        )?;
        held.extend(directories.into_values());
        Ok(held)
    }
    pub(super) fn read(&self, record: &Record, profile: &str) -> Result<Vec<File>> {
        self.read_at(
            "artifacts",
            &record.pin.archive_digest,
            record,
            profile,
            false,
        )
    }
    pub(super) fn publish(&self, pending: &Journal, profile: &str) -> Result<()> {
        files::move_group_held(
            self.parent("staging")?,
            &pending.id,
            self.parent("artifacts")?,
            &pending.record.pin.archive_digest,
            || {
                self.read_at("staging", &pending.id, &pending.record, profile, true)?;
                Ok(())
            },
        )
    }
    pub(super) fn has_artifact(&self, record: &Record) -> Result<bool> {
        exists(&self.root.join("artifacts").join(&record.pin.archive_digest))
    }
    fn group_names(&self, kind: &str) -> Result<Vec<String>> {
        if !self.parents.contains_key(kind) {
            return Ok(Vec::new());
        }
        native::info(self.parent(kind)?)?;
        let mut names = Vec::new();
        for entry in std::fs::read_dir(self.root.join(kind)).map_err(|_| InstallError::Storage)? {
            let entry = entry.map_err(|_| InstallError::Storage)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| InstallError::PathRejected)?;
            let valid = match kind {
                "artifacts" => packages::valid_digest(&name),
                "staging" => super::uuid(&name),
                "quarantine" => name
                    .strip_suffix("-stage")
                    .or_else(|| name.strip_suffix("-artifact"))
                    .is_some_and(super::uuid),
                _ => false,
            };
            if !valid {
                return Err(InstallError::PathRejected);
            }
            self.group(kind, &name, Open::Read)?;
            names.push(name);
            if names.len() > files::MAX_GROUPS {
                return Err(InstallError::BudgetExceeded);
            }
        }
        native::info(self.parent(kind)?)?;
        names.sort();
        Ok(names)
    }
    pub(super) fn assert_owned(&self, records: &[Record], pending: Option<&Journal>) -> Result<()> {
        for kind in ["artifacts", "staging"] {
            for name in self.group_names(kind)? {
                let owned = if kind == "artifacts" {
                    records.iter().any(|r| r.pin.archive_digest == name)
                        || pending
                            .is_some_and(|p| p.prepared && p.record.pin.archive_digest == name)
                } else {
                    pending.is_some_and(|p| p.id == name)
                };
                if !owned {
                    return Err(InstallError::StateInvalid);
                }
            }
        }
        for record in records {
            if !self.has_artifact(record)? {
                return Err(InstallError::ArtifactInvalid);
            }
        }
        Ok(())
    }
    fn walk_group(
        &self,
        kind: &str,
        name: &str,
        moving: bool,
        mut directory: impl FnMut(&str, &File) -> Result<()>,
        mut leaf: impl FnMut(&str, &File) -> Result<()>,
    ) -> Result<()> {
        let root = self.group(
            kind,
            name,
            if moving { Open::DuringMove } else { Open::Read },
        )?;
        let mut held = BTreeMap::from([(String::new(), root)]);
        let mut queue = vec![String::new()];
        let mut count = 0;
        while let Some(parent_path) = queue.pop() {
            for item in std::fs::read_dir(self.root.join(kind).join(name).join(&parent_path))
                .map_err(|_| InstallError::Storage)?
            {
                let item = item.map_err(|_| InstallError::Storage)?;
                let part = item
                    .file_name()
                    .into_string()
                    .map_err(|_| InstallError::PathRejected)?;
                if !packages::valid_path(&part) || part.contains('/') {
                    return Err(InstallError::PathRejected);
                }
                let relative = if parent_path.is_empty() {
                    part.clone()
                } else {
                    format!("{parent_path}/{part}")
                };
                if !packages::valid_path(&relative) {
                    return Err(InstallError::PathRejected);
                }
                let metadata =
                    std::fs::symlink_metadata(item.path()).map_err(|_| InstallError::Storage)?;
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes()
                    & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                    != 0
                {
                    return Err(InstallError::PathRejected);
                }
                if metadata.is_dir() {
                    let opened =
                        native::child(&held[&parent_path], &part, Kind::Directory, Open::Read)?;
                    directory(&relative, &opened)?;
                    held.insert(relative.clone(), opened);
                    queue.push(relative);
                    if held.len() > MAX_DIRECTORIES {
                        return Err(InstallError::BudgetExceeded);
                    }
                } else {
                    if !metadata.is_file() {
                        return Err(InstallError::PathRejected);
                    }
                    let file = native::child(&held[&parent_path], &part, Kind::File, Open::Read)?;
                    leaf(&relative, &file)?;
                    count += 1;
                    if count > MAX_CONTENT_FILES + 1 {
                        return Err(InstallError::BudgetExceeded);
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn usage(&self, reserved: usize) -> Result<files::DiskUsage> {
        let mut usage = files::DiskUsage {
            bytes: 0,
            groups: reserved,
        };
        for kind in ["artifacts", "staging", "quarantine"] {
            for name in self.group_names(kind)? {
                usage.groups = usage
                    .groups
                    .checked_add(1)
                    .ok_or(InstallError::BudgetExceeded)?;
                if usage.groups > files::MAX_GROUPS {
                    return Err(InstallError::BudgetExceeded);
                }
                self.walk_group(
                    kind,
                    &name,
                    false,
                    |_, _| Ok(()),
                    |_, file| {
                        let size = file.metadata().map_err(|_| InstallError::Storage)?.len();
                        if size > packages::MAX_ARCHIVE_BYTES {
                            return Err(InstallError::BudgetExceeded);
                        }
                        usage.bytes = usage
                            .bytes
                            .checked_add(size)
                            .ok_or(InstallError::BudgetExceeded)?;
                        if usage.bytes > files::MAX_DISK_BYTES {
                            return Err(InstallError::BudgetExceeded);
                        }
                        Ok(())
                    },
                )?;
            }
        }
        Ok(usage)
    }
    fn partial(&self, kind: &str, name: &str, expected: &[Content], moving: bool) -> Result<()> {
        let mut allowed = BTreeSet::new();
        for content in expected {
            let mut parent = content.path.rsplit_once('/').map(|(p, _)| p);
            while let Some(path) = parent {
                allowed.insert(path);
                parent = path.rsplit_once('/').map(|(p, _)| p);
            }
        }
        self.walk_group(
            kind,
            name,
            moving,
            |path, _| {
                if allowed.contains(path) {
                    Ok(())
                } else {
                    Err(InstallError::PathRejected)
                }
            },
            |relative, file| {
                let maximum = if relative == "seal.json" {
                    MAX_SEAL_BYTES
                } else {
                    expected
                        .iter()
                        .find(|f| f.path == relative)
                        .ok_or(InstallError::PathRejected)?
                        .size
                };
                if file.metadata().map_err(|_| InstallError::Storage)?.len() > maximum {
                    return Err(InstallError::BudgetExceeded);
                }
                Ok(())
            },
        )
    }
    pub(super) fn quarantine(
        &self,
        pending: &Journal,
        profile: &str,
        expected: &[Content],
    ) -> Result<()> {
        self.parent("quarantine")?;
        let groups = [
            ("stage", "staging", pending.id.as_str()),
            (
                "artifact",
                "artifacts",
                pending.record.pin.archive_digest.as_str(),
            ),
        ];
        // Validate the whole batch before the first move. Every move repeats
        // validation under the held native source and parent handles.
        let mut selected = Vec::new();
        for (kind, source, name) in groups {
            let destination = format!("{}-{kind}", pending.id);
            let present = exists(&self.root.join(source).join(name))?;
            let quarantined = exists(&self.root.join("quarantine").join(&destination))?;
            if present && quarantined {
                return Err(InstallError::StateInvalid);
            }
            if !present && !quarantined {
                continue;
            }
            if kind == "artifact" && !pending.prepared {
                return Err(InstallError::StateInvalid);
            }
            let (selected_kind, selected_name) = if present {
                (source, name)
            } else {
                ("quarantine", destination.as_str())
            };
            let verify = |moving| {
                if kind == "artifact" {
                    self.read_at(
                        selected_kind,
                        selected_name,
                        &pending.record,
                        profile,
                        moving,
                    )?;
                    Ok(())
                } else {
                    self.partial(selected_kind, selected_name, expected, moving)
                }
            };
            verify(false)?;
            selected.push((kind, source, name, destination, present));
        }
        for (kind, source, name, destination, present) in selected {
            if present {
                files::move_group_held(
                    self.parent(source)?,
                    name,
                    self.parent("quarantine")?,
                    &destination,
                    || {
                        if kind == "artifact" {
                            self.read_at(source, name, &pending.record, profile, true)?;
                            Ok(())
                        } else {
                            self.partial(source, name, expected, true)
                        }
                    },
                )?;
            }
            self.partial("quarantine", &destination, expected, false)?;
            self.privatize(&destination)?;
            if kind == "artifact" {
                self.read_at("quarantine", &destination, &pending.record, profile, false)?;
            } else {
                self.partial("quarantine", &destination, expected, false)?;
            }
        }
        Ok(())
    }
    fn privatize(&self, name: &str) -> Result<()> {
        let policy = Policy::private()?;
        let root = self.group("quarantine", name, Open::Protect)?;
        policy.apply(&root, true)?;
        let mut held = BTreeMap::from([(String::new(), root)]);
        let mut queue = vec![String::new()];
        let mut count = 0;
        while let Some(parent) = queue.pop() {
            for entry in std::fs::read_dir(self.root.join("quarantine").join(name).join(&parent))
                .map_err(|_| InstallError::Storage)?
            {
                let entry = entry.map_err(|_| InstallError::Storage)?;
                let part = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| InstallError::PathRejected)?;
                let path = if parent.is_empty() {
                    part.clone()
                } else {
                    format!("{parent}/{part}")
                };
                let metadata =
                    std::fs::symlink_metadata(entry.path()).map_err(|_| InstallError::Storage)?;
                let directory = metadata.is_dir();
                let file = native::child(
                    &held[&parent],
                    &part,
                    if directory {
                        Kind::Directory
                    } else {
                        Kind::File
                    },
                    Open::Protect,
                )?;
                policy.apply(&file, directory)?;
                if directory {
                    held.insert(path.clone(), file);
                    queue.push(path);
                    if held.len() > MAX_DIRECTORIES {
                        return Err(InstallError::BudgetExceeded);
                    }
                } else {
                    count += 1;
                    if count > MAX_CONTENT_FILES + 1 {
                        return Err(InstallError::BudgetExceeded);
                    }
                }
            }
        }
        Ok(())
    }
}
