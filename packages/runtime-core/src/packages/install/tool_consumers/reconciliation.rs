//! Explicit quarantine for this fixed inert consumer domain. Never adopts a
//! signed package, rewrites an installed lock, deletes bytes or grants execution.
use super::*;
use crate::packages::install::reconciliation::{Kind, Orphan};
const PENDING: &str = "tool-consumer-reconciliation-pending-v1";
const RECEIPT: &str = "tool-consumer-reconciliation-receipt-v1";

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
/// Only read-only T0 observation constructs this plan.
/// ```compile_fail
/// use flowtools_runtime_core::packages::install::tool_consumers::reconciliation::ConsumerReconciliationPlan;
/// let forged = ConsumerReconciliationPlan { pending: todo!() };
/// ```
pub struct ConsumerReconciliationPlan {
    pending: Pending,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ConsumerReconciliationSummary {
    pub artifacts: usize,
    pub stages: usize,
    pub bytes: u64,
}
impl ConsumerReconciliationPlan {
    pub fn summary(&self) -> ConsumerReconciliationSummary {
        summary(&self.pending)
    }
}
fn summary(p: &Pending) -> ConsumerReconciliationSummary {
    ConsumerReconciliationSummary {
        artifacts: p
            .orphans
            .iter()
            .filter(|o| o.kind == Kind::Artifact)
            .count(),
        stages: p.orphans.iter().filter(|o| o.kind == Kind::Stage).count(),
        bytes: p
            .orphans
            .iter()
            .flat_map(|o| o.files.values())
            .map(|f| f.size)
            .sum(),
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt<'a> {
    format_version: u16,
    outcome: &'static str,
    plan: &'a Pending,
    committed_scope_digest: &'a str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConsumerReconciliationPhase {
    JournalCommitted,
    GroupQuarantined(usize),
    BeforeCommit,
    Committed,
}
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
    super::companions_idle(connection)?;
    let present: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key=?1)",
            [super::PENDING],
            |r| r.get(0),
        )
        .map_err(storage)?;
    if present {
        Err(InstallError::RecoveryPending.into())
    } else {
        Ok(())
    }
}
fn trust_scope(connection: &rusqlite::Connection) -> Result<String> {
    let rows = ForeignGuard::capture(connection)?
        .rows
        .into_iter()
        .filter(|(key, _)| key.starts_with("package-trust-") || key == "recovery-generation")
        .map(|(key, value)| (key, hash(value.as_bytes())))
        .collect::<Vec<_>>();
    digest(&rows)
}
fn load_state(trust: &PackageTrustStore, store: &mut DataStore) -> Result<Inventory> {
    companions_idle(&store.connection)?;
    consumer_sources::validate_reconciliation_sources(trust, store)?;
    inventory(store)
}
fn validate(p: &Pending, state: &Inventory) -> Result<()> {
    if p.format_version != 1
        || !uuid(&p.id)
        || p.orphans.len() > files::MAX_GROUPS
        || [
            &p.profile_digest,
            &p.before_digest,
            &p.companions_digest,
            &p.scope_digest,
        ]
        .iter()
        .any(|s| !valid_digest(s))
    {
        return Err(invalid());
    }
    let (mut sources, mut destinations) = (BTreeSet::new(), BTreeSet::new());
    let mut bytes = 0u64;
    for orphan in &p.orphans {
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
        for (name, blob) in &orphan.files {
            let limit = match name.as_str() {
                "manifest.json" => MAX_PAYLOAD_BYTES as u64,
                "package.zip" => MAX_ARCHIVE_BYTES,
                "seal.json" => 512,
                _ => return Err(InstallError::PathRejected.into()),
            };
            if !valid_digest(&blob.sha256) || blob.size > limit {
                return Err(invalid());
            }
            bytes = bytes
                .checked_add(blob.size)
                .ok_or(InstallError::BudgetExceeded)?;
        }
    }
    if bytes > files::MAX_DISK_BYTES {
        return Err(InstallError::BudgetExceeded.into());
    }
    Ok(())
}
fn physical(store: &DataStore, state: &Inventory, pending: &Pending) -> Result<()> {
    usage(store, 0, 0)?;
    if let Some(layout) = files::Layout::open_for_tool_consumers(store, false)? {
        layout.assert_owned_with_orphans(&state.records, None, &pending.orphans)?;
        for record in &state.records {
            layout.read(record)?;
        }
        if !pending.orphans.is_empty() {
            layout.require_quarantine()?;
        }
        for orphan in &pending.orphans {
            layout.check_orphan(orphan)?;
        }
    } else if !state.records.is_empty() || !pending.orphans.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    Ok(())
}
fn guard_on(
    connection: &rusqlite::Connection,
    state: &Inventory,
    pending: &Pending,
    expected: Option<&str>,
    scope: &str,
) -> Result<()> {
    companions_idle(connection)?;
    let current = inventory_on(connection, &state.profile_digest)?;
    if digest(&current)? != pending.before_digest
        || current.profile_digest != pending.profile_digest
        || companions(connection)? != pending.companions_digest
        || trust_scope(connection)? != scope
        || read::<Pending>(connection, PENDING)?
            .as_ref()
            .map(digest)
            .transpose()?
            .as_deref()
            != expected
    {
        return Err(InstallError::InventoryChanged.into());
    }
    Ok(())
}
// Freeze exact relevant rows once, then check them under the held native source
// immediately before each move. No repeated large JSON tree parsing per effect.
const OBSERVED_ROWS: &str = "key GLOB 'package-trust-*' OR key='recovery-generation' OR key IN ('package-install-inventory-v1','tool-archive-inventory-v1','tool-image-inventory-v1','package-install-pending-v1','package-reconciliation-pending-v1','tool-archive-install-pending-v1','tool-archive-reconciliation-pending-v1','tool-image-materialization-pending-v1','tool-image-reconciliation-pending-v1','tool-consumer-inventory-v1','tool-consumer-install-pending-v1','tool-consumer-reconciliation-pending-v1')";
struct MetadataGuard {
    rows: Vec<(String, String)>,
}
impl MetadataGuard {
    fn capture(connection: &rusqlite::Connection) -> Result<Self> {
        let mut statement = connection.prepare(&format!("SELECT key,CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=16777216 THEN value ELSE NULL END FROM core_metadata WHERE {OBSERVED_ROWS} ORDER BY key")).map_err(storage)?;
        let mut cursor = statement.query([]).map_err(storage)?;
        let (mut rows, mut bytes) = (Vec::new(), 0usize);
        while let Some(row) = cursor.next().map_err(storage)? {
            let key: String = row.get(0).map_err(storage)?;
            let value: String = row.get(1).map_err(storage)?;
            bytes = bytes
                .checked_add(key.len())
                .and_then(|n| n.checked_add(value.len()))
                .ok_or(InstallError::BudgetExceeded)?;
            if rows.len() >= 32 || bytes > 67_108_864 {
                return Err(InstallError::BudgetExceeded.into());
            }
            rows.push((key, value));
        }
        Ok(Self { rows })
    }
    fn check(&self, connection: &rusqlite::Connection) -> std::result::Result<(), InstallError> {
        let count: i64 = connection
            .query_row(
                &format!("SELECT count(*) FROM core_metadata WHERE {OBSERVED_ROWS}"),
                [],
                |r| r.get(0),
            )
            .map_err(|_| InstallError::Storage)?;
        if count != self.rows.len() as i64 {
            return Err(InstallError::InventoryChanged);
        }
        let mut query = connection.prepare_cached("SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key=?1 AND typeof(value)='text' AND CAST(value AS BLOB)=CAST(?2 AS BLOB))").map_err(|_| InstallError::Storage)?;
        for (key, value) in &self.rows {
            let same: bool = query
                .query_row(params![key, value], |r| r.get(0))
                .map_err(|_| InstallError::Storage)?;
            if !same {
                return Err(InstallError::InventoryChanged);
            }
        }
        Ok(())
    }
}
pub fn plan_consumer_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<ConsumerReconciliationPlan> {
    ensure_idle(&store.connection)?;
    let state = load_state(trust, store)?;
    usage(store, 0, 0)?;
    let mut orphans = vec![];
    if let Some(layout) = files::Layout::open_for_tool_consumers(store, false)? {
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
        before_digest: digest(&state)?,
        companions_digest: companions(&store.connection)?,
        scope_digest: trust_scope(&store.connection)?,
        orphans,
    };
    validate(&pending, &state)?;
    Ok(ConsumerReconciliationPlan { pending })
}
pub fn apply_consumer_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: ConsumerReconciliationPlan,
) -> Result<ConsumerReconciliationSummary> {
    apply_with_hook(trust, store, plan, |_, _| Ok(()))
}
fn apply_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: ConsumerReconciliationPlan,
    mut hook: impl FnMut(ConsumerReconciliationPhase, &mut DataStore) -> Result<()>,
) -> Result<ConsumerReconciliationSummary> {
    ensure_idle(&store.connection)?;
    let state = load_state(trust, store)?;
    let pending = plan.pending;
    validate(&pending, &state)?;
    guard_on(
        &store.connection,
        &state,
        &pending,
        None,
        &pending.scope_digest,
    )?;
    physical(store, &state, &pending)?;
    if pending.orphans.is_empty() {
        return Ok(summary(&pending));
    }
    let layout = files::Layout::open_for_tool_consumers(store, false)?
        .ok_or(InstallError::ArtifactInvalid)?;
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
    let expected = digest(&pending)?;
    guard_on(
        &tx,
        &state,
        &pending,
        Some(&expected),
        &pending.scope_digest,
    )?;
    tx.commit().map_err(storage)?;
    hook(ConsumerReconciliationPhase::JournalCommitted, store)?;
    finish(trust, store, pending, &mut hook)
}
pub fn resume_consumer_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<Option<ConsumerReconciliationSummary>> {
    let state = load_state(trust, store)?;
    let Some(pending) = read::<Pending>(&store.connection, PENDING)? else {
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
    hook: &mut impl FnMut(ConsumerReconciliationPhase, &mut DataStore) -> Result<()>,
) -> Result<ConsumerReconciliationSummary> {
    let state = load_state(trust, store)?;
    let expected = digest(&pending)?;
    guard_on(
        &store.connection,
        &state,
        &pending,
        Some(&expected),
        &pending.scope_digest,
    )?;
    physical(store, &state, &pending)?;
    let metadata = MetadataGuard::capture(&store.connection)?;
    let layout = files::Layout::open_for_tool_consumers(store, false)?
        .ok_or(InstallError::ArtifactInvalid)?;
    for (index, orphan) in pending.orphans.iter().enumerate() {
        metadata.check(&store.connection)?;
        layout.quarantine_orphan_guarded(orphan, || metadata.check(&store.connection))?;
        hook(ConsumerReconciliationPhase::GroupQuarantined(index), store)?;
    }
    hook(ConsumerReconciliationPhase::BeforeCommit, store)?;
    metadata.check(&store.connection)?;
    let current = load_state(trust, store)?;
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
    hook(ConsumerReconciliationPhase::Committed, store)?;
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
    let receipt = Receipt {
        format_version: 1,
        outcome,
        plan: pending,
        committed_scope_digest: scope,
    };
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, state, pending, Some(expected), scope)?;
    write(&tx, RECEIPT, &receipt)?;
    guard_on(&tx, state, pending, Some(expected), scope)?;
    crate::packages::install::metadata::remove_pending(&tx, PENDING)?;
    guard_on(&tx, state, pending, None, scope)?;
    if read::<serde_json::Value>(&tx, RECEIPT)?
        != Some(serde_json::to_value(&receipt).map_err(|_| invalid())?)
    {
        return Err(InstallError::InventoryChanged.into());
    }
    tx.commit().map_err(storage)?;
    Ok(())
}
pub fn abort_consumer_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<bool> {
    let state = load_state(trust, store)?;
    let Some(pending) = read::<Pending>(&store.connection, PENDING)? else {
        return Ok(false);
    };
    validate(&pending, &state)?;
    if pending.orphans.is_empty() {
        return Err(invalid());
    }
    // Usable current trust is mandatory after restore. Explicit abort may bind
    // that new scope, but must keep the original profile/inventories and bytes.
    let scope = trust_scope(&store.connection)?;
    let expected = digest(&pending)?;
    guard_on(&store.connection, &state, &pending, Some(&expected), &scope)?;
    physical(store, &state, &pending)?;
    commit_receipt(store, &state, &pending, &expected, &scope, "aborted")?;
    Ok(true)
}
#[cfg(test)]
pub(crate) fn apply_fixture_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: ConsumerReconciliationPlan,
    hook: impl FnMut(ConsumerReconciliationPhase, &mut DataStore) -> Result<()>,
) -> Result<ConsumerReconciliationSummary> {
    apply_with_hook(trust, store, plan, hook)
}

#[cfg(test)]
mod effect_tests {
    use super::*;
    #[test]
    fn frozen_metadata_is_checked_inside_the_held_source_callback_before_native_quarantine() {
        let profile = std::env::temp_dir().join(format!(
            "flowtools-consumer-effect-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&profile).unwrap();
        let store = DataStore::open(&profile.join("runtime.sqlite")).unwrap();
        let layout = files::Layout::open_for_tool_consumers(&store, true)
            .unwrap()
            .unwrap();
        let source = layout.stage(&uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("package.zip"), b"UNCHANGED_ORPHAN").unwrap();
        store.connection.execute("INSERT INTO core_metadata(key,value) VALUES('package-trust-effect-test','original')",[]).unwrap();
        let metadata = MetadataGuard::capture(&store.connection).unwrap();
        let orphans = layout.discover_orphans(&Vec::<Record>::new()).unwrap();
        assert_eq!(orphans.len(), 1);
        let result=layout.quarantine_orphan_guarded(&orphans[0],|| {
            store.connection.execute("UPDATE core_metadata SET value='changed' WHERE key='package-trust-effect-test'",[]).unwrap();
            metadata.check(&store.connection)
        });
        assert_eq!(result.err(), Some(InstallError::InventoryChanged));
        assert!(source.exists());
        assert_eq!(
            std::fs::read(source.join("package.zip")).unwrap(),
            b"UNCHANGED_ORPHAN"
        );
        assert!(std::fs::read_dir(layout.root.join("quarantine"))
            .unwrap()
            .next()
            .is_none());
        drop(layout);
        drop(store);
        let resolved = std::fs::canonicalize(&profile).unwrap();
        assert_eq!(
            resolved.parent().unwrap(),
            std::fs::canonicalize(std::env::temp_dir()).unwrap()
        );
        assert!(resolved
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("flowtools-consumer-effect-")
            .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok_and(|id| id.to_string() == s)));
        std::fs::remove_dir_all(resolved).unwrap();
    }
}
