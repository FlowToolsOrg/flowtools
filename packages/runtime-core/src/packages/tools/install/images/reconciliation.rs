//! Exact observation/quarantine of T0-owned image orphans after metadata restore.
//! No seal adoption, tool identity, accepted lease, deletion or process API.
use super::*;
use crate::packages::install::reconciliation::Kind;
use std::collections::BTreeMap;

const PENDING: &str = "tool-image-reconciliation-pending-v1";
const RECEIPT: &str = "tool-image-reconciliation-receipt-v1";
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pending {
    format_version: u16,
    id: String,
    profile_digest: String,
    before_digest: String,
    companions_digest: String,
    scope_digest: String,
    orphans: Vec<tree::Orphan>,
}
/// A T0 observation, never a client-deserialized cleanup or executable proof.
/// ```compile_fail
/// use flowtools_runtime_core::packages::tools::install::images::reconciliation::ImageReconciliationPlan;
/// let forged = ImageReconciliationPlan { pending: todo!() };
/// ```
pub struct ImageReconciliationPlan {
    pending: Pending,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ImageReconciliationSummary {
    pub artifacts: usize,
    pub stages: usize,
    pub bytes: u64,
}
impl ImageReconciliationPlan {
    pub fn summary(&self) -> ImageReconciliationSummary {
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
pub(crate) enum ImageReconciliationPhase {
    JournalCommitted,
    GroupMoved(usize),
    BeforeNodeProtection(usize, usize),
    GroupPrivatized(usize),
    BeforeCommit,
    Committed,
}
fn summary(pending: &Pending) -> ImageReconciliationSummary {
    ImageReconciliationSummary {
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
        bytes: pending.orphans.iter().map(|o| o.bytes()).sum(),
    }
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
    crate::packages::install::tool_consumers::ensure_idle(connection)?;
    super::super::reconciliation::ensure_idle(connection)?;
    let present:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key IN ('package-install-pending-v1','package-reconciliation-pending-v1','tool-archive-install-pending-v1','tool-image-materialization-pending-v1'))",[],|r|r.get(0)).map_err(storage)?;
    if present {
        Err(InstallError::RecoveryPending.into())
    } else {
        Ok(())
    }
}
fn companions(connection: &rusqlite::Connection) -> Result<String> {
    let mut values = BTreeMap::new();
    for key in ["package-install-inventory-v1", "tool-archive-inventory-v1"] {
        values.insert(key, read_on::<serde_json::Value>(connection, key)?);
    }
    // Preserve the original a6 stamp exactly when the new domain is absent.
    if let Some(value) = read_on::<serde_json::Value>(connection, "tool-consumer-inventory-v1")? {
        values.insert("tool-consumer-inventory-v1", Some(value));
    }
    value_digest(&values)
}
fn state(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<(super::super::Inventory, Inventory)> {
    companions_idle(&store.connection)?;
    let source = source_state(trust, store)?;
    let images = inventory(store, &source)?;
    Ok((source, images))
}
fn validate(pending: &Pending, images: &Inventory) -> Result<()> {
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
    let (mut sources, mut destinations) = (BTreeSet::new(), BTreeSet::new());
    let mut bytes = 0u64;
    for orphan in &pending.orphans {
        orphan.validate(&images.records)?;
        if !sources.insert((orphan.kind, &orphan.source))
            || !destinations.insert(orphan.destination())
        {
            return Err(invalid());
        }
        bytes = bytes
            .checked_add(orphan.bytes())
            .ok_or(InstallError::BudgetExceeded)?;
    }
    if bytes > files::MAX_DISK_BYTES {
        return Err(InstallError::BudgetExceeded.into());
    }
    Ok(())
}
fn matches(
    connection: &rusqlite::Connection,
    images: &Inventory,
    pending: &Pending,
    scope: &str,
) -> Result<()> {
    companions_idle(connection)?;
    if pending.profile_digest != images.profile_digest
        || pending.before_digest != value_digest(images)?
        || pending.companions_digest != companions(connection)?
        || scope != trust_stamp(connection)?
    {
        return Err(InstallError::InventoryChanged.into());
    }
    Ok(())
}
fn guard_on(
    connection: &rusqlite::Connection,
    images: &Inventory,
    pending: &Pending,
    expected: Option<&str>,
    scope: &str,
) -> Result<()> {
    let actual: Inventory = read_on(connection, KEY)?.unwrap_or(Inventory {
        format_version: 1,
        profile_digest: images.profile_digest.clone(),
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
// A short-lived exact compare guard, not an approval/cache or a policy revision.
// Every node effect checks the same metadata rows in SQLite without repeatedly
// parsing a whole, potentially large orphan tree. Extra/missing relevant rows or
// non-text values refuse; no caller-selected keys or namespace enter this query.
const OBSERVED_ROWS:&str="key GLOB 'package-trust-*' OR key='recovery-generation' OR key IN ('package-install-inventory-v1','tool-archive-inventory-v1','tool-image-inventory-v1','package-install-pending-v1','package-reconciliation-pending-v1','tool-archive-install-pending-v1','tool-archive-reconciliation-pending-v1','tool-image-materialization-pending-v1','tool-image-reconciliation-pending-v1','tool-consumer-inventory-v1','tool-consumer-install-pending-v1','tool-consumer-reconciliation-pending-v1')";
struct MetadataGuard {
    rows: Vec<(String, String)>,
}
impl MetadataGuard {
    fn capture(connection: &rusqlite::Connection) -> Result<Self> {
        let mut statement=connection.prepare(&format!("SELECT key,CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=16777216 THEN value ELSE NULL END FROM core_metadata WHERE {OBSERVED_ROWS} ORDER BY key")).map_err(storage)?;
        let mut cursor = statement.query([]).map_err(storage)?;
        let mut rows = Vec::new();
        let mut bytes = 0usize;
        while let Some(row) = cursor.next().map_err(storage)? {
            let key: String = row.get(0).map_err(storage)?;
            let value: String = row.get(1).map_err(storage)?;
            bytes = bytes
                .checked_add(key.len())
                .and_then(|n| n.checked_add(value.len()))
                .ok_or(InstallError::BudgetExceeded)?;
            if bytes > 67_108_864 || rows.len() >= 32 {
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
        let mut statement=connection.prepare_cached("SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key=?1 AND typeof(value)='text' AND CAST(value AS BLOB)=CAST(?2 AS BLOB))").map_err(|_|InstallError::Storage)?;
        for (key, value) in &self.rows {
            let same: bool = statement
                .query_row(rusqlite::params![key, value], |r| r.get(0))
                .map_err(|_| InstallError::Storage)?;
            if !same {
                return Err(InstallError::InventoryChanged);
            }
        }
        Ok(())
    }
}
fn physical(
    store: &DataStore,
    source: &super::super::Inventory,
    images: &Inventory,
    pending: &Pending,
) -> Result<()> {
    if let Some(raw) = files::Layout::open_for_tools(store, false)? {
        for record in &source.records {
            raw.read(record)?;
        }
    } else if !source.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    if let Some(layout) = tree::Layout::open(store, false)? {
        check_usage(store, &layout, 0, 0)?;
        layout.assert_image_orphans(&images.records, &pending.orphans)?;
        for record in &images.records {
            layout.read(record, &images.profile_digest)?;
        }
        for orphan in &pending.orphans {
            layout.check_image_orphan(orphan)?;
        }
    } else if !images.records.is_empty() || !pending.orphans.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    // No image namespace must not bypass the shared raw/plugin budget.
    let usage = super::super::retained_tool_usage(store)?.combine(
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
pub fn plan_image_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<ImageReconciliationPlan> {
    ensure_idle(&store.connection)?;
    let (source, images) = state(trust, store)?;
    let mut pending = Pending {
        format_version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        profile_digest: images.profile_digest.clone(),
        before_digest: value_digest(&images)?,
        companions_digest: companions(&store.connection)?,
        scope_digest: trust_stamp(&store.connection)?,
        orphans: vec![],
    };
    if let Some(layout) = tree::Layout::open(store, false)? {
        check_usage(store, &layout, 0, 0)?;
        for record in &images.records {
            layout.read(record, &images.profile_digest)?;
        }
        pending.orphans = layout.discover_image_orphans(&images.records)?;
    }
    validate(&pending, &images)?;
    physical(store, &source, &images, &pending)?;
    Ok(ImageReconciliationPlan { pending })
}
pub fn apply_image_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: ImageReconciliationPlan,
) -> Result<ImageReconciliationSummary> {
    apply_with_hook(trust, store, plan, |_, _| Ok(()))
}
fn apply_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: ImageReconciliationPlan,
    mut hook: impl FnMut(ImageReconciliationPhase, &mut DataStore) -> Result<()>,
) -> Result<ImageReconciliationSummary> {
    ensure_idle(&store.connection)?;
    let (source, images) = state(trust, store)?;
    let pending = plan.pending;
    validate(&pending, &images)?;
    matches(&store.connection, &images, &pending, &pending.scope_digest)?;
    physical(store, &source, &images, &pending)?;
    if pending.orphans.is_empty() {
        return Ok(summary(&pending));
    }
    let layout = tree::Layout::open(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    for orphan in &pending.orphans {
        if !layout.check_image_orphan(orphan)? {
            return Err(invalid());
        }
    }
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, &images, &pending, None, &pending.scope_digest)?;
    write(&tx, PENDING, &pending)?;
    let expected = value_digest(&pending)?;
    guard_on(
        &tx,
        &images,
        &pending,
        Some(&expected),
        &pending.scope_digest,
    )?;
    tx.commit().map_err(storage)?;
    hook(ImageReconciliationPhase::JournalCommitted, store)?;
    finish(trust, store, pending, &mut hook)
}
pub fn resume_image_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<Option<ImageReconciliationSummary>> {
    let (_, images) = state(trust, store)?;
    let Some(pending) = read::<Pending>(store, PENDING)? else {
        return Ok(None);
    };
    validate(&pending, &images)?;
    if pending.orphans.is_empty() {
        return Err(invalid());
    }
    Ok(Some(finish(trust, store, pending, &mut |_, _| Ok(()))?))
}
fn finish(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    pending: Pending,
    hook: &mut impl FnMut(ImageReconciliationPhase, &mut DataStore) -> Result<()>,
) -> Result<ImageReconciliationSummary> {
    let (source, images) = state(trust, store)?;
    let expected = value_digest(&pending)?;
    guard_on(
        &store.connection,
        &images,
        &pending,
        Some(&expected),
        &pending.scope_digest,
    )?;
    physical(store, &source, &images, &pending)?;
    let metadata = MetadataGuard::capture(&store.connection)?;
    let layout = tree::Layout::open(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    for (index, orphan) in pending.orphans.iter().enumerate() {
        metadata.check(&store.connection)?;
        layout.move_image_orphan(orphan, || metadata.check(&store.connection))?;
        hook(ImageReconciliationPhase::GroupMoved(index), store)?;
        let mut node = 0;
        layout.privatize_image_orphan(orphan, || {
            hook(
                ImageReconciliationPhase::BeforeNodeProtection(index, node),
                store,
            )
            .map_err(|_| InstallError::Storage)?;
            node += 1;
            metadata.check(&store.connection)
        })?;
        hook(ImageReconciliationPhase::GroupPrivatized(index), store)?;
    }
    hook(ImageReconciliationPhase::BeforeCommit, store)?;
    metadata.check(&store.connection)?;
    let (source, images) = state(trust, store)?;
    physical(store, &source, &images, &pending)?;
    layout.assert_owned(&images.records, None)?;
    for orphan in &pending.orphans {
        layout.require_image_orphan_private(orphan)?;
    }
    commit_receipt(
        store,
        &images,
        &pending,
        &expected,
        &pending.scope_digest,
        "completed",
    )?;
    hook(ImageReconciliationPhase::Committed, store)?;
    Ok(summary(&pending))
}
fn commit_receipt(
    store: &mut DataStore,
    images: &Inventory,
    pending: &Pending,
    expected: &str,
    scope: &str,
    outcome: &'static str,
) -> Result<()> {
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    guard_on(&tx, images, pending, Some(expected), scope)?;
    let receipt = Receipt {
        format_version: 1,
        outcome,
        plan: pending,
    };
    write(&tx, RECEIPT, &receipt)?;
    guard_on(&tx, images, pending, Some(expected), scope)?;
    crate::packages::install::metadata::remove_pending(&tx, PENDING)?;
    guard_on(&tx, images, pending, None, scope)?;
    if value_digest(&read_on::<serde_json::Value>(&tx, RECEIPT)?.ok_or_else(invalid)?)?
        != value_digest(&receipt)?
    {
        return Err(InstallError::InventoryChanged.into());
    }
    tx.commit().map_err(storage)?;
    Ok(())
}
/// Cancel without moving remaining sources or deleting bytes. Already moved
/// groups must first have all artifact read capabilities removed, including
/// after interruption partway through their node policies.
pub fn abort_image_reconciliation(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<bool> {
    let (source, images) = state(trust, store)?;
    let Some(pending) = read::<Pending>(store, PENDING)? else {
        return Ok(false);
    };
    validate(&pending, &images)?;
    if pending.orphans.is_empty() {
        return Err(invalid());
    }
    let scope = trust_stamp(&store.connection)?;
    let expected = value_digest(&pending)?;
    guard_on(
        &store.connection,
        &images,
        &pending,
        Some(&expected),
        &scope,
    )?;
    physical(store, &source, &images, &pending)?;
    let metadata = MetadataGuard::capture(&store.connection)?;
    let layout = tree::Layout::open(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    for orphan in &pending.orphans {
        if !layout.check_image_orphan(orphan)? {
            layout.privatize_image_orphan(orphan, || metadata.check(&store.connection))?;
            layout.require_image_orphan_private(orphan)?;
        }
    }
    metadata.check(&store.connection)?;
    physical(store, &source, &images, &pending)?;
    commit_receipt(store, &images, &pending, &expected, &scope, "aborted")?;
    Ok(true)
}
#[cfg(test)]
pub(crate) fn apply_fixture_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    plan: ImageReconciliationPlan,
    hook: impl FnMut(ImageReconciliationPhase, &mut DataStore) -> Result<()>,
) -> Result<ImageReconciliationSummary> {
    apply_with_hook(trust, store, plan, hook)
}
#[cfg(test)]
mod consumer_compatibility_tests {
    use super::*;
    #[test]
    fn absent_consumer_inventory_preserves_the_original_a6_companion_stamp() {
        let store = DataStore::memory().unwrap();
        assert_eq!(
            companions(&store.connection).unwrap(),
            "f4b7a820c8e2bdc24073a00a9da35987e53163276dc2a808ae5531df17baf25e"
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
            "f4b7a820c8e2bdc24073a00a9da35987e53163276dc2a808ae5531df17baf25e"
        );
    }
}
