//! Observed T0-owned orphan bytes, never an active image or signed seal proof.
use super::*;
use crate::packages::install::reconciliation::Kind as GroupKind;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObservedFile {
    pin: FilePin,
    security_digest: String,
    read_only: bool,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObservedDirectory {
    pin: DirectoryPin,
    security_digest: String,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Observation {
    files: Vec<ObservedFile>,
    directories: Vec<ObservedDirectory>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::packages::tools::install::images) struct Orphan {
    pub(in crate::packages::tools::install::images) kind: GroupKind,
    pub(in crate::packages::tools::install::images) source: String,
    destination_id: String,
    observation: Observation,
}
impl Orphan {
    pub(in crate::packages::tools::install::images) fn destination(&self) -> String {
        format!(
            "{}-{}",
            self.destination_id,
            match self.kind {
                GroupKind::Artifact => "artifact",
                GroupKind::Stage => "stage",
            }
        )
    }
    pub(in crate::packages::tools::install::images) fn bytes(&self) -> u64 {
        self.observation
            .files
            .iter()
            .map(|f| f.pin.content.size)
            .sum()
    }
    pub(in crate::packages::tools::install::images) fn validate(
        &self,
        records: &[Record],
    ) -> Result<()> {
        self.kind.validate_source(&self.source)?;
        if !super::super::uuid(&self.destination_id)
            || (self.kind == GroupKind::Artifact
                && records.iter().any(|r| r.pin.archive_digest == self.source))
            || self.observation.files.len() > MAX_CONTENT_FILES + 1
            || self.observation.directories.is_empty()
            || self.observation.directories.len() > MAX_DIRECTORIES
        {
            return Err(InstallError::StateInvalid);
        }
        let mut paths = BTreeSet::new();
        let mut identities = BTreeSet::new();
        for directory in &self.observation.directories {
            let pin = &directory.pin;
            if (!pin.path.is_empty() && !packages::valid_path(&pin.path))
                || !paths.insert(pin.path.as_str())
                || !super::super::identity_valid(&pin.identity)
                || !packages::valid_digest(&directory.security_digest)
                || !identities.insert((pin.identity.volume, &pin.identity.file_id))
            {
                return Err(InstallError::StateInvalid);
            }
        }
        if !paths.contains("") {
            return Err(InstallError::StateInvalid);
        }
        let directory_paths = paths.clone();
        let mut bytes = 0u64;
        for file in &self.observation.files {
            let pin = &file.pin;
            if !packages::valid_path(&pin.content.path)
                || !paths.insert(pin.content.path.as_str())
                || !packages::valid_digest(&pin.content.sha256)
                || !packages::valid_digest(&file.security_digest)
                || !super::super::identity_valid(&pin.identity)
                || pin.content.size > packages::MAX_ARCHIVE_BYTES
                || !identities.insert((pin.identity.volume, &pin.identity.file_id))
            {
                return Err(InstallError::StateInvalid);
            }
            bytes = bytes
                .checked_add(pin.content.size)
                .ok_or(InstallError::BudgetExceeded)?;
        }
        for path in paths.iter().filter(|p| !p.is_empty()) {
            let parent = path.rsplit_once('/').map_or("", |(p, _)| p);
            if !directory_paths.contains(parent) {
                return Err(InstallError::StateInvalid);
            }
        }
        if bytes > files::MAX_DISK_BYTES {
            return Err(InstallError::BudgetExceeded);
        }
        Ok(())
    }
}
fn file_observation(path: &str, file: &File) -> Result<ObservedFile> {
    let info = native::info(file)?;
    let size = file.metadata().map_err(|_| InstallError::Storage)?.len();
    if info.nNumberOfLinks != 1 || size > packages::MAX_ARCHIVE_BYTES {
        return Err(InstallError::BudgetExceeded);
    }
    let mut bytes = Vec::new();
    file.take(size + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| InstallError::Storage)?;
    if bytes.len() as u64 != size {
        return Err(InstallError::ArtifactInvalid);
    }
    Ok(ObservedFile {
        pin: FilePin {
            content: Content {
                path: path.into(),
                size,
                sha256: packages::hash(&bytes),
            },
            identity: identity(&info),
        },
        security_digest: super::super::security::fingerprint(file)?,
        read_only: info.dwFileAttributes & FILE_ATTRIBUTE_READONLY != 0,
    })
}
fn directory_observation(path: &str, file: &File) -> Result<ObservedDirectory> {
    Ok(ObservedDirectory {
        pin: DirectoryPin {
            path: path.into(),
            identity: identity(&native::info(file)?),
        },
        security_digest: super::super::security::fingerprint(file)?,
    })
}
impl Layout {
    fn observe_orphan(&self, kind: &str, name: &str, moving: bool) -> Result<Observation> {
        let root = self.group(
            kind,
            name,
            if moving { Open::DuringMove } else { Open::Read },
        )?;
        let mut directories = vec![directory_observation("", &root)?];
        let mut files = Vec::new();
        self.walk_group(
            kind,
            name,
            moving,
            |path, file| {
                directories.push(directory_observation(path, file)?);
                Ok(())
            },
            |path, file| {
                files.push(file_observation(path, file)?);
                Ok(())
            },
        )?;
        files.sort_by(|a, b| a.pin.content.path.cmp(&b.pin.content.path));
        directories.sort_by(|a, b| a.pin.path.cmp(&b.pin.path));
        Ok(Observation { files, directories })
    }
    pub(in crate::packages::tools::install::images) fn discover_image_orphans(
        &self,
        records: &[Record],
    ) -> Result<Vec<Orphan>> {
        let mut orphans = Vec::new();
        for kind in [GroupKind::Artifact, GroupKind::Stage] {
            for name in self.group_names(kind.parent())? {
                if kind == GroupKind::Artifact
                    && records.iter().any(|r| r.pin.archive_digest == name)
                {
                    continue;
                }
                if orphans.len() >= files::MAX_GROUPS {
                    return Err(InstallError::BudgetExceeded);
                }
                let orphan = Orphan {
                    kind,
                    observation: self.observe_orphan(kind.parent(), &name, false)?,
                    source: name,
                    destination_id: uuid::Uuid::new_v4().to_string(),
                };
                orphan.validate(records)?;
                orphans.push(orphan);
            }
        }
        orphans.sort_by(|a, b| (a.kind, &a.source).cmp(&(b.kind, &b.source)));
        self.assert_image_orphans(records, &orphans)?;
        if !orphans.is_empty() {
            self.parent("quarantine")?;
        }
        Ok(orphans)
    }
    pub(in crate::packages::tools::install::images) fn assert_image_orphans(
        &self,
        records: &[Record],
        orphans: &[Orphan],
    ) -> Result<()> {
        for kind in [GroupKind::Artifact, GroupKind::Stage] {
            for name in self.group_names(kind.parent())? {
                let owned = (kind == GroupKind::Artifact
                    && records.iter().any(|r| r.pin.archive_digest == name))
                    || orphans.iter().any(|o| o.kind == kind && o.source == name);
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
    // Security may differ only after this exact group is already in quarantine:
    // each node is either the observed original or the exact private policy.
    fn observation_matches(
        actual: &Observation,
        expected: &Observation,
        quarantined: bool,
        private: &Policy,
    ) -> Result<()> {
        if actual.files.len() != expected.files.len()
            || actual.directories.len() != expected.directories.len()
        {
            return Err(InstallError::ArtifactInvalid);
        }
        for (a, e) in actual.files.iter().zip(&expected.files) {
            if a.pin != e.pin
                || a.read_only != e.read_only
                || (a.security_digest != e.security_digest
                    && (!quarantined || a.security_digest != private.digest(false)))
            {
                return Err(InstallError::ArtifactInvalid);
            }
        }
        for (a, e) in actual.directories.iter().zip(&expected.directories) {
            if a.pin != e.pin
                || (a.security_digest != e.security_digest
                    && (!quarantined || a.security_digest != private.digest(true)))
            {
                return Err(InstallError::ArtifactInvalid);
            }
        }
        Ok(())
    }
    pub(in crate::packages::tools::install::images) fn check_image_orphan(
        &self,
        orphan: &Orphan,
    ) -> Result<bool> {
        let present = exists(&self.root.join(orphan.kind.parent()).join(&orphan.source))?;
        let destination = orphan.destination();
        let quarantined = exists(&self.root.join("quarantine").join(&destination))?;
        if present == quarantined {
            return Err(InstallError::StateInvalid);
        }
        let actual = self.observe_orphan(
            if present {
                orphan.kind.parent()
            } else {
                "quarantine"
            },
            if present {
                &orphan.source
            } else {
                &destination
            },
            false,
        )?;
        Self::observation_matches(&actual, &orphan.observation, !present, &Policy::private()?)?;
        Ok(present)
    }
    pub(in crate::packages::tools::install::images) fn move_image_orphan(
        &self,
        orphan: &Orphan,
        mut guard: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        if !self.check_image_orphan(orphan)? {
            return Ok(());
        }
        let destination = orphan.destination();
        files::move_group_held(
            self.parent(orphan.kind.parent())?,
            &orphan.source,
            self.parent("quarantine")?,
            &destination,
            || {
                let actual = self.observe_orphan(orphan.kind.parent(), &orphan.source, true)?;
                Self::observation_matches(
                    &actual,
                    &orphan.observation,
                    false,
                    &Policy::private()?,
                )?;
                guard()
            },
        )?;
        if self.check_image_orphan(orphan)? {
            return Err(InstallError::StateInvalid);
        }
        Ok(())
    }
    pub(in crate::packages::tools::install::images) fn require_image_orphan_private(
        &self,
        orphan: &Orphan,
    ) -> Result<()> {
        if self.check_image_orphan(orphan)? {
            return Err(InstallError::StateInvalid);
        }
        let policy = Policy::private()?;
        let actual = self.observe_orphan("quarantine", &orphan.destination(), false)?;
        if actual
            .files
            .iter()
            .any(|f| f.security_digest != policy.digest(false))
            || actual
                .directories
                .iter()
                .any(|d| d.security_digest != policy.digest(true))
        {
            return Err(InstallError::ArtifactInvalid);
        }
        Ok(())
    }
    pub(in crate::packages::tools::install::images) fn privatize_image_orphan(
        &self,
        orphan: &Orphan,
        mut guard: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        if self.check_image_orphan(orphan)? {
            return Err(InstallError::StateInvalid);
        }
        let destination = orphan.destination();
        let policy = Policy::private()?;
        let root = self.group("quarantine", &destination, Open::Protect)?;
        let mut directories = BTreeMap::from([(String::new(), root)]);
        for observed in orphan
            .observation
            .directories
            .iter()
            .filter(|d| !d.pin.path.is_empty())
        {
            let (parent, leaf) = observed
                .pin
                .path
                .rsplit_once('/')
                .unwrap_or(("", &observed.pin.path));
            let handle = native::child(
                directories.get(parent).ok_or(InstallError::StateInvalid)?,
                leaf,
                Kind::Directory,
                Open::Protect,
            )?;
            directories.insert(observed.pin.path.clone(), handle);
        }
        // Protect leaves before parents. Otherwise native inheritance on an
        // interrupted, not-yet-protected stage could alter a child's observed
        // descriptor before its exact check. Retry accepts only original/private.
        for observed in &orphan.observation.files {
            let (parent, leaf) = Self::content_parent(&directories, &observed.pin.content.path)?;
            let file = native::child(parent, leaf, Kind::File, Open::Protect)?;
            let actual = file_observation(&observed.pin.content.path, &file)?;
            if actual.pin != observed.pin
                || actual.read_only != observed.read_only
                || (actual.security_digest != observed.security_digest
                    && actual.security_digest != policy.digest(false))
            {
                return Err(InstallError::ArtifactInvalid);
            }
            guard()?;
            policy.apply(&file, false)?;
        }
        for observed in orphan.observation.directories.iter().rev() {
            let file = directories
                .get(&observed.pin.path)
                .ok_or(InstallError::StateInvalid)?;
            let actual = directory_observation(&observed.pin.path, file)?;
            if actual.pin != observed.pin
                || (actual.security_digest != observed.security_digest
                    && actual.security_digest != policy.digest(true))
            {
                return Err(InstallError::ArtifactInvalid);
            }
            guard()?;
            policy.apply(file, true)?;
        }
        let actual = self.observe_orphan("quarantine", &destination, false)?;
        Self::observation_matches(&actual, &orphan.observation, true, &policy)?;
        if actual
            .files
            .iter()
            .any(|f| f.security_digest != policy.digest(false))
            || actual
                .directories
                .iter()
                .any(|d| d.security_digest != policy.digest(true))
        {
            return Err(InstallError::ArtifactInvalid);
        }
        Ok(())
    }
}
