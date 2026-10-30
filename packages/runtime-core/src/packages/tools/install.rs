//! Windows T0 inert signed-tool ZIP storage. No extraction, registry of runnable
//! programs, accepted-job lease, activation or executable identity.
pub mod images;
pub mod reconciliation;
use super::super::{
    self as packages, manifest::ManifestCompatibility, trust::PackageTrustStore, PackageArch,
    PackageKind, PackagePayload, PackagePlatform, PackageTarget,
};
use super::{
    admit_tool_archive, InertToolArchive, ToolAdmissionError, ToolArchiveInput, ToolProgram,
};
use crate::{
    catalog::digest,
    data::DataStore,
    packages::install::{
        files::{self, BlobJournal, BlobRecord},
        InstallError,
    },
};
use base64::{engine::general_purpose, Engine};
use rusqlite::{OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const KEY: &str = "tool-archive-inventory-v1";
const PENDING: &str = "tool-archive-install-pending-v1";
const MAX_RECORDS: usize = 64;
const MAX_STATE_BYTES: usize = 16_777_216;

#[derive(Debug, PartialEq, Eq)]
pub enum ToolInstallError {
    Admission(ToolAdmissionError),
    Storage(InstallError),
}
impl From<ToolAdmissionError> for ToolInstallError {
    fn from(e: ToolAdmissionError) -> Self {
        Self::Admission(e)
    }
}
impl From<InstallError> for ToolInstallError {
    fn from(e: InstallError) -> Self {
        Self::Storage(e)
    }
}
type Result<T> = std::result::Result<T, ToolInstallError>;
fn invalid() -> ToolInstallError {
    InstallError::StateInvalid.into()
}
fn storage(_: rusqlite::Error) -> ToolInstallError {
    InstallError::Storage.into()
}
fn value_digest(value: &impl Serialize) -> Result<String> {
    Ok(digest(&serde_json::to_value(value).map_err(|_| invalid())?))
}
fn uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}

/// Diagnostic signed archive identity; independent of G4 semantic ToolPin. This
/// metadata is constructible and never accepted as an executable capability.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolArchivePin {
    pub publisher: String,
    pub id: String,
    pub version: String,
    pub target: PackageTarget,
    pub build_flavor: String,
    pub archive_digest: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    payload: PackagePayload,
    envelope: String,
    root_digest: String,
    installed_at: u64,
    artifact_id: String,
}
impl Record {
    fn pin(&self) -> ToolArchivePin {
        ToolArchivePin {
            publisher: self.payload.publisher.clone(),
            id: self.payload.id.clone(),
            version: self.payload.version.clone(),
            target: self.payload.target.clone(),
            build_flavor: self.payload.build_flavor.clone(),
            archive_digest: self.payload.archive.sha256.clone(),
        }
    }
}
impl BlobRecord for Record {
    fn payload(&self) -> &PackagePayload {
        &self.payload
    }
    fn artifact_id(&self) -> &str {
        &self.artifact_id
    }
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
    new_artifact: bool,
    prepared: bool,
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
/// A committed inert ZIP receipt, never a runnable tool capability.
/// ```compile_fail
/// use flowtools_runtime_core::packages::tools::install::InstalledInertTool;
/// let forged = InstalledInertTool { record: todo!(), programs: vec![] };
/// ```
pub struct InstalledInertTool {
    record: Record,
    programs: Vec<ToolProgram>,
}
impl InstalledInertTool {
    pub fn pin(&self) -> ToolArchivePin {
        self.record.pin()
    }
    pub fn programs(&self) -> &[ToolProgram] {
        &self.programs
    }
    pub fn installed_at(&self) -> u64 {
        self.record.installed_at
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolInstallPhase {
    JournalCommitted,
    StagePrepared,
    ArtifactPublished,
    BeforeCommit,
    Committed,
}

fn profile(store: &DataStore) -> Result<String> {
    let path = store.database_path().ok_or(InstallError::ProfileRequired)?;
    let path = std::fs::canonicalize(path).map_err(|_| InstallError::Storage)?;
    Ok(digest(&serde_json::json!(path
        .to_str()
        .ok_or(InstallError::PathRejected)?)))
}
fn read<T: serde::de::DeserializeOwned>(store: &DataStore, key: &str) -> Result<Option<T>> {
    read_on(&store.connection, key)
}
fn read_on<T: serde::de::DeserializeOwned>(
    connection: &rusqlite::Connection,
    key: &str,
) -> Result<Option<T>> {
    // Bound the value inside SQLite before allocating its String. A malformed
    // existing row yields NULL/conversion refusal, never an absent-state reset.
    let text:Option<String>=connection.query_row(
        "SELECT CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=?2 THEN value ELSE NULL END FROM core_metadata WHERE key=?1",
        rusqlite::params![key,MAX_STATE_BYTES as i64],|r|r.get(0)).optional().map_err(|_|invalid())?;
    text.map(|text| {
        let value = packages::strict_json(text.as_bytes()).map_err(|_| invalid())?;
        serde_json::from_value(value).map_err(|_| invalid())
    })
    .transpose()
}
fn write(tx: &rusqlite::Transaction<'_>, key: &str, value: &impl Serialize) -> Result<()> {
    let text = serde_json::to_string(value).map_err(|_| invalid())?;
    if text.len() > MAX_STATE_BYTES {
        return Err(InstallError::BudgetExceeded.into());
    }
    tx.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key,text]).map_err(storage)?;
    Ok(())
}
fn validate_record(record: &Record) -> Result<()> {
    packages::validate_package(&record.payload).map_err(|_| invalid())?;
    if record.payload.kind != PackageKind::Tool
        || record.payload.target.platform != PackagePlatform::Windows
        || record.payload.target.arch != PackageArch::X64
        || record.payload.files.len() > super::MAX_TOOL_FILES
        || record.payload.manifest.size > super::MAX_TOOL_MANIFEST_BYTES as u64
        || !uuid(&record.artifact_id)
        || !packages::valid_digest(&record.root_digest)
        || record.installed_at > packages::MAX_SAFE_INTEGER
        || record.envelope.len() > packages::MAX_ENVELOPE_BYTES.div_ceil(3) * 4
    {
        return Err(invalid());
    }
    let bytes = packages::decode_base64(&record.envelope).map_err(|_| invalid())?;
    let envelope =
        packages::decode_envelope(&bytes, packages::PACKAGE_PAYLOAD_TYPE).map_err(|_| invalid())?;
    let payload = packages::strict_json(&envelope.payload).map_err(|_| invalid())?;
    if payload != serde_json::to_value(&record.payload).map_err(|_| invalid())? {
        return Err(invalid());
    }
    Ok(())
}
fn inventory(trust: &PackageTrustStore, store: &mut DataStore) -> Result<Inventory> {
    let blocked:bool=store.connection.query_row("SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key IN ('package-install-pending-v1','package-reconciliation-pending-v1'))",[],|r|r.get(0)).map_err(storage)?;
    if blocked {
        return Err(InstallError::RecoveryPending.into());
    }
    let snapshot = trust
        .snapshot(store)
        .map_err(|e| ToolInstallError::Admission(e.into()))?;
    let state: Inventory = read(store, KEY)?.unwrap_or(Inventory {
        format_version: 1,
        profile_digest: profile(store)?,
        records: vec![],
    });
    if state.format_version != 1
        || state.records.len() > MAX_RECORDS
        || state.profile_digest != profile(store)?
    {
        return Err(invalid());
    }
    let mut identities = BTreeSet::new();
    for record in &state.records {
        validate_record(record)?;
        if !identities.insert((
            record.payload.publisher.clone(),
            record.payload.id.clone(),
            record.payload.version.clone(),
            serde_json::to_string(&record.payload.target).map_err(|_| invalid())?,
        )) || !snapshot.known_versions.iter().any(|k| {
            k.kind == PackageKind::Tool
                && k.publisher == record.payload.publisher
                && k.id == record.payload.id
                && k.version == record.payload.version
                && k.target == record.payload.target
                && k.package_digest == record.payload.archive.sha256
                && k.release_sequence == record.payload.release_sequence
        }) {
            return Err(invalid());
        }
    }
    Ok(state)
}
fn journal(store: &DataStore, state: &Inventory) -> Result<Option<Journal>> {
    journal_on(&store.connection, state)
}
fn journal_on(connection: &rusqlite::Connection, state: &Inventory) -> Result<Option<Journal>> {
    let Some(pending) = read_on::<Journal>(connection, PENDING)? else {
        return Ok(None);
    };
    validate_record(&pending.record)?;
    if pending.format_version != 1
        || !uuid(&pending.id)
        || pending.before_digest != value_digest(state)?
        || (pending.new_artifact && pending.id != pending.record.artifact_id)
        || (pending.new_artifact
            && state
                .records
                .iter()
                .any(|r| r.payload.archive.sha256 == pending.record.payload.archive.sha256))
    {
        return Err(invalid());
    }
    if !pending.new_artifact
        && !state
            .records
            .iter()
            .any(|r| r.pin() == pending.record.pin() && r.artifact_id == pending.record.artifact_id)
    {
        return Err(invalid());
    }
    Ok(Some(pending))
}
fn guard(store: &DataStore, state: &Inventory, pending: &Journal, expected: &str) -> Result<()> {
    guard_on(&store.connection, state, pending, expected)
}
fn guard_on(
    connection: &rusqlite::Connection,
    state: &Inventory,
    pending: &Journal,
    expected: &str,
) -> Result<()> {
    let current = journal_on(connection, state)?.ok_or_else(invalid)?;
    if value_digest(&current)? != expected {
        return Err(invalid());
    }
    let actual: Inventory = read_on(connection, KEY)?.unwrap_or(Inventory {
        format_version: 1,
        profile_digest: state.profile_digest.clone(),
        records: vec![],
    });
    if value_digest(&actual)? != value_digest(state)? || current.id != pending.id {
        return Err(invalid());
    }
    Ok(())
}
fn native() -> Result<()> {
    let target = ManifestCompatibility::embedded()
        .map_err(|e| ToolInstallError::Admission(e.into()))?
        .target;
    if std::env::consts::ARCH != "x86_64"
        || target.platform != PackagePlatform::Windows
        || target.arch != PackageArch::X64
    {
        return Err(InstallError::Unsupported.into());
    }
    Ok(())
}
fn admit(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    input: ToolArchiveInput<'_>,
    clock: &mut impl FnMut() -> u64,
    admission: impl FnOnce(
        &PackageTrustStore,
        &mut DataStore,
        ToolArchiveInput<'_>,
        &mut dyn FnMut() -> u64,
    ) -> std::result::Result<InertToolArchive, ToolAdmissionError>,
) -> Result<InertToolArchive> {
    Ok(admission(trust, store, input, clock)?)
}

/// T0 native Host must already reserve/protect this profile. This separate
/// explicit transaction re-admits raw signed bytes before IO and commit. The
/// public empty production review profile still refuses before creating a pool.
pub fn install_inert_tool(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    input: ToolArchiveInput<'_>,
    clock: impl FnMut() -> u64,
) -> Result<InstalledInertTool> {
    install_with_hook(
        trust,
        store,
        input,
        clock,
        |t, s, b, c| admit_tool_archive(t, s, b, c),
        |_, _| Ok(()),
    )
}
fn install_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    input: ToolArchiveInput<'_>,
    mut clock: impl FnMut() -> u64,
    mut admission: impl FnMut(
        &PackageTrustStore,
        &mut DataStore,
        ToolArchiveInput<'_>,
        &mut dyn FnMut() -> u64,
    ) -> std::result::Result<InertToolArchive, ToolAdmissionError>,
    mut hook: impl FnMut(ToolInstallPhase, &mut DataStore) -> Result<()>,
) -> Result<InstalledInertTool> {
    native()?;
    reconciliation::ensure_idle(&store.connection)?;
    images::require_no_pending(&store.connection)?;
    let state = inventory(trust, store)?;
    if journal(store, &state)?.is_some() {
        return Err(InstallError::RecoveryPending.into());
    }
    let candidate = admit(
        trust,
        store,
        ToolArchiveInput { ..input },
        &mut clock,
        &mut admission,
    )?;
    let mut layout = files::Layout::open_for_tools(store, false)?;
    if let Some(layout) = &layout {
        layout.assert_owned(&state.records, None)?;
        layout.disk_bytes()?;
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    let payload = candidate.archive().descriptor().payload().clone();
    let mut record = Record {
        payload,
        envelope: general_purpose::STANDARD.encode(input.envelope),
        root_digest: candidate.archive().descriptor().root_digest().into(),
        installed_at: 0,
        artifact_id: uuid::Uuid::new_v4().to_string(),
    };
    let position = state.records.iter().position(|r| r.pin() == record.pin());
    if position.is_none() && state.records.len() >= MAX_RECORDS {
        return Err(InstallError::BudgetExceeded.into());
    }
    let new_artifact = position.is_none();
    if let Some(index) = position {
        record.artifact_id = state.records[index].artifact_id.clone();
    }
    // Reserve the bounded inventory shape before any journal or disk publication.
    let size = serde_json::to_vec(&record).map_err(|_| invalid())?.len()
        + serde_json::to_vec(&state).map_err(|_| invalid())?.len()
        + 16;
    if size > MAX_STATE_BYTES {
        return Err(InstallError::BudgetExceeded.into());
    }
    if let Some(layout) = &layout {
        if layout.has_artifact(&record)? == new_artifact {
            return Err(InstallError::ArtifactInvalid.into());
        }
        if !new_artifact {
            layout.read(&record)?;
            let index = position.ok_or_else(invalid)?;
            if state.records[index].envelope == record.envelope {
                // A repeated installation observes current trust again after
                // disk verification but keeps the original installation record.
                let fresh = admit(
                    trust,
                    store,
                    ToolArchiveInput { ..input },
                    &mut clock,
                    &mut admission,
                )?;
                return Ok(InstalledInertTool {
                    record: state.records[index].clone(),
                    programs: fresh.programs().to_vec(),
                });
            }
        }
    }
    if layout.is_none() {
        layout = files::Layout::open_for_tools(store, true)?;
    }
    let mut layout = layout.ok_or(InstallError::ProfileRequired)?;
    let extra = if new_artifact {
        record.payload.manifest.size + record.payload.archive.size + 512
    } else {
        0
    };
    let other_usage = files::Layout::open(store, false)?
        .map(|l| l.disk_usage_reserving(0))
        .transpose()?
        .unwrap_or_default()
        .combine(images::retained_usage(store)?)?;
    let usage = layout.disk_usage_reserving(usize::from(new_artifact))?;
    if usage
        .groups
        .checked_add(other_usage.groups)
        .is_none_or(|n| n > files::MAX_GROUPS)
    {
        return Err(InstallError::BudgetExceeded.into());
    }
    if usage
        .bytes
        .checked_add(other_usage.bytes)
        .and_then(|n| n.checked_add(extra))
        .is_none_or(|n| n > files::MAX_DISK_BYTES)
    {
        return Err(InstallError::BudgetExceeded.into());
    }
    let mut pending = Journal {
        format_version: 1,
        id: if new_artifact {
            record.artifact_id.clone()
        } else {
            uuid::Uuid::new_v4().to_string()
        },
        before_digest: value_digest(&state)?,
        record,
        new_artifact,
        prepared: false,
    };
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    write(&tx, PENDING, &pending)?;
    tx.commit().map_err(storage)?;
    hook(ToolInstallPhase::JournalCommitted, store)?;
    let writing = value_digest(&pending)?;
    guard(store, &state, &pending, &writing)?;
    if new_artifact {
        layout.stage_bytes(&pending, input.manifest, input.archive)?;
    }
    // Compare and update under the same single writer transaction.
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, &state, &pending, &writing)?;
    pending.prepared = true;
    write(&tx, PENDING, &pending)?;
    tx.commit().map_err(storage)?;
    let prepared = value_digest(&pending)?;
    hook(ToolInstallPhase::StagePrepared, store)?;
    guard(store, &state, &pending, &prepared)?;
    if new_artifact {
        layout.publish(&pending)?;
    }
    hook(ToolInstallPhase::ArtifactPublished, store)?;
    hook(ToolInstallPhase::BeforeCommit, store)?;
    let fresh = admit(
        trust,
        store,
        ToolArchiveInput { ..input },
        &mut clock,
        &mut admission,
    )?;
    if value_digest(fresh.archive().descriptor().payload())?
        != value_digest(&pending.record.payload)?
    {
        return Err(invalid());
    }
    layout.read(&pending.record)?;
    layout.assert_owned(&state.records, Some(&pending))?;
    guard(store, &state, &pending, &prepared)?;
    pending.record.root_digest = fresh.archive().descriptor().root_digest().into();
    pending.record.installed_at = clock();
    // Sample and recheck the final T0 time, never assign a later unverified clock.
    trust
        .check_receipts(
            store,
            &[fresh.archive().descriptor()],
            &pending.record.payload.target,
            pending.record.installed_at,
        )
        .map_err(|e| ToolInstallError::Admission(e.into()))?;
    let receipt = InstalledInertTool {
        record: pending.record.clone(),
        programs: fresh.programs().to_vec(),
    };
    let before = state;
    let mut state = Inventory {
        format_version: 1,
        profile_digest: before.profile_digest.clone(),
        records: before.records.clone(),
    };
    if let Some(index) = position {
        state.records[index] = pending.record.clone();
    } else {
        state.records.push(pending.record.clone());
    }
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, &before, &pending, &prepared)?;
    write(&tx, KEY, &state)?;
    tx.execute("DELETE FROM core_metadata WHERE key=?1", [PENDING])
        .map_err(storage)?;
    let committed: Inventory = read_on(&tx, KEY)?.ok_or_else(invalid)?;
    if value_digest(&committed)? != value_digest(&state)?
        || read_on::<Journal>(&tx, PENDING)?.is_some()
    {
        return Err(invalid());
    }
    tx.commit().map_err(storage)?;
    hook(ToolInstallPhase::Committed, store)?;
    Ok(receipt)
}

/// Explicit abort only quarantines journal-owned bytes; it never adopts,
/// activates, lowers trust floors or resumes an uncertain installation.
pub fn abort_inert_tool_install(trust: &PackageTrustStore, store: &mut DataStore) -> Result<bool> {
    native()?;
    reconciliation::ensure_idle(&store.connection)?;
    images::require_no_pending(&store.connection)?;
    let state = inventory(trust, store)?;
    let Some(pending) = journal(store, &state)? else {
        return Ok(false);
    };
    let expected = value_digest(&pending)?;
    if let Some(mut layout) = files::Layout::open_for_tools(store, false)? {
        layout.assert_owned(&state.records, Some(&pending))?;
        layout.quarantine(&pending)?;
    } else if pending.prepared {
        return Err(InstallError::ArtifactInvalid.into());
    }
    guard(store, &state, &pending, &expected)?;
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, &state, &pending, &expected)?;
    tx.execute("DELETE FROM core_metadata WHERE key=?1", [PENDING])
        .map_err(storage)?;
    tx.commit().map_err(storage)?;
    Ok(true)
}

pub fn inspect_inert_tools(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<Vec<ToolArchivePin>> {
    native()?;
    reconciliation::ensure_idle(&store.connection)?;
    images::require_no_pending(&store.connection)?;
    let state = inventory(trust, store)?;
    if journal(store, &state)?.is_some() {
        return Err(InstallError::RecoveryPending.into());
    }
    if let Some(layout) = files::Layout::open_for_tools(store, false)? {
        layout.assert_owned(&state.records, None)?;
        layout.disk_bytes()?;
        for record in &state.records {
            layout.read(record)?;
        }
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    Ok(state.records.iter().map(Record::pin).collect())
}

#[cfg(test)]
pub(crate) fn install_fixture_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    input: ToolArchiveInput<'_>,
    clock: impl FnMut() -> u64,
    hook: impl FnMut(ToolInstallPhase, &mut DataStore) -> Result<()>,
) -> Result<InstalledInertTool> {
    install_with_hook(
        trust,
        store,
        input,
        clock,
        |t, s, b, c| super::admit_fixture(t, s, b, c),
        hook,
    )
}

// A pending journal of any validity blocks unrelated installation/inspection.
// Presence only ever refuses; it never adopts, cleans or repairs its contents.
pub(crate) fn require_no_pending_tool_transaction(
    connection: &rusqlite::Connection,
) -> std::result::Result<(), InstallError> {
    reconciliation::ensure_idle(connection)?;
    images::require_no_pending(connection)?;
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
pub(crate) fn retained_tool_usage(
    store: &DataStore,
) -> std::result::Result<files::DiskUsage, InstallError> {
    files::Layout::open_for_tools(store, false)?
        .map(|l| l.disk_usage_reserving(0))
        .transpose()?
        .unwrap_or_default()
        .combine(images::retained_usage(store)?)
}
