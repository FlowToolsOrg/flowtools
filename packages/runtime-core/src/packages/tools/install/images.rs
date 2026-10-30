//! Explicit private materialization of currently admitted installed tool bytes.
//! The resulting held files are inert: no tool lease, grant, activation or runner.
mod native;
pub mod reconciliation;
mod security;
mod tree;

use super::{
    invalid, profile, read, read_on, storage, uuid, value_digest, write, InstalledInertTool,
    Record as ArchiveRecord, Result, ToolArchivePin, ToolInstallError,
};
use crate::{
    data::DataStore,
    packages::{
        self,
        install::{files, InstallError},
        trust::PackageTrustStore,
    },
};
use rusqlite::TransactionBehavior;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs::File};

const KEY: &str = "tool-image-inventory-v1";
const PENDING: &str = "tool-image-materialization-pending-v1";
const MAX_RECORDS: usize = 64;
const MAX_CONTENT_FILES: usize = 33;
const MAX_DIRECTORIES: usize = 64;
const MAX_SEAL_BYTES: u64 = 32768;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Identity {
    volume: u32,
    file_id: String,
    creation: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Content {
    path: String,
    size: u64,
    sha256: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FilePin {
    content: Content,
    identity: Identity,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectoryPin {
    path: String,
    identity: Identity,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    pin: ToolArchivePin,
    source_artifact_id: String,
    seal_id: String,
    files: Vec<FilePin>,
    directories: Vec<DirectoryPin>,
    seal_file: Option<FilePin>,
    materialized_at: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Inventory {
    format_version: u16,
    profile_digest: String,
    records: Vec<Record>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    format_version: u16,
    id: String,
    before_digest: String,
    record: Record,
    prepared: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Seal<'a> {
    format_version: u16,
    profile_digest: &'a str,
    archive_digest: &'a str,
    source_artifact_id: &'a str,
    seal_id: &'a str,
    files: &'a [FilePin],
    directories: &'a [DirectoryPin],
}
fn seal(record: &Record, profile: &str) -> std::result::Result<Vec<u8>, InstallError> {
    let bytes = serde_json::to_vec(&Seal {
        format_version: 1,
        profile_digest: profile,
        archive_digest: &record.pin.archive_digest,
        source_artifact_id: &record.source_artifact_id,
        seal_id: &record.seal_id,
        files: &record.files,
        directories: &record.directories,
    })
    .map_err(|_| InstallError::StateInvalid)?;
    if bytes.len() as u64 > MAX_SEAL_BYTES {
        return Err(InstallError::BudgetExceeded);
    }
    Ok(bytes)
}
/// Only the private materialization transaction can construct these held files.
/// They do not expose paths/capability policy or authorize a native process.
/// ```compile_fail
/// use flowtools_runtime_core::packages::tools::install::images::MaterializedInertTool;
/// let forged=MaterializedInertTool { record:todo!(), held:vec![] };
/// ```
pub struct MaterializedInertTool {
    record: Record,
    _layout: tree::Layout,
    _source_layout: files::Layout,
    _held: Vec<File>,
}
impl MaterializedInertTool {
    pub fn pin(&self) -> ToolArchivePin {
        self.record.pin.clone()
    }
    pub fn materialized_at(&self) -> u64 {
        self.record.materialized_at
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MaterializePhase {
    JournalCommitted,
    StagePrepared,
    ArtifactPublished,
    BeforeCommit,
    Committed,
}

fn expected(source: &ArchiveRecord) -> Vec<Content> {
    let mut entries = source
        .payload
        .files
        .iter()
        .map(|f| Content {
            path: f.path.clone(),
            size: f.size,
            sha256: f.sha256.clone(),
        })
        .collect::<Vec<_>>();
    entries.push(Content {
        path: "manifest.json".into(),
        size: source.payload.manifest.size,
        sha256: source.payload.manifest.sha256.clone(),
    });
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}
fn identity_valid(id: &Identity) -> bool {
    let valid = |s: &str| {
        s.len() == 16
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    valid(&id.file_id) && valid(&id.creation)
}
fn validate_record(record: &Record, source: &ArchiveRecord, prepared: bool) -> Result<()> {
    if record.pin != source.pin()
        || record.source_artifact_id != source.artifact_id
        || !uuid(&record.seal_id)
        || record.materialized_at > packages::MAX_SAFE_INTEGER
    {
        return Err(invalid());
    }
    if !prepared {
        if !record.files.is_empty() || !record.directories.is_empty() || record.seal_file.is_some()
        {
            return Err(invalid());
        }
        return Ok(());
    }
    let expected = expected(source);
    if expected.len() > MAX_CONTENT_FILES
        || record.files.len() != expected.len()
        || record
            .files
            .iter()
            .map(|f| f.content.clone())
            .collect::<Vec<_>>()
            != expected
        || record.files.iter().any(|f| !identity_valid(&f.identity))
        || record.directories.is_empty()
        || record.directories.len() > MAX_DIRECTORIES
    {
        return Err(invalid());
    }
    let mut directories = BTreeSet::new();
    for directory in &record.directories {
        if (!directory.path.is_empty() && !packages::valid_path(&directory.path))
            || !identity_valid(&directory.identity)
            || !directories.insert(directory.path.as_str())
        {
            return Err(invalid());
        }
    }
    if !directories.contains("") {
        return Err(invalid());
    }
    let mut required_directories = BTreeSet::from([""]);
    for content in &expected {
        let mut parent = content.path.rsplit_once('/').map(|(p, _)| p);
        while let Some(path) = parent {
            if !directories.contains(path) {
                return Err(invalid());
            }
            required_directories.insert(path);
            parent = path.rsplit_once('/').map(|(p, _)| p);
        }
    }
    if directories != required_directories {
        return Err(invalid());
    }
    let Some(seal) = &record.seal_file else {
        return Err(invalid());
    };
    if seal.content.path != "seal.json"
        || seal.content.size > MAX_SEAL_BYTES
        || !packages::valid_digest(&seal.content.sha256)
        || !identity_valid(&seal.identity)
    {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn require_no_pending(
    connection: &rusqlite::Connection,
) -> std::result::Result<(), InstallError> {
    reconciliation::ensure_idle(connection)?;
    let present: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key=?1)",
            [PENDING],
            |r| r.get(0),
        )
        .map_err(|_| InstallError::Storage)?;
    if present {
        Err(InstallError::RecoveryPending)
    } else {
        Ok(())
    }
}

fn archive<'a>(state: &'a super::Inventory, record: &Record) -> Result<&'a ArchiveRecord> {
    state
        .records
        .iter()
        .find(|source| {
            source.artifact_id == record.source_artifact_id && source.pin() == record.pin
        })
        .ok_or_else(invalid)
}
fn inventory(store: &DataStore, source: &super::Inventory) -> Result<Inventory> {
    let state: Inventory = read(store, KEY)?.unwrap_or(Inventory {
        format_version: 1,
        profile_digest: profile(store)?,
        records: vec![],
    });
    if state.format_version != 1
        || state.profile_digest != profile(store)?
        || state.records.len() > MAX_RECORDS
    {
        return Err(invalid());
    }
    let mut hashes = BTreeSet::new();
    for record in &state.records {
        if !hashes.insert(record.pin.archive_digest.as_str()) {
            return Err(invalid());
        }
        validate_record(record, archive(source, record)?, true)?;
        let actual = seal(record, &state.profile_digest)?;
        let pin = record.seal_file.as_ref().ok_or_else(invalid)?;
        if pin.content.size != actual.len() as u64 || pin.content.sha256 != packages::hash(&actual)
        {
            return Err(invalid());
        }
    }
    Ok(state)
}
fn journal_on(
    connection: &rusqlite::Connection,
    state: &Inventory,
    source: &super::Inventory,
) -> Result<Option<Journal>> {
    let Some(pending) = read_on::<Journal>(connection, PENDING)? else {
        return Ok(None);
    };
    if pending.format_version != 1
        || !uuid(&pending.id)
        || pending.id != pending.record.seal_id
        || pending.before_digest != value_digest(state)?
        || pending.record.materialized_at != 0
        || state
            .records
            .iter()
            .any(|r| r.pin.archive_digest == pending.record.pin.archive_digest)
    {
        return Err(invalid());
    }
    validate_record(
        &pending.record,
        archive(source, &pending.record)?,
        pending.prepared,
    )?;
    Ok(Some(pending))
}
fn guard_on(
    connection: &rusqlite::Connection,
    state: &Inventory,
    source: &super::Inventory,
    pending: &Journal,
    digest: &str,
) -> Result<()> {
    if super::journal_on(connection, source)?.is_some() {
        return Err(InstallError::RecoveryPending.into());
    }
    let current_source: super::Inventory = read_on(connection, super::KEY)?.ok_or_else(invalid)?;
    let current: Inventory = read_on(connection, KEY)?.unwrap_or(Inventory {
        format_version: 1,
        profile_digest: state.profile_digest.clone(),
        records: vec![],
    });
    if value_digest(&current_source)? != value_digest(source)?
        || value_digest(&current)? != value_digest(state)?
    {
        return Err(invalid());
    }
    let actual = journal_on(connection, state, source)?.ok_or_else(invalid)?;
    if actual.id != pending.id || value_digest(&actual)? != digest {
        return Err(invalid());
    }
    Ok(())
}
// This is a short-lived compare guard, not a cached mutable root/floor or trust
// receipt. Current trust is fully re-admitted first. Freeze the exact trust and
// restore metadata across the final SQL writes too, including trigger failures.
pub(super) fn trust_stamp(connection: &rusqlite::Connection) -> Result<String> {
    let mut statement=connection.prepare("SELECT key, CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=16777216 THEN value ELSE NULL END FROM core_metadata WHERE key GLOB 'package-trust-*' OR key='recovery-generation' ORDER BY key").map_err(storage)?;
    let mut rows = statement.query([]).map_err(storage)?;
    let mut values = Vec::<(String, String)>::new();
    let mut total = 0usize;
    while let Some(row) = rows.next().map_err(storage)? {
        let key: String = row.get(0).map_err(storage)?;
        let value: String = row.get(1).map_err(storage)?;
        total = total
            .checked_add(key.len())
            .and_then(|n| n.checked_add(value.len()))
            .ok_or(InstallError::BudgetExceeded)?;
        if total > 33_554_432 || values.len() >= 16 {
            return Err(InstallError::BudgetExceeded.into());
        }
        values.push((key, packages::hash(value.as_bytes())));
    }
    value_digest(&values)
}
fn source_state(trust: &PackageTrustStore, store: &mut DataStore) -> Result<super::Inventory> {
    super::native()?;
    super::reconciliation::ensure_idle(&store.connection)?;
    let source = super::inventory(trust, store)?;
    if super::journal(store, &source)?.is_some() {
        return Err(InstallError::RecoveryPending.into());
    }
    Ok(source)
}
fn check_usage(
    store: &DataStore,
    layout: &tree::Layout,
    reserved: usize,
    extra: u64,
) -> Result<()> {
    let usage = layout.usage(reserved)?;
    let raw = files::Layout::open_for_tools(store, false)?
        .map(|l| l.disk_usage_reserving(0))
        .transpose()?
        .unwrap_or_default();
    let plugins = files::Layout::open(store, false)?
        .map(|l| l.disk_usage_reserving(0))
        .transpose()?
        .unwrap_or_default();
    if usage
        .groups
        .checked_add(raw.groups)
        .and_then(|n| n.checked_add(plugins.groups))
        .is_none_or(|n| n > files::MAX_GROUPS)
        || usage
            .bytes
            .checked_add(raw.bytes)
            .and_then(|n| n.checked_add(plugins.bytes))
            .and_then(|n| n.checked_add(extra))
            .is_none_or(|n| n > files::MAX_DISK_BYTES)
    {
        return Err(InstallError::BudgetExceeded.into());
    }
    Ok(())
}
pub fn materialize_inert_tool(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    installed: &InstalledInertTool,
    clock: impl FnMut() -> u64,
) -> Result<MaterializedInertTool> {
    materialize_with_hook(
        trust,
        store,
        installed,
        clock,
        |t, s, input, c| super::super::admit_tool_archive(t, s, input, c),
        |_, _| Ok(()),
    )
}
fn materialize_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    installed: &InstalledInertTool,
    mut clock: impl FnMut() -> u64,
    mut admission: impl FnMut(
        &PackageTrustStore,
        &mut DataStore,
        super::super::ToolArchiveInput<'_>,
        &mut dyn FnMut() -> u64,
    ) -> std::result::Result<
        super::super::InertToolArchive,
        super::super::ToolAdmissionError,
    >,
    mut hook: impl FnMut(MaterializePhase, &mut DataStore) -> Result<()>,
) -> Result<MaterializedInertTool> {
    require_no_pending(&store.connection)?;
    let source = source_state(trust, store)?;
    let source_record = source
        .records
        .iter()
        .find(|r| r.artifact_id == installed.record.artifact_id && r.pin() == installed.pin())
        .ok_or_else(invalid)?;
    if value_digest(source_record)? != value_digest(&installed.record)? {
        return Err(invalid());
    }
    let state = inventory(store, &source)?;
    if journal_on(&store.connection, &state, &source)?.is_some() {
        return Err(InstallError::RecoveryPending.into());
    }
    let source_layout =
        files::Layout::open_for_tools(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    source_layout.assert_owned(&source.records, None)?;
    let held_source = source_layout.hold(source_record)?;
    let envelope = packages::decode_base64(&source_record.envelope).map_err(|_| invalid())?;
    let input = super::super::ToolArchiveInput {
        envelope: &envelope,
        manifest: &held_source.manifest,
        archive: &held_source.archive,
    };
    let admitted = super::admit(
        trust,
        store,
        super::super::ToolArchiveInput { ..input },
        &mut clock,
        &mut admission,
    )?;
    if admitted.archive().files().contains_key("seal.json")
        || expected(source_record).len() > MAX_CONTENT_FILES
    {
        return Err(InstallError::PathRejected.into());
    }
    let old_layout = tree::Layout::open(store, false)?;
    if let Some(layout) = &old_layout {
        layout.assert_owned(&state.records, None)?;
        check_usage(store, layout, 0, 0)?;
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    if let Some(existing) = state.records.iter().find(|r| r.pin == installed.pin()) {
        let layout = old_layout.ok_or(InstallError::ArtifactInvalid)?;
        let mut held = layout.read(existing, &state.profile_digest)?;
        super::admit(
            trust,
            store,
            super::super::ToolArchiveInput { ..input },
            &mut clock,
            &mut admission,
        )?;
        held.extend(held_source.guards);
        return Ok(MaterializedInertTool {
            record: existing.clone(),
            _layout: layout,
            _source_layout: source_layout,
            _held: held,
        });
    }
    if state.records.len() >= MAX_RECORDS {
        return Err(InstallError::BudgetExceeded.into());
    }
    let layout = match old_layout {
        Some(layout) => layout,
        None => tree::Layout::open(store, true)?.ok_or(InstallError::ProfileRequired)?,
    };
    let mut pending = Journal {
        format_version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        before_digest: value_digest(&state)?,
        record: Record {
            pin: installed.pin(),
            source_artifact_id: source_record.artifact_id.clone(),
            seal_id: String::new(),
            files: vec![],
            directories: vec![],
            seal_file: None,
            materialized_at: 0,
        },
        prepared: false,
    };
    pending.record.seal_id = pending.id.clone();
    let mut extra = MAX_SEAL_BYTES;
    for bytes in admitted.archive().files().values() {
        extra = extra
            .checked_add(bytes.len() as u64)
            .ok_or(InstallError::BudgetExceeded)?;
    }
    check_usage(store, &layout, 1, extra)?;
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    write(&tx, PENDING, &pending)?;
    tx.commit().map_err(storage)?;
    hook(MaterializePhase::JournalCommitted, store)?;
    let writing = value_digest(&pending)?;
    guard_on(&store.connection, &state, &source, &pending, &writing)?;
    let prepared_record =
        layout.stage(&pending, admitted.archive().files(), &state.profile_digest)?;
    validate_record(&prepared_record, source_record, true)?;
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, &state, &source, &pending, &writing)?;
    pending.record = prepared_record;
    pending.prepared = true;
    write(&tx, PENDING, &pending)?;
    tx.commit().map_err(storage)?;
    let prepared = value_digest(&pending)?;
    hook(MaterializePhase::StagePrepared, store)?;
    guard_on(&store.connection, &state, &source, &pending, &prepared)?;
    layout.publish(&pending, &state.profile_digest)?;
    hook(MaterializePhase::ArtifactPublished, store)?;
    hook(MaterializePhase::BeforeCommit, store)?;
    source_layout.read(source_record)?;
    let fresh = super::admit(
        trust,
        store,
        super::super::ToolArchiveInput { ..input },
        &mut clock,
        &mut admission,
    )?;
    let mut held = layout.read(&pending.record, &state.profile_digest)?;
    layout.assert_owned(&state.records, Some(&pending))?;
    check_usage(store, &layout, 0, 0)?;
    let now = clock();
    trust
        .check_receipts(
            store,
            &[fresh.archive().descriptor()],
            &source_record.payload.target,
            now,
        )
        .map_err(|e| ToolInstallError::Admission(e.into()))?;
    let stamp = trust_stamp(&store.connection)?;
    guard_on(&store.connection, &state, &source, &pending, &prepared)?;
    let mut record = pending.record.clone();
    record.materialized_at = now;
    let mut records = state.records.clone();
    records.push(record.clone());
    let next = Inventory {
        format_version: 1,
        profile_digest: state.profile_digest.clone(),
        records,
    };
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, &state, &source, &pending, &prepared)?;
    if trust_stamp(&tx)? != stamp {
        return Err(invalid());
    }
    write(&tx, KEY, &next)?;
    tx.execute("DELETE FROM core_metadata WHERE key=?1", [PENDING])
        .map_err(storage)?;
    let written: Inventory = read_on(&tx, KEY)?.ok_or_else(invalid)?;
    let raw_source: super::Inventory = read_on(&tx, super::KEY)?.ok_or_else(invalid)?;
    if value_digest(&written)? != value_digest(&next)?
        || read_on::<Journal>(&tx, PENDING)?.is_some()
        || value_digest(&raw_source)? != value_digest(&source)?
        || super::journal_on(&tx, &source)?.is_some()
        || trust_stamp(&tx)? != stamp
    {
        return Err(invalid());
    }
    tx.commit().map_err(storage)?;
    hook(MaterializePhase::Committed, store)?;
    held.extend(held_source.guards);
    Ok(MaterializedInertTool {
        record,
        _layout: layout,
        _source_layout: source_layout,
        _held: held,
    })
}
pub fn abort_inert_tool_materialization(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<bool> {
    reconciliation::ensure_idle(&store.connection)?;
    let source = source_state(trust, store)?;
    let state = inventory(store, &source)?;
    let Some(pending) = journal_on(&store.connection, &state, &source)? else {
        return Ok(false);
    };
    let digest = value_digest(&pending)?;
    let stamp = trust_stamp(&store.connection)?;
    if let Some(layout) = tree::Layout::open(store, false)? {
        layout.assert_owned(&state.records, Some(&pending))?;
        layout.quarantine(
            &pending,
            &state.profile_digest,
            &expected(archive(&source, &pending.record)?),
        )?;
    } else if pending.prepared {
        return Err(InstallError::ArtifactInvalid.into());
    }
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, &state, &source, &pending, &digest)?;
    if trust_stamp(&tx)? != stamp {
        return Err(invalid());
    }
    tx.execute("DELETE FROM core_metadata WHERE key=?1", [PENDING])
        .map_err(storage)?;
    let written: Inventory = read_on(&tx, KEY)?.unwrap_or(Inventory {
        format_version: 1,
        profile_digest: state.profile_digest.clone(),
        records: vec![],
    });
    let raw_source: super::Inventory = read_on(&tx, super::KEY)?.ok_or_else(invalid)?;
    if read_on::<Journal>(&tx, PENDING)?.is_some()
        || value_digest(&written)? != value_digest(&state)?
        || value_digest(&raw_source)? != value_digest(&source)?
        || super::journal_on(&tx, &source)?.is_some()
        || trust_stamp(&tx)? != stamp
    {
        return Err(invalid());
    }
    tx.commit().map_err(storage)?;
    Ok(true)
}
pub fn inspect_inert_tool_images(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<Vec<ToolArchivePin>> {
    require_no_pending(&store.connection)?;
    let source = source_state(trust, store)?;
    let state = inventory(store, &source)?;
    if let Some(layout) = tree::Layout::open(store, false)? {
        layout.assert_owned(&state.records, None)?;
        check_usage(store, &layout, 0, 0)?;
        for record in &state.records {
            layout.read(record, &state.profile_digest)?;
        }
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    Ok(state.records.iter().map(|r| r.pin.clone()).collect())
}
// A raw ZIP quarantine must never strand a committed image reference or proceed
// through unrecognized image bytes. This validates existing images only; it
// neither adopts nor quarantines an image orphan.
pub(super) fn reconciliation_preflight(store: &DataStore, source: &super::Inventory) -> Result<()> {
    require_no_pending(&store.connection)?;
    let state = inventory(store, source)?;
    if let Some(layout) = tree::Layout::open(store, false)? {
        layout.assert_owned(&state.records, None)?;
        check_usage(store, &layout, 0, 0)?;
        for record in &state.records {
            layout.read(record, &state.profile_digest)?;
        }
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    Ok(())
}
pub(crate) fn retained_usage(
    store: &DataStore,
) -> std::result::Result<files::DiskUsage, InstallError> {
    Ok(tree::Layout::open(store, false)?
        .map(|l| l.usage(0))
        .transpose()?
        .unwrap_or_default())
}
#[cfg(test)]
pub(crate) fn materialize_fixture_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    installed: &InstalledInertTool,
    clock: impl FnMut() -> u64,
    hook: impl FnMut(MaterializePhase, &mut DataStore) -> Result<()>,
) -> Result<MaterializedInertTool> {
    materialize_with_hook(
        trust,
        store,
        installed,
        clock,
        |t, s, input, c| super::super::admit_fixture(t, s, input, c),
        hook,
    )
}
