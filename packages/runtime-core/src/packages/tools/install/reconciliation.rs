//! Explicit quarantine of unreferenced inert tool ZIP groups. This is neither
//! admission nor an image recovery, accepted-job lease, grant or execution API.
use super::*;
use crate::packages::install::reconciliation::{Kind, Orphan};
use std::collections::BTreeMap;

const PENDING: &str = "tool-archive-reconciliation-pending-v1";
const RECEIPT: &str = "tool-archive-reconciliation-receipt-v1";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pending {
    format_version: u16,
    id: String,
    profile_digest: String,
    before_digest: String,
    companions_digest: String,
    scope_digest: String,
    orphans: Vec<Orphan>,
}
/// Only a read-only T0 observation constructs this plan; client metadata cannot.
/// ```compile_fail
/// use flowtools_runtime_core::packages::tools::install::reconciliation::ToolArchiveReconciliationPlan;
/// let forged = ToolArchiveReconciliationPlan { pending: todo!() };
/// ```
pub struct ToolArchiveReconciliationPlan {
    pending: Pending,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ToolArchiveReconciliationSummary {
    pub artifacts: usize,
    pub stages: usize,
    pub bytes: u64,
}
impl ToolArchiveReconciliationPlan {
    pub fn summary(&self) -> ToolArchiveReconciliationSummary {
        summary(&self.pending)
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt<'a> {
    format_version: u16,
    outcome: &'static str,
    plan: &'a Pending,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReconciliationPhase {
    JournalCommitted,
    GroupQuarantined(usize),
    BeforeCommit,
    Committed,
}
fn summary(pending: &Pending) -> ToolArchiveReconciliationSummary {
    ToolArchiveReconciliationSummary {
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
// Presence is sufficient only for refusal. Never parse an unrelated journal to
// decide whether cleanup is permitted.
pub(crate) fn ensure_idle(
    connection: &rusqlite::Connection,
) -> std::result::Result<(), InstallError> {
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
fn companions_idle(connection: &rusqlite::Connection) -> Result<()> {
    images::require_no_pending(connection)?;
    let present: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key IN ('package-install-pending-v1','package-reconciliation-pending-v1','tool-archive-install-pending-v1'))", [], |r| r.get(0)
    ).map_err(storage)?;
    if present {
        Err(InstallError::RecoveryPending.into())
    } else {
        Ok(())
    }
}
fn companions(connection: &rusqlite::Connection) -> Result<String> {
    let mut values = BTreeMap::new();
    for key in ["package-install-inventory-v1", "tool-image-inventory-v1"] {
        values.insert(key, read_on::<serde_json::Value>(connection, key)?);
    }
    // Preserve the original a5 stamp exactly when the new domain is absent.
    if let Some(value) = read_on::<serde_json::Value>(connection, "tool-consumer-inventory-v1")? {
        values.insert("tool-consumer-inventory-v1", Some(value));
    }
    value_digest(&values)
}
fn source_state(trust: &PackageTrustStore, store: &mut DataStore) -> Result<Inventory> {
    native()?;
    companions_idle(&store.connection)?;
    let state = inventory(trust, store)?;
    if journal(store, &state)?.is_some() {
        return Err(InstallError::RecoveryPending.into());
    }
    Ok(state)
}
fn validate(pending: &Pending, state: &Inventory) -> Result<()> {
    if pending.format_version != 1
        || !uuid(&pending.id)
        || [
            &pending.profile_digest,
            &pending.before_digest,
            &pending.companions_digest,
            &pending.scope_digest,
        ]
        .iter()
        .any(|d| !packages::valid_digest(d))
        || pending.orphans.len() > files::MAX_GROUPS
    {
        return Err(invalid());
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
            return Err(invalid());
        }
        for (name, file) in &orphan.files {
            let limit = match name.as_str() {
                "manifest.json" => packages::MAX_PAYLOAD_BYTES as u64,
                "package.zip" => packages::MAX_ARCHIVE_BYTES,
                "seal.json" => 512,
                _ => return Err(InstallError::PathRejected.into()),
            };
            if !packages::valid_digest(&file.sha256) || file.size > limit {
                return Err(invalid());
            }
            bytes = bytes
                .checked_add(file.size)
                .ok_or(InstallError::BudgetExceeded)?;
        }
    }
    if bytes > files::MAX_DISK_BYTES {
        return Err(InstallError::BudgetExceeded.into());
    }
    Ok(())
}
fn usage(store: &DataStore) -> Result<()> {
    let usage = retained_tool_usage(store)?.combine(
        files::Layout::open(store, false)?
            .map(|l| l.disk_usage_reserving(0))
            .transpose()?
            .unwrap_or_default(),
    )?;
    if usage.bytes > files::MAX_DISK_BYTES || usage.groups > files::MAX_GROUPS {
        return Err(InstallError::BudgetExceeded.into());
    }
    Ok(())
}
fn physical(store: &DataStore, state: &Inventory, pending: &Pending) -> Result<()> {
    images::reconciliation_preflight(store, state)?;
    usage(store)?;
    if let Some(layout) = files::Layout::open_for_tools(store, false)? {
        layout.assert_owned_with_orphans(&state.records, None, &pending.orphans)?;
        for record in &state.records {
            layout.read(record)?;
        }
        if !pending.orphans.is_empty() {
            layout.require_quarantine()?;
        }
        // No effect until the complete batch has passed this preflight.
        for orphan in &pending.orphans {
            layout.check_orphan(orphan)?;
        }
    } else if !state.records.is_empty() || !pending.orphans.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    Ok(())
}
fn matches(
    connection: &rusqlite::Connection,
    state: &Inventory,
    pending: &Pending,
    scope: &str,
) -> Result<()> {
    companions_idle(connection)?;
    if pending.profile_digest != state.profile_digest
        || pending.before_digest != value_digest(state)?
        || pending.companions_digest != companions(connection)?
        || scope != images::trust_stamp(connection)?
    {
        return Err(InstallError::InventoryChanged.into());
    }
    Ok(())
}
// Repeat inside the transaction, and after SQL writes, so triggers cannot commit
// changed inventories/trust/journals alongside a success receipt.
fn guard_on(
    connection: &rusqlite::Connection,
    state: &Inventory,
    pending: &Pending,
    expected: Option<&str>,
    scope: &str,
) -> Result<()> {
    let actual: Inventory = read_on(connection, KEY)?.unwrap_or(Inventory {
        format_version: 1,
        profile_digest: state.profile_digest.clone(),
        records: vec![],
    });
    matches(connection, &actual, pending, scope)?;
    let current = read_on::<Pending>(connection, PENDING)?
        .as_ref()
        .map(value_digest)
        .transpose()?;
    if current.as_deref() != expected {
        return Err(InstallError::InventoryChanged.into());
    }
    Ok(())
}
pub fn plan_tool_archive_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<ToolArchiveReconciliationPlan> {
    ensure_idle(&store.connection)?;
    let state = source_state(trust, store)?;
    images::reconciliation_preflight(store, &state)?;
    usage(store)?;
    let mut orphans = vec![];
    if let Some(layout) = files::Layout::open_for_tools(store, false)? {
        for record in &state.records {
            layout.read(record)?;
        }
        orphans = layout.discover_orphans(&state.records)?;
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    let pending = Pending {
        format_version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        profile_digest: state.profile_digest.clone(),
        before_digest: value_digest(&state)?,
        companions_digest: companions(&store.connection)?,
        scope_digest: images::trust_stamp(&store.connection)?,
        orphans,
    };
    validate(&pending, &state)?;
    Ok(ToolArchiveReconciliationPlan { pending })
}
pub fn apply_tool_archive_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: ToolArchiveReconciliationPlan,
) -> Result<ToolArchiveReconciliationSummary> {
    apply_with_hook(trust, store, plan, |_, _| Ok(()))
}
fn apply_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: ToolArchiveReconciliationPlan,
    mut hook: impl FnMut(ReconciliationPhase, &mut DataStore) -> Result<()>,
) -> Result<ToolArchiveReconciliationSummary> {
    ensure_idle(&store.connection)?;
    let state = source_state(trust, store)?;
    let pending = plan.pending;
    validate(&pending, &state)?;
    matches(&store.connection, &state, &pending, &pending.scope_digest)?;
    physical(store, &state, &pending)?;
    if pending.orphans.is_empty() {
        return Ok(summary(&pending));
    }
    let layout =
        files::Layout::open_for_tools(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    for orphan in &pending.orphans {
        if !layout.check_orphan(orphan)? {
            return Err(invalid());
        }
    }
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, &state, &pending, None, &pending.scope_digest)?;
    write(&tx, PENDING, &pending)?;
    let expected = value_digest(&pending)?;
    guard_on(
        &tx,
        &state,
        &pending,
        Some(&expected),
        &pending.scope_digest,
    )?;
    tx.commit().map_err(storage)?;
    hook(ReconciliationPhase::JournalCommitted, store)?;
    finish(trust, store, pending, &mut hook)
}
pub fn resume_tool_archive_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<Option<ToolArchiveReconciliationSummary>> {
    let state = source_state(trust, store)?;
    let Some(pending) = read::<Pending>(store, PENDING)? else {
        return Ok(None);
    };
    validate(&pending, &state)?;
    if pending.orphans.is_empty() {
        return Err(invalid());
    }
    Ok(Some(finish(trust, store, pending, &mut |_, _| Ok(()))?))
}
fn finish(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    pending: Pending,
    hook: &mut impl FnMut(ReconciliationPhase, &mut DataStore) -> Result<()>,
) -> Result<ToolArchiveReconciliationSummary> {
    let state = source_state(trust, store)?;
    let expected = value_digest(&pending)?;
    guard_on(
        &store.connection,
        &state,
        &pending,
        Some(&expected),
        &pending.scope_digest,
    )?;
    physical(store, &state, &pending)?;
    let layout =
        files::Layout::open_for_tools(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    for (index, orphan) in pending.orphans.iter().enumerate() {
        let current = source_state(trust, store)?;
        guard_on(
            &store.connection,
            &current,
            &pending,
            Some(&expected),
            &pending.scope_digest,
        )?;
        layout.quarantine_orphan(orphan)?;
        hook(ReconciliationPhase::GroupQuarantined(index), store)?;
    }
    hook(ReconciliationPhase::BeforeCommit, store)?;
    let current = source_state(trust, store)?;
    guard_on(
        &store.connection,
        &current,
        &pending,
        Some(&expected),
        &pending.scope_digest,
    )?;
    physical(store, &current, &pending)?;
    layout.assert_owned(&current.records, None)?;
    commit_receipt(
        store,
        &current,
        &pending,
        &expected,
        &pending.scope_digest,
        "completed",
    )?;
    hook(ReconciliationPhase::Committed, store)?;
    Ok(summary(&pending))
}
fn commit_receipt(
    store: &mut DataStore,
    state: &Inventory,
    pending: &Pending,
    expected: &str,
    scope: &str,
    outcome: &'static str,
) -> Result<()> {
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, state, pending, Some(expected), scope)?;
    let receipt = Receipt {
        format_version: 1,
        outcome,
        plan: pending,
    };
    write(&tx, RECEIPT, &receipt)?;
    guard_on(&tx, state, pending, Some(expected), scope)?;
    packages::install::metadata::remove_pending(&tx, PENDING)?;
    guard_on(&tx, state, pending, None, scope)?;
    if value_digest(&read_on::<serde_json::Value>(&tx, RECEIPT)?.ok_or_else(invalid)?)?
        != value_digest(&receipt)?
    {
        return Err(InstallError::InventoryChanged.into());
    }
    tx.commit().map_err(storage)?;
    Ok(())
}
/// Explicit cancellation preserves both remaining sources and quarantined bytes.
/// A changed trust/restore scope can only abort after current trust recovery;
/// profile, inventories and the complete batch must still match exactly.
pub fn abort_tool_archive_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<bool> {
    let state = source_state(trust, store)?;
    let Some(pending) = read::<Pending>(store, PENDING)? else {
        return Ok(false);
    };
    validate(&pending, &state)?;
    if pending.orphans.is_empty() {
        return Err(invalid());
    }
    let scope = images::trust_stamp(&store.connection)?;
    let expected = value_digest(&pending)?;
    guard_on(&store.connection, &state, &pending, Some(&expected), &scope)?;
    physical(store, &state, &pending)?;
    commit_receipt(store, &state, &pending, &expected, &scope, "aborted")?;
    Ok(true)
}
#[cfg(test)]
pub(crate) fn apply_fixture_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: ToolArchiveReconciliationPlan,
    hook: impl FnMut(ReconciliationPhase, &mut DataStore) -> Result<()>,
) -> Result<ToolArchiveReconciliationSummary> {
    apply_with_hook(trust, store, plan, hook)
}
#[cfg(test)]
mod consumer_compatibility_tests {
    use super::*;
    #[test]
    fn absent_consumer_inventory_preserves_the_original_a5_companion_stamp() {
        let store = DataStore::memory().unwrap();
        assert_eq!(
            companions(&store.connection).unwrap(),
            "3006dbfac8ad119fab8a69a2dfc953db89e1e8ce82ea02f3f2b4cfd47ee34062"
        );
        store
            .connection
            .execute(
                "INSERT INTO core_metadata(key,value) VALUES('tool-consumer-inventory-v1','{}')",
                [],
            )
            .unwrap();
        assert_ne!(
            companions(&store.connection).unwrap(),
            "3006dbfac8ad119fab8a69a2dfc953db89e1e8ce82ea02f3f2b4cfd47ee34062"
        );
    }
}
