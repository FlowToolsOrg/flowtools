//! Explicit quarantine of unreferenced inert blobs after metadata restore.
//! The opaque plan is not an adoption, rollback, grant or executable identity.
use super::*;

const PENDING_RECONCILIATION: &str = "package-reconciliation-pending-v1";
const LAST_RECEIPT: &str = "package-reconciliation-receipt-v1";

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Kind {
    Artifact,
    Stage,
}
impl Kind {
    pub(super) fn parent(self) -> &'static str {
        match self {
            Self::Artifact => "artifacts",
            Self::Stage => "staging",
        }
    }
    pub(super) fn validate_source(self, name: &str) -> Result<(), InstallError> {
        let valid = match self {
            Self::Artifact => valid_digest(name),
            Self::Stage => uuid(name),
        };
        if valid {
            Ok(())
        } else {
            Err(InstallError::PathRejected)
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Orphan {
    pub kind: Kind,
    pub source: String,
    pub destination_id: String,
    pub files: BTreeMap<String, BlobDescriptor>,
}
impl Orphan {
    pub(super) fn destination(&self) -> String {
        format!(
            "{}-{}",
            self.destination_id,
            match self.kind {
                Kind::Artifact => "artifact",
                Kind::Stage => "stage",
            }
        )
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pending {
    format_version: u16,
    id: String,
    profile_digest: String,
    scope_digest: String,
    before_digest: String,
    orphans: Vec<Orphan>,
}
/// Host-created, exact byte-observation plan; never deserialize a client plan.
pub struct InertReconciliationPlan {
    pending: Pending,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt<'a> {
    format_version: u16,
    outcome: &'static str,
    plan: &'a Pending,
}
#[derive(Debug, PartialEq, Eq)]
pub struct InertReconciliationSummary {
    pub artifacts: usize,
    pub stages: usize,
    pub bytes: u64,
}
impl InertReconciliationPlan {
    pub fn summary(&self) -> InertReconciliationSummary {
        summary(&self.pending)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in super::super) enum ReconciliationPhase {
    JournalCommitted,
    GroupQuarantined(usize),
    BeforeCommit,
    Committed,
}
fn summary(pending: &Pending) -> InertReconciliationSummary {
    InertReconciliationSummary {
        artifacts: pending
            .orphans
            .iter()
            .filter(|o| o.kind == Kind::Artifact)
            .count(),
        stages: pending
            .orphans
            .iter()
            .filter(|o| o.kind == Kind::Stage)
            .count(),
        bytes: pending
            .orphans
            .iter()
            .flat_map(|o| o.files.values())
            .map(|f| f.size)
            .sum(),
    }
}
fn uuid(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|v| v.to_string() == id)
}
fn profile(connection: &rusqlite::Connection) -> Result<String, InstallError> {
    let path = connection
        .path()
        .filter(|p| !p.is_empty() && *p != ":memory:")
        .ok_or(InstallError::ProfileRequired)?;
    let canonical = std::fs::canonicalize(path).map_err(|_| InstallError::Storage)?;
    Ok(hash(canonical.as_os_str().as_encoded_bytes()))
}
fn scope(connection: &rusqlite::Connection) -> Result<String, InstallError> {
    let mut values = Vec::new();
    for key in [
        "package-trust-recovery-required",
        "recovery-generation",
        "package-trust-recovery-receipt-v1",
    ] {
        let length: Option<i64> = connection
            .query_row(
                "SELECT length(CAST(value AS BLOB)) FROM core_metadata WHERE key=?1",
                [key],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| InstallError::Storage)?;
        if length.is_some_and(|n| n < 0 || n > MAX_STATE_BYTES as i64) {
            return Err(InstallError::StateInvalid);
        }
        let value: Option<String> = connection
            .query_row("SELECT value FROM core_metadata WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|_| InstallError::Storage)?;
        values.push(value.as_deref().map(|v| hash(v.as_bytes())));
    }
    json_digest(&values)
}
fn validate(pending: &Pending, state: &Inventory) -> Result<(), InstallError> {
    if pending.format_version != 1
        || !uuid(&pending.id)
        || [
            &pending.before_digest,
            &pending.profile_digest,
            &pending.scope_digest,
        ]
        .iter()
        .any(|d| !valid_digest(d))
        || pending.orphans.is_empty()
        || pending.orphans.len() > files::MAX_GROUPS
    {
        return Err(InstallError::StateInvalid);
    }
    let mut sources = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    let mut bytes = 0u64;
    for orphan in &pending.orphans {
        orphan.kind.validate_source(&orphan.source)?;
        if !uuid(&orphan.destination_id)
            || !sources.insert((orphan.kind, &orphan.source))
            || !destinations.insert(orphan.destination())
            || orphan.files.len() > 3
            || (orphan.kind == Kind::Artifact
                && state
                    .records
                    .iter()
                    .any(|r| r.payload.archive.sha256 == orphan.source))
        {
            return Err(InstallError::StateInvalid);
        }
        for (name, file) in &orphan.files {
            let limit = match name.as_str() {
                "manifest.json" => MAX_PAYLOAD_BYTES as u64,
                "package.zip" => MAX_ARCHIVE_BYTES,
                "seal.json" => 512,
                _ => return Err(InstallError::PathRejected),
            };
            if !valid_digest(&file.sha256) || file.size > limit {
                return Err(InstallError::StateInvalid);
            }
            bytes = bytes
                .checked_add(file.size)
                .ok_or(InstallError::BudgetExceeded)?;
        }
    }
    if bytes > files::MAX_DISK_BYTES {
        return Err(InstallError::BudgetExceeded);
    }
    Ok(())
}
pub(super) fn ensure_idle(connection: &rusqlite::Connection) -> Result<(), InstallError> {
    if read::<Pending>(connection, PENDING_RECONCILIATION)?.is_some() {
        return Err(InstallError::RecoveryPending);
    }
    Ok(())
}
fn matches(
    connection: &rusqlite::Connection,
    pending: &Pending,
    state: &Inventory,
) -> Result<(), InstallError> {
    if pending.before_digest != json_digest(state)?
        || pending.profile_digest != profile(connection)?
        || pending.scope_digest != scope(connection)?
    {
        return Err(InstallError::InventoryChanged);
    }
    Ok(())
}
fn physical(
    layout: &files::Layout,
    state: &Inventory,
    pending: &Pending,
) -> Result<(), InstallError> {
    layout.require_quarantine()?;
    layout.disk_bytes()?;
    layout.assert_owned_with_orphans(&state.records, None, &pending.orphans)?;
    for record in &state.records {
        layout.read(record)?;
    }
    // Preflight the entire batch before moving the first group.
    for orphan in &pending.orphans {
        layout.check_orphan(orphan)?;
    }
    Ok(())
}
/// Read-only T0 plan. The Host must reserve/protect the existing private profile.
pub fn plan_inert_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<InertReconciliationPlan, InstallError> {
    ensure_idle(&store.connection)?;
    let state = observed_inventory(trust, store)?;
    if journal(&store.connection, &state)?.is_some() {
        return Err(InstallError::RecoveryPending);
    }
    let mut orphans = Vec::new();
    if let Some(layout) = files::Layout::open(store, false)? {
        layout.disk_bytes()?;
        for record in &state.records {
            layout.read(record)?;
        }
        orphans = layout.discover_orphans(&state.records)?;
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid);
    }
    let pending = Pending {
        format_version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        profile_digest: profile(&store.connection)?,
        scope_digest: scope(&store.connection)?,
        before_digest: json_digest(&state)?,
        orphans,
    };
    if !pending.orphans.is_empty() {
        validate(&pending, &state)?;
    }
    Ok(InertReconciliationPlan { pending })
}
/// Apply a reviewed T0 plan. No plugin capability or client approval flag enters.
pub fn apply_inert_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: InertReconciliationPlan,
) -> Result<InertReconciliationSummary, InstallError> {
    apply_with_hook(trust, store, plan, |_, _| Ok(()))
}
pub(in super::super) fn apply_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: InertReconciliationPlan,
    mut hook: impl FnMut(ReconciliationPhase, &mut DataStore) -> Result<(), InstallError>,
) -> Result<InertReconciliationSummary, InstallError> {
    ensure_idle(&store.connection)?;
    let state = observed_inventory(trust, store)?;
    if journal(&store.connection, &state)?.is_some() {
        return Err(InstallError::RecoveryPending);
    }
    let pending = plan.pending;
    matches(&store.connection, &pending, &state)?;
    if pending.orphans.is_empty() {
        if let Some(layout) = files::Layout::open(store, false)? {
            layout.assert_owned(&state.records, None)?;
            layout.disk_bytes()?;
            for record in &state.records {
                layout.read(record)?;
            }
        } else if !state.records.is_empty() {
            return Err(InstallError::ArtifactInvalid);
        }
        return Ok(summary(&pending));
    }
    validate(&pending, &state)?;
    let layout = files::Layout::open(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    physical(&layout, &state, &pending)?;
    // The reviewed plan is not allowed to adopt an already quarantined target.
    for orphan in &pending.orphans {
        if !layout.check_orphan(orphan)? {
            return Err(InstallError::StateInvalid);
        }
    }
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| InstallError::Storage)?;
    matches(&tx, &pending, &inventory(&tx)?)?;
    ensure_idle(&tx)?;
    write(&tx, PENDING_RECONCILIATION, &pending)?;
    tx.commit().map_err(|_| InstallError::Storage)?;
    hook(ReconciliationPhase::JournalCommitted, store)?;
    resume_with_layout(trust, store, &layout, pending, &mut hook)
}
/// Explicitly continue an exact persisted batch; never auto-adopt orphan bytes.
pub fn resume_inert_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<Option<InertReconciliationSummary>, InstallError> {
    resume_with_hook(trust, store, |_, _| Ok(()))
}
pub(super) fn resume_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    mut hook: impl FnMut(ReconciliationPhase, &mut DataStore) -> Result<(), InstallError>,
) -> Result<Option<InertReconciliationSummary>, InstallError> {
    let state = observed_inventory(trust, store)?;
    if journal(&store.connection, &state)?.is_some() {
        return Err(InstallError::RecoveryPending);
    }
    let Some(pending) = read::<Pending>(&store.connection, PENDING_RECONCILIATION)? else {
        return Ok(None);
    };
    validate(&pending, &state)?;
    matches(&store.connection, &pending, &state)?;
    let layout = files::Layout::open(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    Ok(Some(resume_with_layout(
        trust, store, &layout, pending, &mut hook,
    )?))
}
fn resume_with_layout(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    layout: &files::Layout,
    pending: Pending,
    hook: &mut impl FnMut(ReconciliationPhase, &mut DataStore) -> Result<(), InstallError>,
) -> Result<InertReconciliationSummary, InstallError> {
    let state = observed_inventory(trust, store)?;
    validate(&pending, &state)?;
    matches(&store.connection, &pending, &state)?;
    let expected = json_digest(&pending)?;
    guard(trust, store, &pending, &expected)?;
    physical(layout, &state, &pending)?;
    for (index, orphan) in pending.orphans.iter().enumerate() {
        guard(trust, store, &pending, &expected)?;
        layout.quarantine_orphan(orphan)?;
        hook(ReconciliationPhase::GroupQuarantined(index), store)?;
    }
    hook(ReconciliationPhase::BeforeCommit, store)?;
    guard(trust, store, &pending, &expected)?;
    physical(layout, &state, &pending)?;
    layout.assert_owned(&state.records, None)?;
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| InstallError::Storage)?;
    let current = inventory(&tx)?;
    matches(&tx, &pending, &current)?;
    let actual: Pending =
        read(&tx, PENDING_RECONCILIATION)?.ok_or(InstallError::InventoryChanged)?;
    if json_digest(&actual)? != expected {
        return Err(InstallError::InventoryChanged);
    }
    write(
        &tx,
        LAST_RECEIPT,
        &Receipt {
            format_version: 1,
            outcome: "completed",
            plan: &pending,
        },
    )?;
    tx.execute(
        "DELETE FROM core_metadata WHERE key=?1",
        [PENDING_RECONCILIATION],
    )
    .map_err(|_| InstallError::Storage)?;
    tx.commit().map_err(|_| InstallError::Storage)?;
    hook(ReconciliationPhase::Committed, store)?;
    Ok(summary(&pending))
}
/// Explicit T0 cancellation: keep every byte in its current inert location.
/// A new restored scope must first complete trust recovery. Never adopt blobs.
pub fn abort_inert_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<bool, InstallError> {
    let state = observed_inventory(trust, store)?;
    if journal(&store.connection, &state)?.is_some() {
        return Err(InstallError::RecoveryPending);
    }
    let Some(pending) = read::<Pending>(&store.connection, PENDING_RECONCILIATION)? else {
        return Ok(false);
    };
    validate(&pending, &state)?;
    if pending.profile_digest != profile(&store.connection)?
        || pending.before_digest != json_digest(&state)?
    {
        return Err(InstallError::InventoryChanged);
    }
    let expected = json_digest(&pending)?;
    let layout = files::Layout::open(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    physical(&layout, &state, &pending)?;
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| InstallError::Storage)?;
    if json_digest(&inventory(&tx)?)? != pending.before_digest
        || profile(&tx)? != pending.profile_digest
    {
        return Err(InstallError::InventoryChanged);
    }
    let actual: Pending =
        read(&tx, PENDING_RECONCILIATION)?.ok_or(InstallError::InventoryChanged)?;
    if json_digest(&actual)? != expected {
        return Err(InstallError::InventoryChanged);
    }
    write(
        &tx,
        LAST_RECEIPT,
        &Receipt {
            format_version: 1,
            outcome: "aborted",
            plan: &pending,
        },
    )?;
    tx.execute(
        "DELETE FROM core_metadata WHERE key=?1",
        [PENDING_RECONCILIATION],
    )
    .map_err(|_| InstallError::Storage)?;
    tx.commit().map_err(|_| InstallError::Storage)?;
    Ok(true)
}
fn guard(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    pending: &Pending,
    expected: &str,
) -> Result<(), InstallError> {
    let state = observed_inventory(trust, store)?;
    if journal(&store.connection, &state)?.is_some() {
        return Err(InstallError::RecoveryPending);
    }
    matches(&store.connection, pending, &state)?;
    let actual: Pending =
        read(&store.connection, PENDING_RECONCILIATION)?.ok_or(InstallError::InventoryChanged)?;
    if json_digest(&actual)? != expected {
        return Err(InstallError::InventoryChanged);
    }
    Ok(())
}
