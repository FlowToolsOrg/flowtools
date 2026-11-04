//! Fixed T0 rename/undo journal. Consent is checked at every effect; no overwrite.
use super::*;
use crate::file_workflows::FilePin;

const EXECUTION_KEY: &str = "file-executions-v1";

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, specta::Type, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum FileExecutionPhase {
    Applying,
    Complete,
    Undoing,
    Undone,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileExecutionRecord {
    pub plan_id: String,
    pub phase: FileExecutionPhase,
    pub completed: u32,
    pub pending: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionEntry {
    record: FileExecutionRecord,
    plan_digest: String,
    receipt_id: String,
    receipt_digest: String,
    #[serde(skip)]
    retired_receipts: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    format_version: u16,
    profile_digest: String,
    recovery_generation: Option<String>,
    entries: Vec<ExecutionEntry>,
    #[serde(skip)]
    predecessor: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    format_version: u16,
    plan_id: String,
    pins: Vec<FilePin>,
}

fn journal(store: &DataStore, allow_restore: bool) -> Result<Journal, ErrorCode> {
    let raw = bounded_text(&store.connection, EXECUTION_KEY, MAX_STATE_BYTES)?;
    let Some(raw) = raw else {
        let state = load(store, true)?;
        return Ok(Journal {
            format_version: 1,
            profile_digest: state.profile_digest,
            recovery_generation: state.recovery_generation,
            entries: Vec::new(),
            predecessor: None,
        });
    };
    let mut journal: Journal = serde_json::from_value(
        crate::packages::strict_json(raw.as_bytes()).map_err(|_| ErrorCode::StoreCorrupt)?,
    )
    .map_err(|_| ErrorCode::StoreCorrupt)?;
    journal.predecessor = Some(raw);
    let mut ids = HashSet::new();
    let mut receipts = HashSet::new();
    if journal.format_version != 1
        || !valid_digest(&journal.profile_digest)
        || journal.entries.len() > MAX_PLANS
        || journal
            .recovery_generation
            .as_deref()
            .is_some_and(|g| g.is_empty() || g.len() > 128)
        || journal.entries.iter().any(|e| {
            !valid_id(&e.record.plan_id)
                || !valid_id(&e.receipt_id)
                || !valid_digest(&e.plan_digest)
                || !valid_digest(&e.receipt_digest)
                || !ids.insert(&e.record.plan_id)
                || !receipts.insert(&e.receipt_id)
                || e.record.completed as usize > MAX_FILES
                || (matches!(
                    e.record.phase,
                    FileExecutionPhase::Complete | FileExecutionPhase::Undone
                ) && e.record.pending)
                || (e.record.phase == FileExecutionPhase::Undone && e.record.completed != 0)
        })
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    if !allow_restore
        && scope(store)?
            != (
                journal.profile_digest.clone(),
                journal.recovery_generation.clone(),
            )
    {
        return Err(ErrorCode::RecoveryPending);
    }
    Ok(journal)
}
fn save_journal(
    store: &mut DataStore,
    state: &State,
    journal: &mut Journal,
) -> Result<(), ErrorCode> {
    let text = serde_json::to_string(journal).map_err(|_| ErrorCode::StorageFailed)?;
    if text.len() > MAX_STATE_BYTES {
        return Err(ErrorCode::BudgetExceeded);
    }
    let tx = store
        .connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| ErrorCode::StorageFailed)?;
    if bounded_text(&tx, KEY, MAX_STATE_BYTES)? != state.predecessor
        || bounded_text(&tx, EXECUTION_KEY, MAX_STATE_BYTES)? != journal.predecessor
        || bounded_text(&tx, "recovery-generation", 128)? != journal.recovery_generation
    {
        return Err(ErrorCode::RevisionConflict);
    }
    let before = tx.total_changes();
    let rows = tx.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![EXECUTION_KEY, text]).map_err(|_| ErrorCode::StorageFailed)?;
    if rows != 1
        || tx.total_changes() != before + 1
        || bounded_text(&tx, EXECUTION_KEY, MAX_STATE_BYTES)?.as_deref() != Some(&text)
        || bounded_text(&tx, KEY, MAX_STATE_BYTES)? != state.predecessor
        || bounded_text(&tx, "recovery-generation", 128)? != journal.recovery_generation
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
    journal.predecessor = Some(text);
    // Only a confirmed replacement may discard its superseded private receipt.
    // Unknown commits retain both versions and never delete private bytes.
    for entry in &mut journal.entries {
        for id in entry.retired_receipts.drain(..) {
            private(store)?.remove(&id, "input")?;
        }
    }
    Ok(())
}
pub(super) fn discard_scope(store: &mut DataStore, state: &State) -> Result<(), ErrorCode> {
    let mut journal = journal(store, true)?;
    journal.profile_digest = state.profile_digest.clone();
    journal.recovery_generation = state.recovery_generation.clone();
    let plan_text = serde_json::to_string(state).map_err(|_| ErrorCode::StorageFailed)?;
    let journal_text = serde_json::to_string(&journal).map_err(|_| ErrorCode::StorageFailed)?;
    if plan_text.len() > MAX_STATE_BYTES || journal_text.len() > MAX_STATE_BYTES {
        return Err(ErrorCode::BudgetExceeded);
    }
    let tx = store
        .connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| ErrorCode::StorageFailed)?;
    if bounded_text(&tx, KEY, MAX_STATE_BYTES)? != state.predecessor
        || bounded_text(&tx, EXECUTION_KEY, MAX_STATE_BYTES)? != journal.predecessor
        || bounded_text(&tx, "recovery-generation", 128)? != state.recovery_generation
    {
        return Err(ErrorCode::RevisionConflict);
    }
    for (key, text) in [(KEY, &plan_text), (EXECUTION_KEY, &journal_text)] {
        let before = tx.total_changes();
        let rows=tx.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",rusqlite::params![key,text]).map_err(|_|ErrorCode::StorageFailed)?;
        if rows != 1
            || tx.total_changes() != before + 1
            || bounded_text(&tx, key, MAX_STATE_BYTES)?.as_deref() != Some(text)
        {
            return Err(ErrorCode::StoreCorrupt);
        }
    }
    if bounded_text(&tx, KEY, MAX_STATE_BYTES)?.as_deref() != Some(&plan_text)
        || bounded_text(&tx, "recovery-generation", 128)? != state.recovery_generation
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    tx.commit().map_err(|_| ErrorCode::StorageFailed)
}
pub(super) fn ensure_unstarted(store: &DataStore, id: &str) -> Result<(), ErrorCode> {
    if journal(store, false)?
        .entries
        .iter()
        .any(|e| e.record.plan_id == id)
    {
        return Err(ErrorCode::RevisionConflict);
    }
    Ok(())
}
pub(crate) fn retained_input_ids(store: &DataStore) -> Result<HashSet<String>, ErrorCode> {
    Ok(journal(store, true)?
        .entries
        .into_iter()
        .map(|e| e.receipt_id)
        .collect())
}
pub fn records(store: &DataStore) -> Result<Vec<FileExecutionRecord>, ErrorCode> {
    Ok(journal(store, false)?
        .entries
        .into_iter()
        .map(|e| e.record)
        .collect())
}
fn write_receipt(
    store: &DataStore,
    entry: &mut ExecutionEntry,
    receipt: &Receipt,
) -> Result<(), ErrorCode> {
    let value = serde_json::to_value(receipt).map_err(|_| ErrorCode::StorageFailed)?;
    let id = uuid::Uuid::new_v4().to_string();
    private(store)?.write(&id, "input", &value)?;
    if !entry.receipt_id.is_empty() {
        entry.retired_receipts.push(entry.receipt_id.clone());
    }
    entry.receipt_id = id;
    entry.receipt_digest = digest(&value);
    Ok(())
}
fn same_file(left: &FilePin, right: &FilePin) -> bool {
    let mut expected = right.clone();
    expected.created = left.created.clone(); // NTFS tunneling after an intended rename.
    *left == expected
}

/// The authenticated T0 Host confirms this exact plan digest. This function is
/// crate-private: neither plugin code nor a caller-provided path is authority.
#[cfg(windows)]
pub(crate) fn execute(
    store: &mut DataStore,
    id: &str,
    revision: u32,
    confirmation: &str,
    undo: bool,
    mut authorize: impl FnMut() -> Result<(), ErrorCode>,
) -> Result<FileExecutionRecord, ErrorCode> {
    use crate::file_workflows::windows::Directory;
    authorize()?;
    let state = load(store, false)?;
    let plan = state
        .entries
        .iter()
        .find(|e| e.record.id == id)
        .ok_or(ErrorCode::InvalidRequest)?;
    if plan.record.cancelled
        || plan.record.revision != revision
        || plan.record.digest != confirmation
    {
        return Err(ErrorCode::RevisionConflict);
    }
    let draft = definition(store, plan)?;
    let d = &draft.definition;
    let root = Directory::open(std::path::Path::new(&d.root_path), store.profile())?;
    if root.pin != d.root {
        return Err(ErrorCode::RevisionConflict);
    }
    let mut journal = journal(store, false)?;
    let index = match journal.entries.iter().position(|e| e.record.plan_id == id) {
        Some(index) => index,
        None if undo => return Err(ErrorCode::InvalidRequest),
        None => {
            if journal.entries.len() >= MAX_PLANS {
                return Err(ErrorCode::BudgetExceeded);
            }
            draft.recheck(store.profile().ok_or(ErrorCode::StorageFailed)?)?;
            let mut entry = ExecutionEntry {
                record: FileExecutionRecord {
                    plan_id: id.into(),
                    phase: FileExecutionPhase::Applying,
                    completed: 0,
                    pending: false,
                },
                plan_digest: confirmation.into(),
                receipt_id: String::new(),
                receipt_digest: String::new(),
                retired_receipts: Vec::new(),
            };
            write_receipt(
                store,
                &mut entry,
                &Receipt {
                    format_version: 1,
                    plan_id: id.into(),
                    pins: d.actions.iter().map(|a| a.pin.clone()).collect(),
                },
            )?;
            journal.entries.push(entry);
            save_journal(store, &state, &mut journal)?;
            journal.entries.len() - 1
        }
    };
    let entry = &journal.entries[index];
    if entry.plan_digest != confirmation {
        return Err(ErrorCode::RevisionConflict);
    }
    let value = private(store)?.read(&entry.receipt_id, "input")?;
    if digest(&value) != entry.receipt_digest {
        return Err(ErrorCode::StoreCorrupt);
    }
    let mut receipt: Receipt =
        serde_json::from_value(value).map_err(|_| ErrorCode::StoreCorrupt)?;
    if receipt.format_version != 1
        || receipt.plan_id != id
        || receipt.pins.len() != d.actions.len()
        || entry.record.completed as usize > d.actions.len()
        || receipt
            .pins
            .iter()
            .zip(&d.actions)
            .any(|(p, a)| !same_file(p, &a.pin))
        || (entry.record.phase == FileExecutionPhase::Complete
            && entry.record.completed as usize != d.actions.len())
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    if (!undo && entry.record.phase == FileExecutionPhase::Complete)
        || (undo && entry.record.phase == FileExecutionPhase::Undone)
    {
        return Ok(entry.record.clone());
    }
    if !undo
        && matches!(
            entry.record.phase,
            FileExecutionPhase::Undoing | FileExecutionPhase::Undone
        )
    {
        return Err(ErrorCode::InvalidRequest);
    }
    // An in-flight step must first be reconciled in its ORIGINAL direction.
    if undo && entry.record.phase == FileExecutionPhase::Applying && entry.record.pending {
        return Err(ErrorCode::RevisionConflict);
    }
    if undo && entry.record.phase != FileExecutionPhase::Undoing {
        journal.entries[index].record.phase = FileExecutionPhase::Undoing;
        save_journal(store, &state, &mut journal)?;
    }
    // Full-batch preflight holds every actual source against writers/deleters.
    let record = &journal.entries[index].record;
    let completed = record.completed as usize;
    let pending_index = if record.pending {
        if undo {
            completed.checked_sub(1)
        } else {
            Some(completed)
        }
    } else {
        None
    };
    let mut held = Vec::new();
    let mut recovered = false;
    for (n, action) in d.actions.iter().enumerate() {
        let at_target = n < completed;
        let (name, other) = if at_target {
            (&action.target, &action.source)
        } else {
            (&action.source, &action.target)
        };
        if let Some(source) = root.source(name)? {
            if source.pin != receipt.pins[n] || !root.absent(other)? {
                return Err(ErrorCode::RevisionConflict);
            }
            held.push(source);
        } else if pending_index == Some(n) {
            let source = root.source(other)?.ok_or(ErrorCode::RevisionConflict)?;
            if !same_file(&source.pin, &receipt.pins[n]) {
                return Err(ErrorCode::RevisionConflict);
            }
            receipt.pins[n] = source.pin.clone();
            held.push(source);
            recovered = true;
        } else {
            return Err(ErrorCode::RevisionConflict);
        }
    }
    root.validate()?;
    if recovered {
        let entry = &mut journal.entries[index];
        entry.record.completed = if undo {
            entry.record.completed - 1
        } else {
            entry.record.completed + 1
        };
        entry.record.pending = false;
        write_receipt(store, entry, &receipt)?;
        save_journal(store, &state, &mut journal)?;
    }
    loop {
        let completed = journal.entries[index].record.completed as usize;
        let step = if undo {
            completed.checked_sub(1)
        } else if completed < d.actions.len() {
            Some(completed)
        } else {
            None
        };
        let Some(step) = step else {
            break;
        };
        authorize()?;
        journal.entries[index].record.pending = true;
        save_journal(store, &state, &mut journal)?; // Durable intent precedes the effect.
        authorize()?;
        let action = &d.actions[step];
        let target = if undo { &action.source } else { &action.target };
        receipt.pins[step] = root.rename(&held[step], target)?;
        let entry = &mut journal.entries[index];
        entry.record.completed = if undo {
            entry.record.completed - 1
        } else {
            entry.record.completed + 1
        };
        entry.record.pending = false;
        write_receipt(store, entry, &receipt)?;
        save_journal(store, &state, &mut journal)?;
    }
    journal.entries[index].record.phase = if undo {
        FileExecutionPhase::Undone
    } else {
        FileExecutionPhase::Complete
    };
    save_journal(store, &state, &mut journal)?;
    Ok(journal.entries[index].record.clone())
}

#[cfg(all(test, windows))]
mod tests;
