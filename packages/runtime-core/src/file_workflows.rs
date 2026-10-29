//! Read-only T0 admission for bounded, explicitly selected file rename plans.
//! A draft is not a grant, job, recovery receipt or executable identity.
use crate::{catalog::digest, protocol::ErrorCode};
use serde::{Deserialize, Serialize};
use std::path::Path;

const MAX_ENTRIES: usize = 512;
const MAX_FILES: usize = 128;
const MAX_FILE_BYTES: u64 = 67_108_864;
const MAX_TOTAL_BYTES: u64 = 268_435_456;

pub mod store;
#[cfg(windows)]
mod windows;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RootPin {
    volume_id: String,
    file_id: String,
    path_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FilePin {
    volume_id: String,
    file_id: String,
    size: u64,
    modified: String,
    created: String,
    sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Action {
    source: String,
    target: String,
    pin: FilePin,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Definition {
    root_path: String,
    root: RootPin,
    prefix: String,
    actions: Vec<Action>,
    skipped: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenamePreviewFile {
    pub source: String,
    pub target: String,
    pub bytes: u32,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenamePreview {
    pub digest: String,
    pub files: Vec<RenamePreviewFile>,
    pub skipped: u32,
    pub total_bytes: u32,
}
/// Opaque, inert and not serializable. Only T0 Rust selects OS directories.
pub struct RenameDraft {
    definition: Definition,
}
impl RenameDraft {
    pub fn preview(&self) -> RenamePreview {
        RenamePreview {
            digest: digest(&serde_json::to_value(&self.definition).expect("bounded native plan")),
            files: self
                .definition
                .actions
                .iter()
                .map(|action| RenamePreviewFile {
                    source: action.source.clone(),
                    target: action.target.clone(),
                    bytes: action.pin.size as u32,
                    sha256: action.pin.sha256.clone(),
                })
                .collect(),
            skipped: self.definition.skipped,
            total_bytes: self
                .definition
                .actions
                .iter()
                .map(|action| action.pin.size as u32)
                .sum(),
        }
    }
    /// Reopens only the original selected directory and compares every current
    /// source identity/content and target before reporting a fresh observation.
    /// It performs no filesystem or storage writes.
    pub fn recheck(&self, protected_profile: &Path) -> Result<RenamePreview, ErrorCode> {
        #[cfg(windows)]
        {
            self.preflight(protected_profile)?;
            Ok(self.preview())
        }
        #[cfg(not(windows))]
        {
            let _ = protected_profile;
            Err(ErrorCode::CapabilityUndeclared)
        }
    }
    #[cfg(windows)]
    fn preflight(&self, protected: &Path) -> Result<Preflight, ErrorCode> {
        let root =
            windows::Directory::open(Path::new(&self.definition.root_path), Some(protected))?;
        if root.pin != self.definition.root {
            return Err(ErrorCode::RevisionConflict);
        }
        let mut sources = Vec::new();
        for action in &self.definition.actions {
            let source = root
                .source(&action.source)?
                .ok_or(ErrorCode::RevisionConflict)?;
            if source.pin != action.pin || !root.absent(&action.target)? {
                return Err(ErrorCode::RevisionConflict);
            }
            sources.push(source);
        }
        Ok(Preflight { root, sources })
    }
}
#[cfg(windows)]
struct Preflight {
    root: windows::Directory,
    sources: Vec<windows::HeldSource>,
}
#[cfg(windows)]
impl Drop for Preflight {
    fn drop(&mut self) {
        // Keep the directory and pinned ancestors until all source handles close.
        self.sources.clear();
        let _ = &self.root;
    }
}

/// Read-only admission. Call only after a trusted native directory selection or
/// an explicit local management selection. No client path flags grant access.
pub fn prepare_rename(
    directory: &Path,
    prefix: &str,
    protected_profile: &Path,
) -> Result<RenameDraft, ErrorCode> {
    #[cfg(windows)]
    {
        if prefix.is_empty()
            || prefix.encode_utf16().count() > 32
            || prefix.chars().any(|c| {
                c.is_control()
                    || "\\/:*?\"<>|".contains(c)
                    || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            })
        {
            return Err(ErrorCode::InputInvalid);
        }
        let root = windows::Directory::open(directory, Some(protected_profile))?;
        let mut actions = Vec::new();
        let mut held = Vec::new();
        let mut total = 0u64;
        let mut skipped = 0;
        for name in root.entries()? {
            if name.starts_with(prefix) {
                skipped += 1;
                continue;
            }
            if actions.len() >= MAX_FILES {
                return Err(ErrorCode::BudgetExceeded);
            }
            let source = root.source(&name)?.ok_or(ErrorCode::RevisionConflict)?;
            let target = format!("{prefix}{name}");
            windows::component(&target)?;
            if !root.absent(&target)? {
                return Err(ErrorCode::RevisionConflict);
            }
            total = total
                .checked_add(source.pin.size)
                .ok_or(ErrorCode::BudgetExceeded)?;
            if total > MAX_TOTAL_BYTES {
                return Err(ErrorCode::BudgetExceeded);
            }
            actions.push(Action {
                source: name,
                target,
                pin: source.pin.clone(),
            });
            held.push(source);
        }
        root.validate()?;
        let root_path = directory.to_str().ok_or(ErrorCode::ScopeDenied)?.to_owned();
        Ok(RenameDraft {
            definition: Definition {
                root_path,
                root: root.pin.clone(),
                prefix: prefix.into(),
                actions,
                skipped,
            },
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (directory, prefix, protected_profile);
        Err(ErrorCode::CapabilityUndeclared)
    }
}

#[cfg(all(test, windows))]
mod tests;
