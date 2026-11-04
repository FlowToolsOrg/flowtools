//! Fixed, read-only inspection input. All opens below a held profile are single
//! component NT opens; unknown workspaces are retained and refuse new admission.
use super::super::{native, security::Policy};
use crate::{data::safe_path, packages::install::InstallError};
use native::{Kind, Open};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::Write,
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_GENERIC_READ, FILE_SHARE_READ, FILE_TRAVERSE,
};
type Result<T> = std::result::Result<T, InstallError>;
pub(super) const MAX_INPUT: usize = 1_048_576;
const NAMESPACE: &str = "native-media-inspection-v1";
const MAX_GROUPS: usize = 64;
pub(super) struct Workspace {
    path: PathBuf,
    _held: Vec<File>,
}
fn profile_guard(path: &Path) -> Result<File> {
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
fn uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}
impl Workspace {
    pub(super) fn create(
        profile: &Path,
        id: &str,
        sid: &str,
        bytes: &[u8],
        known: &BTreeSet<String>,
    ) -> Result<Self> {
        if !uuid(id) || !known.contains(id) || bytes.is_empty() || bytes.len() > MAX_INPUT {
            return Err(InstallError::StateInvalid);
        }
        let profile_file = profile_guard(profile)?;
        let private = Policy::private()?;
        let namespace_path = profile.join(NAMESPACE);
        let namespace = match fs::symlink_metadata(&namespace_path) {
            Ok(_) => {
                let file = native::child(&profile_file, NAMESPACE, Kind::Directory, Open::Read)?;
                private.verify(&file, true)?;
                file
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let file = native::child(&profile_file, NAMESPACE, Kind::Directory, Open::Create)?;
                private.apply(&file, true)?;
                file
            }
            Err(_) => return Err(InstallError::Storage),
        };
        // Whole namespace preflight, before creating this input. No adoption,
        // cleanup or follow-through on foreign/corrupt names or file objects.
        let mut groups = 0;
        let mut total = 0u64;
        for entry in fs::read_dir(&namespace_path).map_err(|_| InstallError::Storage)? {
            let entry = entry.map_err(|_| InstallError::Storage)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| InstallError::PathRejected)?;
            groups += 1;
            if groups >= MAX_GROUPS || !uuid(&name) || !known.contains(&name) {
                return Err(InstallError::RecoveryPending);
            }
            let directory = native::child(&namespace, &name, Kind::Directory, Open::Read)?;
            for (files, leaf) in fs::read_dir(entry.path())
                .map_err(|_| InstallError::Storage)?
                .enumerate()
            {
                let leaf = leaf.map_err(|_| InstallError::Storage)?;
                if files != 0 || leaf.file_name() != "input" {
                    return Err(InstallError::ArtifactInvalid);
                }
                let input = native::child(&directory, "input", Kind::File, Open::Read)?;
                let info = native::info(&input)?;
                let length = (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow);
                if length > MAX_INPUT as u64 {
                    return Err(InstallError::BudgetExceeded);
                }
                total = total
                    .checked_add(length)
                    .ok_or(InstallError::BudgetExceeded)?;
            }
        }
        if total + bytes.len() as u64 > MAX_INPUT as u64 * MAX_GROUPS as u64 {
            return Err(InstallError::BudgetExceeded);
        }
        let policy = Policy::inspection(sid)?;
        let directory = native::child(&namespace, id, Kind::Directory, Open::Create)?;
        policy.apply(&directory, true)?;
        let mut input = native::child(&directory, "input", Kind::File, Open::Create)?;
        input
            .write_all(bytes)
            .and_then(|_| input.sync_all())
            .map_err(|_| InstallError::Storage)?;
        policy.apply(&input, false)?;
        drop(input);
        let input = native::child(&directory, "input", Kind::File, Open::Read)?;
        policy.verify(&input, false)?;
        policy.verify(&directory, true)?;
        let info = native::info(&input)?;
        if (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow)
            != bytes.len() as u64
        {
            return Err(InstallError::ArtifactInvalid);
        }
        Ok(Self {
            path: namespace_path.join(id),
            _held: vec![profile_file, namespace, directory, input],
        })
    }
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}
