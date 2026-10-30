//! Windows/x64 T0 inert package inventory and abort-only process-crash recovery.
//! No production installer, activation, health result, grant or execution identity.
use super::{
    archive::validate_archive,
    dependencies::{
        plan_signed_plugin, SignedArchiveLock, SignedArchivePin, SignedDependencyError,
        SignedDependencyRequest,
    },
    manifest::{validate_plugin_manifest, CompatiblePluginArchive, ManifestCompatibility},
    trust::{DescriptorVerification, PackageTrustStore},
    *,
};
use crate::{
    data::DataStore,
    dependencies::{DependencyError, DependencyIdentity},
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

pub(crate) mod files;
pub(crate) mod reconciliation;
pub use reconciliation::{
    abort_inert_reconciliation, apply_inert_reconciliation, plan_inert_reconciliation,
    resume_inert_reconciliation, InertReconciliationPlan, InertReconciliationSummary,
};
#[cfg(test)]
pub(super) use reconciliation::{apply_with_hook as reconcile_with_hook, ReconciliationPhase};
const KEY: &str = "package-install-inventory-v1";
const PENDING: &str = "package-install-pending-v1";
const MAX_RECORDS: usize = 128;
const MAX_STATE_BYTES: usize = 16_777_216;

#[derive(Debug, PartialEq, Eq)]
pub enum InstallError {
    Package(PackageError),
    Dependency(DependencyError),
    Storage,
    StateInvalid,
    ArtifactInvalid,
    PathRejected,
    RecoveryPending,
    ProfileRequired,
    Unsupported,
    BudgetExceeded,
    InventoryChanged,
}
impl From<PackageError> for InstallError {
    fn from(error: PackageError) -> Self {
        Self::Package(error)
    }
}
impl From<SignedDependencyError> for InstallError {
    fn from(error: SignedDependencyError) -> Self {
        match error {
            SignedDependencyError::Package(e) => Self::Package(e),
            SignedDependencyError::Dependency(e) => Self::Dependency(e),
        }
    }
}
/// Borrowed raw bytes assembled by T0. Neither paths nor validation flags enter.
#[derive(Clone, Copy)]
pub struct InertPackageBytes<'a> {
    pub envelope: &'a [u8],
    pub manifest: &'a [u8],
    pub archive: &'a [u8],
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    payload: PackagePayload,
    envelope: String,
    lock: SignedArchiveLock,
    root_digest: String,
    installed_at: u64,
    artifact_id: String,
}
impl Record {
    fn pin(&self) -> SignedArchivePin {
        SignedArchivePin {
            publisher: self.payload.publisher.clone(),
            id: self.payload.id.clone(),
            version: self.payload.version.clone(),
            target: self.payload.target.clone(),
            package_digest: self.payload.archive.sha256.clone(),
        }
    }
}
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Inventory {
    format_version: u16,
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
/// Inert committed metadata only. This receipt cannot launch an installed ZIP.
pub struct InstalledInertPackage {
    record: Record,
}
impl InstalledInertPackage {
    pub fn pin(&self) -> SignedArchivePin {
        self.record.pin()
    }
    pub fn dependency_lock(&self) -> &SignedArchiveLock {
        &self.record.lock
    }
    pub fn installed_at(&self) -> u64 {
        self.record.installed_at
    }
}
pub struct InertInventorySnapshot {
    pins: Vec<SignedArchivePin>,
    locks: Vec<SignedArchiveLock>,
}
impl InertInventorySnapshot {
    pub fn packages(&self) -> &[SignedArchivePin] {
        &self.pins
    }
    pub fn locks(&self) -> &[SignedArchiveLock] {
        &self.locks
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InstallPhase {
    JournalCommitted,
    StagePrepared,
    ArtifactPublished,
    BeforeCommit,
    Committed,
}

fn json_digest<T: Serialize>(value: &T) -> Result<String, InstallError> {
    Ok(crate::catalog::digest(
        &serde_json::to_value(value).map_err(|_| InstallError::StateInvalid)?,
    ))
}
fn native(target: &PackageTarget) -> Result<(), InstallError> {
    if std::env::consts::ARCH != "x86_64"
        || target.platform != PackagePlatform::Windows
        || target.arch != PackageArch::X64
    {
        return Err(InstallError::Unsupported);
    }
    Ok(())
}
fn read<T: serde::de::DeserializeOwned>(
    connection: &rusqlite::Connection,
    key: &str,
) -> Result<Option<T>, InstallError> {
    let length: Option<i64> = connection
        .query_row(
            "SELECT length(CAST(value AS BLOB)) FROM core_metadata WHERE key=?1",
            [key],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| InstallError::Storage)?;
    match length {
        None => Ok(None),
        Some(n) if n < 0 || n > MAX_STATE_BYTES as i64 => Err(InstallError::StateInvalid),
        Some(_) => {
            let text: String = connection
                .query_row("SELECT value FROM core_metadata WHERE key=?1", [key], |r| {
                    r.get(0)
                })
                .map_err(|_| InstallError::Storage)?;
            let value = strict_json(text.as_bytes()).map_err(|_| InstallError::StateInvalid)?;
            Ok(Some(
                serde_json::from_value(value).map_err(|_| InstallError::StateInvalid)?,
            ))
        }
    }
}
fn write<T: Serialize>(
    connection: &rusqlite::Connection,
    key: &str,
    value: &T,
) -> Result<(), InstallError> {
    let text = serde_json::to_string(value).map_err(|_| InstallError::StateInvalid)?;
    if text.len() > MAX_STATE_BYTES {
        return Err(InstallError::BudgetExceeded);
    }
    connection.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,text]).map_err(|_| InstallError::Storage)?;
    Ok(())
}
fn validate_record(record: &Record) -> Result<(), InstallError> {
    validate_package(&record.payload).map_err(|_| InstallError::StateInvalid)?;
    native(&record.payload.target)?;
    if record.payload.kind != PackageKind::Plugin
        || !uuid::Uuid::parse_str(&record.artifact_id)
            .is_ok_and(|id| id.to_string() == record.artifact_id)
        || !valid_digest(&record.root_digest)
        || record.installed_at > MAX_SAFE_INTEGER
        || record.lock.format_version != 1
        || record
            .lock
            .verified_digest()
            .map_err(|_| InstallError::StateInvalid)?
            != record.lock.digest
        || record.lock.roots != vec![record.pin().identity()]
        || !record.lock.packages.contains(&record.pin())
        || !record.lock.tools.is_empty()
        || serde_json::to_value(&record.lock.target).map_err(|_| InstallError::StateInvalid)?
            != serde_json::to_value(&record.payload.target)
                .map_err(|_| InstallError::StateInvalid)?
    {
        return Err(InstallError::StateInvalid);
    }
    if record.envelope.len() > MAX_ENVELOPE_BYTES.div_ceil(3) * 4 {
        return Err(InstallError::StateInvalid);
    }
    let bytes = decode_base64(&record.envelope).map_err(|_| InstallError::StateInvalid)?;
    let envelope =
        decode_envelope(&bytes, PACKAGE_PAYLOAD_TYPE).map_err(|_| InstallError::StateInvalid)?;
    let payload = strict_json(&envelope.payload).map_err(|_| InstallError::StateInvalid)?;
    if payload != serde_json::to_value(&record.payload).map_err(|_| InstallError::StateInvalid)? {
        return Err(InstallError::StateInvalid);
    }
    Ok(())
}
fn inventory(connection: &rusqlite::Connection) -> Result<Inventory, InstallError> {
    let state: Inventory = read(connection, KEY)?.unwrap_or(Inventory {
        format_version: 2,
        records: vec![],
    });
    if state.format_version != 2 || state.records.len() > MAX_RECORDS {
        return Err(InstallError::StateInvalid);
    }
    let mut versions = HashSet::new();
    for record in &state.records {
        validate_record(record)?;
        if !versions.insert((
            record.payload.publisher.clone(),
            record.payload.id.clone(),
            record.payload.version.clone(),
            record.payload.target.clone(),
        )) {
            return Err(InstallError::StateInvalid);
        }
        for pin in &record.lock.packages {
            if !state
                .records
                .iter()
                .any(|r| r.pin() == *pin && r.payload.target == record.payload.target)
            {
                return Err(InstallError::StateInvalid);
            }
        }
    }
    Ok(state)
}
fn observed_inventory(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<Inventory, InstallError> {
    super::tools::install::require_no_pending_tool_transaction(&store.connection)?;
    let snapshot = trust.snapshot(store)?;
    let state = inventory(&store.connection)?;
    for record in &state.records {
        let payload = &record.payload;
        if !snapshot.known_versions.iter().any(|known| {
            known.kind == payload.kind
                && known.publisher == payload.publisher
                && known.id == payload.id
                && known.version == payload.version
                && known.target == payload.target
                && known.release_sequence == payload.release_sequence
                && known.package_digest == payload.archive.sha256
        }) {
            return Err(InstallError::StateInvalid);
        }
    }
    Ok(state)
}
fn journal(
    connection: &rusqlite::Connection,
    state: &Inventory,
) -> Result<Option<Journal>, InstallError> {
    let Some(value) = read::<Journal>(connection, PENDING)? else {
        return Ok(None);
    };
    validate_record(&value.record)?;
    if value.format_version != 2
        || (value.new_artifact && value.id != value.record.artifact_id)
        || !uuid::Uuid::parse_str(&value.id).is_ok_and(|id| id.to_string() == value.id)
        || value.before_digest != json_digest(state)?
        || (value.new_artifact
            && state
                .records
                .iter()
                .any(|r| r.payload.archive.sha256 == value.record.payload.archive.sha256))
        || (!value.new_artifact
            && !state
                .records
                .iter()
                .any(|r| r.payload.archive.sha256 == value.record.payload.archive.sha256))
        || value
            .record
            .lock
            .packages
            .iter()
            .any(|pin| *pin != value.record.pin() && !state.records.iter().any(|r| r.pin() == *pin))
    {
        return Err(InstallError::StateInvalid);
    }
    Ok(Some(value))
}
fn guard_pending(
    connection: &rusqlite::Connection,
    expected: &Journal,
    digest: &str,
) -> Result<(), InstallError> {
    let current = inventory(connection)?;
    if json_digest(&current)? != expected.before_digest {
        return Err(InstallError::InventoryChanged);
    }
    let actual = journal(connection, &current)?.ok_or(InstallError::InventoryChanged)?;
    if json_digest(&actual)? != digest {
        return Err(InstallError::InventoryChanged);
    }
    Ok(())
}
fn admit(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    bytes: InertPackageBytes<'_>,
    compatibility: &ManifestCompatibility,
    now: u64,
) -> Result<CompatiblePluginArchive, InstallError> {
    let descriptor = trust.verify_descriptor(
        store,
        DescriptorVerification {
            envelope: bytes.envelope,
            manifest: bytes.manifest,
            archive: bytes.archive,
            target: compatibility.target.clone(),
            now,
        },
    )?;
    Ok(validate_plugin_manifest(
        validate_archive(&descriptor, bytes.manifest, bytes.archive)?,
        compatibility,
    )?)
}
fn resolve(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    candidate: &CompatiblePluginArchive,
    state: &Inventory,
    layout: Option<&files::Layout>,
    compatibility: &ManifestCompatibility,
    now: u64,
) -> Result<super::dependencies::VerifiedPluginDependencyPlan, InstallError> {
    let root = &candidate.archive().descriptor().payload();
    let root_identity = SignedArchivePin {
        publisher: root.publisher.clone(),
        id: root.id.clone(),
        version: root.version.clone(),
        target: root.target.clone(),
        package_digest: root.archive.sha256.clone(),
    }
    .identity();
    let mut latest: BTreeMap<DependencyIdentity, &Record> = BTreeMap::new();
    for record in &state.records {
        let identity = record.pin().identity();
        if identity == root_identity || record.payload.target != compatibility.target {
            continue;
        }
        if latest.get(&identity).is_none_or(|previous| {
            previous.payload.release_sequence < record.payload.release_sequence
        }) {
            latest.insert(identity, record);
        }
    }
    let mut queue = VecDeque::from([candidate.manifest().clone()]);
    let mut seen = BTreeSet::from([root_identity]);
    let mut providers = Vec::new();
    let mut selected_records = Vec::new();
    let mut manifest_bytes = root.manifest.size;
    let mut expanded_bytes =
        root.manifest.size + root.files.iter().map(|file| file.size).sum::<u64>();
    while let Some(manifest) = queue.pop_front() {
        for dependency in manifest["dependencies"]["services"]
            .as_array()
            .ok_or(InstallError::StateInvalid)?
        {
            let identity = DependencyIdentity {
                publisher: dependency["publisher"]
                    .as_str()
                    .ok_or(InstallError::StateInvalid)?
                    .into(),
                id: dependency["id"]
                    .as_str()
                    .ok_or(InstallError::StateInvalid)?
                    .into(),
            };
            if !seen.insert(identity.clone()) {
                continue;
            }
            let Some(record) = latest.get(&identity) else {
                continue;
            };
            manifest_bytes += record.payload.manifest.size;
            expanded_bytes += record.payload.manifest.size
                + record
                    .payload
                    .files
                    .iter()
                    .map(|file| file.size)
                    .sum::<u64>();
            if manifest_bytes > 16_777_216 || expanded_bytes > 134_217_728 {
                return Err(InstallError::BudgetExceeded);
            }
            let (manifest, archive) = layout.ok_or(InstallError::ArtifactInvalid)?.read(*record)?;
            let envelope =
                decode_base64(&record.envelope).map_err(|_| InstallError::StateInvalid)?;
            let provider = admit(
                trust,
                store,
                InertPackageBytes {
                    envelope: &envelope,
                    manifest: &manifest,
                    archive: &archive,
                },
                compatibility,
                now,
            )?;
            if json_digest(provider.archive().descriptor().payload())?
                != json_digest(&record.payload)?
            {
                return Err(InstallError::StateInvalid);
            }
            queue.push_back(provider.manifest().clone());
            providers.push(provider);
            selected_records.push(*record);
        }
    }
    let plan = plan_signed_plugin(
        trust,
        store,
        SignedDependencyRequest {
            candidate,
            providers: &providers.iter().collect::<Vec<_>>(),
            compatibility,
            now,
        },
    )?;
    if selected_records.iter().any(|record| {
        record
            .lock
            .packages
            .iter()
            .any(|pin| !plan.lock().packages.contains(pin))
    }) {
        return Err(InstallError::Dependency(DependencyError::VersionConflict));
    }
    Ok(plan)
}

/// The native Host must reserve and protect DataStore's profile first. The T0
/// clock is sampled again just before commit; clients cannot supply a clock.
pub fn install_inert_plugin(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    bytes: InertPackageBytes<'_>,
    compatibility: &ManifestCompatibility,
    clock: impl FnMut() -> u64,
) -> Result<InstalledInertPackage, InstallError> {
    install_with_hook(trust, store, bytes, compatibility, clock, |_, _| Ok(()))
}
pub(super) fn install_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    bytes: InertPackageBytes<'_>,
    compatibility: &ManifestCompatibility,
    mut clock: impl FnMut() -> u64,
    mut hook: impl FnMut(InstallPhase, &mut DataStore) -> Result<(), InstallError>,
) -> Result<InstalledInertPackage, InstallError> {
    native(&compatibility.target)?;
    reconciliation::ensure_idle(&store.connection)?;
    let mut state = observed_inventory(trust, store)?;
    if journal(&store.connection, &state)?.is_some() {
        return Err(InstallError::RecoveryPending);
    }
    let now = clock();
    let candidate = admit(
        trust,
        store,
        InertPackageBytes { ..bytes },
        compatibility,
        now,
    )?;
    let existing = files::Layout::open(store, false)?;
    if let Some(layout) = &existing {
        layout.assert_owned(&state.records, None)?;
        layout.disk_bytes()?;
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid);
    }
    let plan = resolve(
        trust,
        store,
        &candidate,
        &state,
        existing.as_ref(),
        compatibility,
        now,
    )?;
    let record = Record {
        payload: candidate.archive().descriptor().payload().clone(),
        envelope: general_purpose::STANDARD.encode(bytes.envelope),
        lock: plan.lock().clone(),
        root_digest: plan.root_digest().into(),
        installed_at: now,
        artifact_id: uuid::Uuid::new_v4().to_string(),
    };
    let position = state
        .records
        .iter()
        .position(|r| r.pin() == record.pin() && r.payload.target == record.payload.target);
    if let Some(index) = position {
        existing
            .as_ref()
            .ok_or(InstallError::ArtifactInvalid)?
            .read(&state.records[index])?;
        if state.records[index].envelope == record.envelope
            && state.records[index].lock == record.lock
        {
            return Ok(InstalledInertPackage {
                record: state.records[index].clone(),
            });
        }
    } else if state.records.len() >= MAX_RECORDS {
        return Err(InstallError::BudgetExceeded);
    }
    drop(existing);
    let mut layout = files::Layout::open(store, true)?.ok_or(InstallError::ProfileRequired)?;
    let new_artifact = !state
        .records
        .iter()
        .any(|r| r.payload.archive.sha256 == record.payload.archive.sha256);
    if layout.has_artifact(&record)? == new_artifact {
        return Err(InstallError::ArtifactInvalid);
    }
    let extra = if new_artifact {
        record.payload.manifest.size + record.payload.archive.size + 512
    } else {
        0
    };
    let tool_usage = super::tools::install::retained_tool_usage(store)?;
    let package_usage = layout.disk_usage_reserving(usize::from(new_artifact))?;
    if package_usage
        .groups
        .checked_add(tool_usage.groups)
        .is_none_or(|n| n > files::MAX_GROUPS)
    {
        return Err(InstallError::BudgetExceeded);
    }
    if package_usage
        .bytes
        .checked_add(tool_usage.bytes)
        .and_then(|bytes| bytes.checked_add(extra))
        .is_none_or(|n| n > files::MAX_DISK_BYTES)
    {
        return Err(InstallError::BudgetExceeded);
    }
    let mut record = record;
    if !new_artifact {
        record.artifact_id = state
            .records
            .iter()
            .find(|r| r.payload.archive.sha256 == record.payload.archive.sha256)
            .ok_or(InstallError::StateInvalid)?
            .artifact_id
            .clone();
    }
    let mut pending = Journal {
        format_version: 2,
        id: if new_artifact {
            record.artifact_id.clone()
        } else {
            uuid::Uuid::new_v4().to_string()
        },
        before_digest: json_digest(&state)?,
        record,
        new_artifact,
        prepared: false,
    };
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| InstallError::Storage)?;
    write(&tx, PENDING, &pending)?;
    tx.commit().map_err(|_| InstallError::Storage)?;
    hook(InstallPhase::JournalCommitted, store)?;
    let writing_digest = json_digest(&pending)?;
    guard_pending(&store.connection, &pending, &writing_digest)?;
    if new_artifact {
        layout.stage_bytes(&pending, bytes.manifest, bytes.archive)?;
    }
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| InstallError::Storage)?;
    guard_pending(&tx, &pending, &writing_digest)?;
    pending.prepared = true;
    write(&tx, PENDING, &pending)?;
    tx.commit().map_err(|_| InstallError::Storage)?;
    let prepared_digest = json_digest(&pending)?;
    hook(InstallPhase::StagePrepared, store)?;
    guard_pending(&store.connection, &pending, &prepared_digest)?;
    if new_artifact {
        layout.publish(&pending)?;
    }
    hook(InstallPhase::ArtifactPublished, store)?;
    hook(InstallPhase::BeforeCommit, store)?;
    let commit_time = clock();
    let fresh = admit(
        trust,
        store,
        InertPackageBytes { ..bytes },
        compatibility,
        commit_time,
    )?;
    let plan = resolve(
        trust,
        store,
        &fresh,
        &state,
        Some(&layout),
        compatibility,
        commit_time,
    )?;
    if plan.lock() != &pending.record.lock {
        return Err(InstallError::InventoryChanged);
    }
    layout.read(&pending.record)?;
    layout.assert_owned(&state.records, Some(&pending))?;
    pending.record.root_digest = plan.root_digest().into();
    pending.record.installed_at = commit_time;
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| InstallError::Storage)?;
    guard_pending(&tx, &pending, &prepared_digest)?;
    if let Some(index) = position {
        state.records[index] = pending.record.clone();
    } else {
        state.records.push(pending.record.clone());
    }
    write(&tx, KEY, &state)?;
    tx.execute("DELETE FROM core_metadata WHERE key=?1", [PENDING])
        .map_err(|_| InstallError::Storage)?;
    tx.commit().map_err(|_| InstallError::Storage)?;
    hook(InstallPhase::Committed, store)?;
    Ok(InstalledInertPackage {
        record: pending.record,
    })
}
pub fn inspect_inert_inventory(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<InertInventorySnapshot, InstallError> {
    reconciliation::ensure_idle(&store.connection)?;
    let state = observed_inventory(trust, store)?;
    if journal(&store.connection, &state)?.is_some() {
        return Err(InstallError::RecoveryPending);
    }
    if let Some(layout) = files::Layout::open(store, false)? {
        layout.assert_owned(&state.records, None)?;
        layout.disk_bytes()?;
        for record in &state.records {
            layout.read(record)?;
        }
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid);
    }
    Ok(InertInventorySnapshot {
        pins: state.records.iter().map(Record::pin).collect(),
        locks: state.records.into_iter().map(|r| r.lock).collect(),
    })
}
/// Explicit T0 abort only: never roll forward, run package code, adopt an orphan,
/// lower trust floors, clear backup-recovery markers or delete retained blobs.
pub fn abort_inert_install(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<bool, InstallError> {
    reconciliation::ensure_idle(&store.connection)?;
    let state = observed_inventory(trust, store)?;
    let Some(pending) = journal(&store.connection, &state)? else {
        return Ok(false);
    };
    if let Some(mut layout) = files::Layout::open(store, false)? {
        layout.assert_owned(&state.records, Some(&pending))?;
        layout.disk_bytes()?;
        for record in &state.records {
            layout.read(record)?;
        }
        layout.quarantine(&pending)?;
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid);
    }
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| InstallError::Storage)?;
    guard_pending(&tx, &pending, &json_digest(&pending)?)?;
    tx.execute("DELETE FROM core_metadata WHERE key=?1", [PENDING])
        .map_err(|_| InstallError::Storage)?;
    tx.commit().map_err(|_| InstallError::Storage)?;
    Ok(true)
}
