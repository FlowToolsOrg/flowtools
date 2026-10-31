//! Durable, inert file plans. This store never renames or authorizes a file.
//! Private definitions use the existing DPAPI payload store; SQLite holds only
//! bounded metadata and borrows DataStore's single writer.
use super::{
    Definition, RenameDraft, RenamePreview, MAX_ENTRIES, MAX_FILES, MAX_FILE_BYTES, MAX_TOTAL_BYTES,
};
use crate::{catalog::digest, data::DataStore, private_jobs::PrivateJobs, protocol::ErrorCode};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const KEY: &str = "file-plans-v1";
const MAX_PLANS: usize = 64;
const MAX_STATE_BYTES: usize = 65_536;

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, specta::Type, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilePlanRecord {
    pub id: String,
    pub revision: u32,
    pub digest: String,
    pub files: u32,
    pub skipped: u32,
    pub total_bytes: u32,
    pub cancelled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilePlanRecovery {
    pub challenge: String,
    pub count: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
    record: FilePlanRecord,
    input_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    format_version: u16,
    profile_digest: String,
    recovery_generation: Option<String>,
    entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrivatePlan {
    format_version: u16,
    plan_id: String,
    definition: Definition,
}

fn valid_id(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}
fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn scope(store: &DataStore) -> Result<(String, Option<String>), ErrorCode> {
    let path = store.database_path().ok_or(ErrorCode::StorageFailed)?;
    let path = std::fs::canonicalize(path).map_err(|_| ErrorCode::StorageFailed)?;
    let profile = digest(&serde_json::json!(path
        .to_str()
        .ok_or(ErrorCode::StorageFailed)?));
    let generation: Option<String> = store
        .connection
        .query_row(
            "SELECT value FROM core_metadata WHERE key='recovery-generation'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::StorageFailed)?;
    if generation
        .as_deref()
        .is_some_and(|v| v.is_empty() || v.len() > 128)
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    Ok((profile, generation))
}
fn load(store: &DataStore, allow_restore: bool) -> Result<State, ErrorCode> {
    let text: Option<String> = store
        .connection
        .query_row(
            "SELECT value FROM core_metadata WHERE key=?1",
            [KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::StorageFailed)?;
    // A memory-only Runtime has no file plans and must still support orphan
    // retention. Creating a private plan always requires a disk profile.
    let Some(text) = text else {
        let (profile_digest, recovery_generation) = if store.database_path().is_some() {
            scope(store)?
        } else {
            (digest(&serde_json::json!("memory-only-file-plans")), None)
        };
        return Ok(State {
            format_version: 1,
            profile_digest,
            recovery_generation,
            entries: Vec::new(),
        });
    };
    if text.len() > MAX_STATE_BYTES {
        return Err(ErrorCode::StoreCorrupt);
    }
    let state: State = serde_json::from_str(&text).map_err(|_| ErrorCode::StoreCorrupt)?;
    if state.format_version != 1
        || !valid_digest(&state.profile_digest)
        || state
            .recovery_generation
            .as_deref()
            .is_some_and(|v| v.is_empty() || v.len() > 128)
        || state.entries.len() > MAX_PLANS
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    let mut ids = HashSet::new();
    let mut inputs = HashSet::new();
    for entry in &state.entries {
        let r = &entry.record;
        if !valid_id(&r.id)
            || !valid_id(&entry.input_id)
            || r.revision == 0
            || !valid_digest(&r.digest)
            || r.files as usize > MAX_FILES
            || r.skipped as usize > MAX_ENTRIES
            || u64::from(r.files) + u64::from(r.skipped) > MAX_ENTRIES as u64
            || r.total_bytes as u64 > MAX_TOTAL_BYTES
            || u64::from(r.total_bytes) > u64::from(r.files) * MAX_FILE_BYTES
            || (!r.cancelled && r.revision != 1)
            || (r.cancelled && r.revision < 2)
            || !ids.insert(&r.id)
            || !inputs.insert(&entry.input_id)
        {
            return Err(ErrorCode::StoreCorrupt);
        }
    }
    if !allow_restore {
        let current = scope(store)?;
        if state.profile_digest != current.0 || state.recovery_generation != current.1 {
            return Err(ErrorCode::RecoveryPending);
        }
    }
    Ok(state)
}
fn save(store: &mut DataStore, state: &State) -> Result<(), ErrorCode> {
    let text = serde_json::to_string(state).map_err(|_| ErrorCode::StorageFailed)?;
    if text.len() > MAX_STATE_BYTES {
        return Err(ErrorCode::BudgetExceeded);
    }
    let tx = store
        .connection
        .transaction()
        .map_err(|_| ErrorCode::StorageFailed)?;
    tx.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![KEY, text]).map_err(|_| ErrorCode::StorageFailed)?;
    tx.commit().map_err(|_| ErrorCode::StorageFailed)
}
fn private(store: &DataStore) -> Result<PrivateJobs, ErrorCode> {
    PrivateJobs::open(store.profile().ok_or(ErrorCode::StorageFailed)?)
}
fn definition(store: &DataStore, entry: &Entry) -> Result<RenameDraft, ErrorCode> {
    let payload: PrivatePlan =
        serde_json::from_value(private(store)?.read(&entry.input_id, "input")?)
            .map_err(|_| ErrorCode::StoreCorrupt)?;
    let d = &payload.definition;
    if payload.format_version != 1
        || payload.plan_id != entry.record.id
        || d.actions.len() > MAX_FILES
        || d.skipped as usize > MAX_ENTRIES
        || d.actions.iter().any(|a| a.pin.size > MAX_FILE_BYTES)
        || d.actions.iter().map(|a| a.pin.size).sum::<u64>() > MAX_TOTAL_BYTES
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    let draft = RenameDraft {
        definition: payload.definition,
    };
    let preview = draft.preview();
    if preview.digest != entry.record.digest
        || preview.files.len() as u32 != entry.record.files
        || preview.total_bytes != entry.record.total_bytes
        || preview.skipped != entry.record.skipped
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    Ok(draft)
}

/// T0 Rust API only. No client paths, grants, dispatch or write adapters.
pub struct FilePlanStore;
impl FilePlanStore {
    pub fn persist(
        store: &mut DataStore,
        request_key: &str,
        draft: RenameDraft,
    ) -> Result<FilePlanRecord, ErrorCode> {
        if !valid_id(request_key) {
            return Err(ErrorCode::InvalidRequest);
        }
        let mut state = load(store, false)?;
        let preview = draft.preview();
        if let Some(existing) = state
            .entries
            .iter()
            .find(|entry| entry.record.id == request_key)
        {
            if existing.record.digest != preview.digest {
                return Err(ErrorCode::IdempotencyConflict);
            }
            definition(store, existing)?;
            return Ok(existing.record.clone());
        }
        if state.entries.len() >= MAX_PLANS {
            return Err(ErrorCode::BudgetExceeded);
        }
        draft.recheck(store.profile().ok_or(ErrorCode::StorageFailed)?)?;
        let record = FilePlanRecord {
            id: request_key.into(),
            revision: 1,
            digest: preview.digest,
            files: preview.files.len() as u32,
            skipped: preview.skipped,
            total_bytes: preview.total_bytes,
            cancelled: false,
        };
        let input_id = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::to_value(PrivatePlan {
            format_version: 1,
            plan_id: record.id.clone(),
            definition: draft.definition,
        })
        .map_err(|_| ErrorCode::StorageFailed)?;
        // Private bytes are durable before publishing their reference. A failed
        // SQLite commit leaves an unreferenced encrypted blob, never a plan.
        private(store)?.write(&input_id, "input", &payload)?;
        state.entries.push(Entry {
            record: record.clone(),
            input_id,
        });
        save(store, &state)?;
        Ok(record)
    }
    pub fn list(store: &DataStore) -> Result<Vec<FilePlanRecord>, ErrorCode> {
        Ok(load(store, false)?
            .entries
            .into_iter()
            .map(|entry| entry.record)
            .collect())
    }
    pub fn preview(store: &DataStore, id: &str, revision: u32) -> Result<RenamePreview, ErrorCode> {
        Self::observe(store, id, revision, false)
    }
    pub fn recheck(store: &DataStore, id: &str, revision: u32) -> Result<RenamePreview, ErrorCode> {
        Self::observe(store, id, revision, true)
    }
    fn observe(
        store: &DataStore,
        id: &str,
        revision: u32,
        recheck: bool,
    ) -> Result<RenamePreview, ErrorCode> {
        let state = load(store, false)?;
        let entry = state
            .entries
            .iter()
            .find(|entry| entry.record.id == id)
            .ok_or(ErrorCode::InvalidRequest)?;
        if entry.record.revision != revision {
            return Err(ErrorCode::RevisionConflict);
        }
        if entry.record.cancelled {
            return Err(ErrorCode::InvalidRequest);
        }
        let draft = definition(store, entry)?;
        if recheck {
            draft.recheck(store.profile().ok_or(ErrorCode::StorageFailed)?)
        } else {
            Ok(draft.preview())
        }
    }
    pub fn cancel(
        store: &mut DataStore,
        id: &str,
        revision: u32,
    ) -> Result<FilePlanRecord, ErrorCode> {
        let mut state = load(store, false)?;
        let entry = state
            .entries
            .iter_mut()
            .find(|entry| entry.record.id == id)
            .ok_or(ErrorCode::InvalidRequest)?;
        if entry.record.revision != revision {
            return Err(ErrorCode::RevisionConflict);
        }
        if !entry.record.cancelled {
            entry.record.cancelled = true;
            entry.record.revision = entry
                .record
                .revision
                .checked_add(1)
                .ok_or(ErrorCode::BudgetExceeded)?;
            let record = entry.record.clone();
            save(store, &state)?;
            Ok(record)
        } else {
            Ok(entry.record.clone())
        }
    }
    /// Preservation only: validate all metadata even when scope is stale. Never
    /// accept old plans merely to make generic private-payload cleanup succeed.
    pub(crate) fn retained_input_ids(store: &DataStore) -> Result<HashSet<String>, ErrorCode> {
        Ok(load(store, true)?
            .entries
            .into_iter()
            .map(|entry| entry.input_id)
            .collect())
    }
    pub fn recovery(store: &DataStore) -> Result<Option<FilePlanRecovery>, ErrorCode> {
        let state = load(store, true)?;
        let current = scope(store)?;
        if state.profile_digest == current.0 && state.recovery_generation == current.1 {
            return Ok(None);
        }
        Ok(Some(FilePlanRecovery {
            challenge: digest(
                &serde_json::json!({"state": state, "profile": current.0, "generation": current.1}),
            ),
            count: state.entries.len() as u32,
        }))
    }
    /// Called only after explicit T0 management consent for this exact challenge.
    /// Disposes all old plans without filesystem effects or private-byte deletion.
    pub fn discard_restored(
        store: &mut DataStore,
        challenge: &str,
    ) -> Result<Vec<FilePlanRecord>, ErrorCode> {
        let recovery = Self::recovery(store)?.ok_or(ErrorCode::InvalidRequest)?;
        if recovery.challenge != challenge {
            return Err(ErrorCode::RevisionConflict);
        }
        let mut state = load(store, true)?;
        let current = scope(store)?;
        state.profile_digest = current.0;
        state.recovery_generation = current.1;
        for entry in &mut state.entries {
            entry.record.cancelled = true;
            entry.record.revision = entry
                .record
                .revision
                .checked_add(1)
                .ok_or(ErrorCode::BudgetExceeded)?;
        }
        save(store, &state)?;
        Ok(state
            .entries
            .into_iter()
            .map(|entry| entry.record)
            .collect())
    }
}

#[cfg(all(test, windows))]
mod tests;
