//! Separate inert signed-plugin/tool installation domain. Legacy G5 v2 bytes
//! and tool-free locks are unchanged. This is never an execution/lease API.
pub mod reconciliation;
use super::*;
use crate::packages::{
    dependencies::tools::{
        plan_signed_tools, SignedToolArchiveLock, SignedToolDependencyRequest,
        VerifiedToolDependencyPlan,
    },
    tools::install::{
        consumer_sources::{self, HeldToolArchives},
        ToolArchivePin, ToolInstallError,
    },
    trust::TrustSnapshot,
};
use base64::{engine::general_purpose, Engine};
use files::{BlobJournal, BlobRecord};

const KEY: &str = "tool-consumer-inventory-v1";
const PENDING: &str = "tool-consumer-install-pending-v1";
type Result<T> = std::result::Result<T, ToolInstallError>;
fn invalid() -> ToolInstallError {
    InstallError::StateInvalid.into()
}
fn storage(_: rusqlite::Error) -> ToolInstallError {
    InstallError::Storage.into()
}
fn digest(value: &impl Serialize) -> Result<String> {
    Ok(super::json_digest(value)?)
}
fn profile(store: &DataStore) -> Result<String> {
    let path = store.database_path().ok_or(InstallError::ProfileRequired)?;
    let path = std::fs::canonicalize(path).map_err(|_| InstallError::Storage)?;
    Ok(hash(path.as_os_str().as_encoded_bytes()))
}
fn uuid(text: &str) -> bool {
    uuid::Uuid::parse_str(text).is_ok_and(|id| id.to_string() == text)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    payload: PackagePayload,
    envelope: String,
    lock: SignedToolArchiveLock,
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
    companions_digest: String,
    scope_digest: String,
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
/// Opaque committed inert metadata; no accepted lease or executable identity.
/// ```compile_fail
/// use flowtools_runtime_core::packages::install::tool_consumers::InstalledToolConsumer;
/// let forged = InstalledToolConsumer { record: todo!() };
/// ```
pub struct InstalledToolConsumer {
    record: Record,
}
impl InstalledToolConsumer {
    pub fn pin(&self) -> SignedArchivePin {
        self.record.pin()
    }
    pub fn dependency_lock(&self) -> &SignedToolArchiveLock {
        &self.record.lock
    }
    pub fn installed_at(&self) -> u64 {
        self.record.installed_at
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConsumerInstallPhase {
    JournalCommitted,
    StagePrepared,
    ArtifactPublished,
    BeforeCommit,
    Committed,
}

fn validate_record(record: &Record) -> Result<()> {
    validate_package(&record.payload).map_err(|_| invalid())?;
    native(&record.payload.target)?;
    if record.payload.kind != PackageKind::Plugin
        || !uuid(&record.artifact_id)
        || !valid_digest(&record.root_digest)
        || record.installed_at > MAX_SAFE_INTEGER
        || record.envelope.len() > MAX_ENVELOPE_BYTES.div_ceil(3) * 4
        || record.lock.verified_digest().map_err(|_| invalid())? != record.lock.digest
        || record.lock.roots != vec![record.pin().identity()]
        || !record.lock.packages.contains(&record.pin())
        || record.lock.tools.is_empty()
        || record.lock.packages.len() > 512
        || record.lock.tools.len() > 512
        || serde_json::to_value(&record.lock.target).map_err(|_| invalid())?
            != serde_json::to_value(&record.payload.target).map_err(|_| invalid())?
    {
        return Err(invalid());
    }
    let envelope = decode_base64(&record.envelope).map_err(|_| invalid())?;
    let payload = decode_envelope(&envelope, PACKAGE_PAYLOAD_TYPE).map_err(|_| invalid())?;
    if strict_json(&payload.payload).map_err(|_| invalid())?
        != serde_json::to_value(&record.payload).map_err(|_| invalid())?
    {
        return Err(invalid());
    }
    Ok(())
}
fn inventory(store: &DataStore) -> Result<Inventory> {
    let state: Inventory = read(&store.connection, KEY)?.unwrap_or(Inventory {
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
    let mut versions = HashSet::new();
    for record in &state.records {
        validate_record(record)?;
        if !versions.insert((
            record.payload.publisher.clone(),
            record.payload.id.clone(),
            record.payload.version.clone(),
            record.payload.target.clone(),
        )) {
            return Err(invalid());
        }
    }
    Ok(state)
}
fn journal(connection: &rusqlite::Connection, state: &Inventory) -> Result<Option<Journal>> {
    let Some(pending) = read::<Journal>(connection, PENDING)? else {
        return Ok(None);
    };
    validate_record(&pending.record)?;
    if pending.format_version != 1
        || !uuid(&pending.id)
        || pending.before_digest != digest(state)?
        || !valid_digest(&pending.companions_digest)
        || !valid_digest(&pending.scope_digest)
        || (pending.new_artifact && pending.id != pending.record.artifact_id)
        || (pending.new_artifact
            && state
                .records
                .iter()
                .any(|r| r.payload.archive.sha256 == pending.record.payload.archive.sha256))
        || (!pending.new_artifact
            && !state.records.iter().any(|r| {
                r.pin() == pending.record.pin() && r.artifact_id == pending.record.artifact_id
            }))
    {
        return Err(invalid());
    }
    Ok(Some(pending))
}
pub(crate) fn ensure_idle(
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
fn companions_idle(connection: &rusqlite::Connection) -> Result<()> {
    let present: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key IN ('package-install-pending-v1','package-reconciliation-pending-v1','tool-archive-install-pending-v1','tool-archive-reconciliation-pending-v1','tool-image-materialization-pending-v1','tool-image-reconciliation-pending-v1'))",
        [], |r| r.get(0),
    ).map_err(storage)?;
    if present {
        Err(InstallError::RecoveryPending.into())
    } else {
        Ok(())
    }
}
fn selected_stamp(connection: &rusqlite::Connection, keys: &[&str]) -> Result<String> {
    let mut rows = Vec::new();
    for key in keys {
        rows.push((*key, read::<serde_json::Value>(connection, key)?));
    }
    digest(&rows)
}
fn companions(connection: &rusqlite::Connection) -> Result<String> {
    selected_stamp(
        connection,
        &[
            "package-install-inventory-v1",
            "tool-archive-inventory-v1",
            "tool-image-inventory-v1",
        ],
    )
}
fn scope(connection: &rusqlite::Connection) -> Result<String> {
    // Markers can be raw text/UUIDs; preserve their exact bounded bytes.
    let mut values = Vec::new();
    for key in [
        "package-trust-recovery-required",
        "recovery-generation",
        "package-trust-recovery-receipt-v1",
    ] {
        let value: Option<String> = connection.query_row(
            "SELECT CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=16777216 THEN value ELSE NULL END FROM core_metadata WHERE key=?1", [key], |r| r.get(0),
        ).optional().map_err(storage)?;
        values.push((key, value.map(|v| hash(v.as_bytes()))));
    }
    digest(&values)
}
fn known(snapshot: &TrustSnapshot, payload: &PackagePayload) -> bool {
    snapshot.known_versions.iter().any(|k| {
        k.kind == payload.kind
            && k.publisher == payload.publisher
            && k.id == payload.id
            && k.version == payload.version
            && k.target == payload.target
            && k.release_sequence == payload.release_sequence
            && k.package_digest == payload.archive.sha256
    })
}
// Called by the existing raw inventory reader with its already validated pins.
// No recursive admission, repair, adoption or alternate namespace parameter.
pub(crate) fn validate_retained(
    store: &DataStore,
    snapshot: &TrustSnapshot,
    tools: &[ToolArchivePin],
) -> std::result::Result<(), InstallError> {
    validate_retained_inner(store, snapshot, tools).map_err(|e| match e {
        ToolInstallError::Storage(e) => e,
        _ => InstallError::StateInvalid,
    })
}
pub(super) fn ensure_references(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> std::result::Result<(), InstallError> {
    let present: bool = store.connection.query_row("SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key IN ('tool-consumer-inventory-v1','tool-consumer-install-pending-v1'))", [], |r| r.get(0)).map_err(|_| InstallError::Storage)?;
    if present {
        consumer_sources::validate_inventory(trust, store).map_err(|e| match e {
            ToolInstallError::Storage(e) => e,
            _ => InstallError::StateInvalid,
        })?;
    }
    Ok(())
}
fn validate_retained_inner(
    store: &DataStore,
    snapshot: &TrustSnapshot,
    tools: &[ToolArchivePin],
) -> Result<()> {
    let (state, pending) = validated_references(store, snapshot, tools)?;
    if state.records.is_empty() && pending.is_none() {
        if let Some(layout) = files::Layout::open_for_tool_consumers(store, false)? {
            layout.assert_owned(&state.records, None)?;
            layout.disk_bytes()?;
        }
        return Ok(());
    }
    let layout = files::Layout::open_for_tool_consumers(store, false)?
        .ok_or(InstallError::ArtifactInvalid)?;
    layout.assert_owned(
        &state.records,
        pending.as_ref().map(|p| p as &dyn BlobJournal),
    )?;
    for record in &state.records {
        layout.read(record)?;
    }
    Ok(())
}
// Metadata/reference checks do not admit an orphan. Only the explicit fixed
// consumer reconciler uses this path before its complete native observation.
pub(crate) fn validate_reconciliation_references(
    store: &DataStore,
    snapshot: &TrustSnapshot,
    tools: &[ToolArchivePin],
) -> Result<()> {
    validated_references(store, snapshot, tools).map(|_| ())
}
fn validated_references(
    store: &DataStore,
    snapshot: &TrustSnapshot,
    tools: &[ToolArchivePin],
) -> Result<(Inventory, Option<Journal>)> {
    let state = inventory(store)?;
    let pending = journal(&store.connection, &state)?;
    if state.records.is_empty() && pending.is_none() {
        return Ok((state, pending));
    }
    let legacy = super::inventory(&store.connection)?;
    for record in &legacy.records {
        if !known(snapshot, &record.payload) {
            return Err(invalid());
        }
    }
    for record in state
        .records
        .iter()
        .chain(pending.iter().map(|j| &j.record))
    {
        if !known(snapshot, &record.payload) {
            return Err(invalid());
        }
        for pin in &record.lock.packages {
            if *pin != record.pin()
                && !state.records.iter().any(|r| r.pin() == *pin)
                && !legacy.records.iter().any(|r| r.pin() == *pin)
            {
                return Err(invalid());
            }
        }
        for pin in &record.lock.tools {
            if !record
                .lock
                .packages
                .iter()
                .any(|p| p.identity() == pin.consumer)
                || !tools.iter().any(|t| {
                    t.publisher == pin.publisher
                        && t.id == pin.id
                        && t.version == pin.version
                        && t.target == pin.target
                        && t.build_flavor == pin.build_flavor
                        && t.archive_digest == pin.package_digest
                })
            {
                return Err(invalid());
            }
        }
    }
    Ok((state, pending))
}
pub(crate) fn retained_usage(
    store: &DataStore,
) -> std::result::Result<files::DiskUsage, InstallError> {
    files::Layout::open_for_tool_consumers(store, false)?
        .map(|l| l.disk_usage_reserving(0))
        .transpose()
        .map(|v| v.unwrap_or_default())
}

#[derive(Clone, Copy)]
enum Provider<'a> {
    Legacy(&'a super::Record),
    Bound(&'a Record),
}
impl Provider<'_> {
    fn payload(&self) -> &PackagePayload {
        match self {
            Self::Legacy(r) => &r.payload,
            Self::Bound(r) => &r.payload,
        }
    }
    fn pin(&self) -> SignedArchivePin {
        match self {
            Self::Legacy(r) => r.pin(),
            Self::Bound(r) => r.pin(),
        }
    }
    fn envelope(&self) -> &str {
        match self {
            Self::Legacy(r) => &r.envelope,
            Self::Bound(r) => &r.envelope,
        }
    }
    fn record(&self) -> &dyn BlobRecord {
        match self {
            Self::Legacy(r) => *r,
            Self::Bound(r) => *r,
        }
    }
    fn packages(&self) -> &[SignedArchivePin] {
        match self {
            Self::Legacy(r) => &r.lock.packages,
            Self::Bound(r) => &r.lock.packages,
        }
    }
}
struct Resolved {
    plan: VerifiedToolDependencyPlan,
    _held: Vec<files::HeldBlob>,
    _legacy: Option<files::Layout>,
    _consumers: Option<files::Layout>,
    _tools: HeldToolArchives,
}
struct ResolveScope<'a> {
    state: &'a Inventory,
    pending: Option<&'a Journal>,
    compatibility: &'a ManifestCompatibility,
}
fn resolve(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    candidate: &CompatiblePluginArchive,
    scope: ResolveScope<'_>,
    clock: &mut dyn FnMut() -> u64,
    source: &mut impl FnMut(
        &PackageTrustStore,
        &mut DataStore,
        &mut dyn FnMut() -> u64,
    ) -> Result<HeldToolArchives>,
) -> Result<Resolved> {
    let ResolveScope {
        state,
        pending,
        compatibility,
    } = scope;
    let legacy = super::validated_inventory(trust, store)?;
    let legacy_layout = files::Layout::open(store, false)?;
    let consumer_layout = files::Layout::open_for_tool_consumers(store, false)?;
    if let Some(layout) = &legacy_layout {
        layout.assert_owned(&legacy.records, None)?;
        for record in &legacy.records {
            layout.read(record)?;
        }
    } else if !legacy.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    if let Some(layout) = &consumer_layout {
        layout.assert_owned(&state.records, pending.map(|p| p as &dyn BlobJournal))?;
        for record in &state.records {
            layout.read(record)?;
        }
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    let tools = source(trust, store, clock)?;
    let root = candidate.archive().descriptor().payload();
    let root_identity = DependencyIdentity {
        publisher: root.publisher.clone(),
        id: root.id.clone(),
    };
    let mut latest: BTreeMap<DependencyIdentity, Provider<'_>> = BTreeMap::new();
    for provider in legacy
        .records
        .iter()
        .map(Provider::Legacy)
        .chain(state.records.iter().map(Provider::Bound))
    {
        let identity = provider.pin().identity();
        if identity == root_identity || provider.payload().target != compatibility.target {
            continue;
        }
        if let Some(previous) = latest.get(&identity) {
            if previous.payload().release_sequence == provider.payload().release_sequence
                && previous.pin() != provider.pin()
            {
                return Err(invalid());
            }
        }
        if latest
            .get(&identity)
            .is_none_or(|p| p.payload().release_sequence < provider.payload().release_sequence)
        {
            latest.insert(identity, provider);
        }
    }
    let mut queue = VecDeque::from([candidate.manifest().clone()]);
    let mut seen = BTreeSet::from([root_identity]);
    let mut providers = Vec::new();
    let mut selected = Vec::new();
    let mut held = Vec::new();
    let mut manifest_bytes = root.manifest.size;
    let mut expanded_bytes = root.manifest.size + root.files.iter().map(|f| f.size).sum::<u64>();
    while let Some(manifest) = queue.pop_front() {
        for dependency in manifest["dependencies"]["services"]
            .as_array()
            .ok_or_else(invalid)?
        {
            let identity = DependencyIdentity {
                publisher: dependency["publisher"].as_str().ok_or_else(invalid)?.into(),
                id: dependency["id"].as_str().ok_or_else(invalid)?.into(),
            };
            if !seen.insert(identity.clone()) {
                continue;
            }
            let Some(provider) = latest.get(&identity) else {
                continue;
            };
            let payload = provider.payload();
            manifest_bytes = manifest_bytes
                .checked_add(payload.manifest.size)
                .ok_or(InstallError::BudgetExceeded)?;
            expanded_bytes = expanded_bytes
                .checked_add(payload.manifest.size)
                .and_then(|n| {
                    payload
                        .files
                        .iter()
                        .try_fold(n, |sum, f| sum.checked_add(f.size))
                })
                .ok_or(InstallError::BudgetExceeded)?;
            if manifest_bytes > 16_777_216 || expanded_bytes > 134_217_728 {
                return Err(InstallError::BudgetExceeded.into());
            }
            let layout = match provider {
                Provider::Legacy(_) => legacy_layout.as_ref(),
                Provider::Bound(_) => consumer_layout.as_ref(),
            }
            .ok_or(InstallError::ArtifactInvalid)?;
            let bytes = layout.hold(provider.record())?;
            let envelope = decode_base64(provider.envelope()).map_err(|_| invalid())?;
            let admitted = super::admit(
                trust,
                store,
                InertPackageBytes {
                    envelope: &envelope,
                    manifest: &bytes.manifest,
                    archive: &bytes.archive,
                },
                compatibility,
                clock(),
            )?;
            if digest(admitted.archive().descriptor().payload())? != digest(payload)? {
                return Err(invalid());
            }
            queue.push_back(admitted.manifest().clone());
            providers.push(admitted);
            selected.push(*provider);
            held.push(bytes);
        }
    }
    let plan = plan_signed_tools(
        trust,
        store,
        SignedToolDependencyRequest {
            candidate,
            providers: &providers.iter().collect::<Vec<_>>(),
            tools: &tools.archives.iter().collect::<Vec<_>>(),
            compatibility,
        },
        &mut *clock,
    )
    .map_err(InstallError::from)?;
    if plan.lock().tools.is_empty() {
        return Err(InstallError::Dependency(DependencyError::InvalidDeclaration).into());
    }
    for provider in selected {
        if provider
            .packages()
            .iter()
            .any(|pin| !plan.lock().packages.contains(pin))
            || matches!(provider, Provider::Bound(record) if record.lock.tools.iter().any(|pin| !plan.lock().tools.contains(pin)))
        {
            return Err(InstallError::Dependency(DependencyError::VersionConflict).into());
        }
    }
    Ok(Resolved {
        plan,
        _held: held,
        _legacy: legacy_layout,
        _consumers: consumer_layout,
        _tools: tools,
    })
}

// This transient exact guard freezes all relevant foreign metadata across SQL
// writes/native publication. Fresh trust admission precedes the final capture;
// normal observation-clock advancement is never mistaken for an old approval.
const FOREIGN_ROWS: &str = "key GLOB 'package-trust-*' OR key='recovery-generation' OR key IN ('package-install-inventory-v1','tool-archive-inventory-v1','tool-image-inventory-v1','package-install-pending-v1','package-reconciliation-pending-v1','tool-archive-install-pending-v1','tool-archive-reconciliation-pending-v1','tool-image-materialization-pending-v1','tool-image-reconciliation-pending-v1','tool-consumer-reconciliation-pending-v1')";
struct ForeignGuard {
    rows: Vec<(String, String)>,
}
impl ForeignGuard {
    fn capture(connection: &rusqlite::Connection) -> Result<Self> {
        let mut statement = connection.prepare(&format!("SELECT key,CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=16777216 THEN value ELSE NULL END FROM core_metadata WHERE {FOREIGN_ROWS} ORDER BY key")).map_err(storage)?;
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
    fn check(&self, connection: &rusqlite::Connection) -> Result<()> {
        let current = Self::capture(connection)?;
        if current.rows != self.rows {
            return Err(InstallError::InventoryChanged.into());
        }
        Ok(())
    }
}
fn guard(
    store: &DataStore,
    state: &Inventory,
    pending: &Journal,
    expected: &str,
    check_scope: bool,
) -> Result<()> {
    reconciliation::ensure_idle(&store.connection)?;
    companions_idle(&store.connection)?;
    if digest(&inventory(store)?)? != digest(state)?
        || companions(&store.connection)? != pending.companions_digest
        || (check_scope && scope(&store.connection)? != pending.scope_digest)
        || journal(&store.connection, state)?
            .as_ref()
            .map(digest)
            .transpose()?
            .as_deref()
            != Some(expected)
    {
        return Err(InstallError::InventoryChanged.into());
    }
    Ok(())
}
fn guard_on(connection: &rusqlite::Connection, state: &Inventory, expected: &str) -> Result<()> {
    reconciliation::ensure_idle(connection)?;
    let current = inventory_on(connection, &state.profile_digest)?;
    if digest(&current)? != digest(state)?
        || journal(connection, &current)?
            .as_ref()
            .map(digest)
            .transpose()?
            .as_deref()
            != Some(expected)
    {
        return Err(InstallError::InventoryChanged.into());
    }
    Ok(())
}
fn usage(store: &DataStore, extra: u64, groups: usize) -> Result<()> {
    let retained = crate::packages::tools::install::retained_tool_usage(store)?.combine(
        files::Layout::open(store, false)?
            .map(|l| l.disk_usage_reserving(0))
            .transpose()?
            .unwrap_or_default(),
    )?;
    if retained
        .bytes
        .checked_add(extra)
        .is_none_or(|n| n > files::MAX_DISK_BYTES)
        || retained
            .groups
            .checked_add(groups)
            .is_none_or(|n| n > files::MAX_GROUPS)
    {
        return Err(InstallError::BudgetExceeded.into());
    }
    Ok(())
}

pub fn install_inert_tool_consumer(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    bytes: InertPackageBytes<'_>,
    compatibility: &ManifestCompatibility,
    clock: impl FnMut() -> u64,
) -> Result<InstalledToolConsumer> {
    install_with_hook(
        trust,
        store,
        bytes,
        compatibility,
        clock,
        consumer_sources::read,
        |_, _| Ok(()),
    )
}
fn install_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    bytes: InertPackageBytes<'_>,
    compatibility: &ManifestCompatibility,
    mut clock: impl FnMut() -> u64,
    mut source: impl FnMut(
        &PackageTrustStore,
        &mut DataStore,
        &mut dyn FnMut() -> u64,
    ) -> Result<HeldToolArchives>,
    mut hook: impl FnMut(ConsumerInstallPhase, &mut DataStore) -> Result<()>,
) -> Result<InstalledToolConsumer> {
    native(&compatibility.target)?;
    companions_idle(&store.connection)?;
    ensure_idle(&store.connection)?;
    let mut state = inventory(store)?;
    let original_companions = companions(&store.connection)?;
    let original_scope = scope(&store.connection)?;
    let candidate = super::admit(trust, store, bytes, compatibility, clock())?;
    let resolved = resolve(
        trust,
        store,
        &candidate,
        ResolveScope {
            state: &state,
            pending: None,
            compatibility,
        },
        &mut clock,
        &mut source,
    )?;
    companions_idle(&store.connection)?;
    ensure_idle(&store.connection)?;
    if digest(&inventory(store)?)? != digest(&state)? {
        return Err(InstallError::InventoryChanged.into());
    }
    if original_companions != companions(&store.connection)?
        || original_scope != scope(&store.connection)?
    {
        return Err(InstallError::InventoryChanged.into());
    }
    let mut record = Record {
        payload: candidate.archive().descriptor().payload().clone(),
        envelope: general_purpose::STANDARD.encode(bytes.envelope),
        lock: resolved.plan.lock().clone(),
        root_digest: resolved.plan.root_digest().into(),
        installed_at: resolved.plan.verified_at(),
        artifact_id: uuid::Uuid::new_v4().to_string(),
    };
    let position = state.records.iter().position(|r| r.pin() == record.pin());
    if let Some(position) = position {
        let existing = &state.records[position];
        if existing.envelope == record.envelope && existing.lock == record.lock {
            return Ok(InstalledToolConsumer {
                record: existing.clone(),
            });
        }
        record.artifact_id = existing.artifact_id.clone();
    } else if state.records.len() >= MAX_RECORDS {
        return Err(InstallError::BudgetExceeded.into());
    }
    if serde_json::to_vec(&state).map_err(|_| invalid())?.len()
        + serde_json::to_vec(&record).map_err(|_| invalid())?.len()
        + 16
        > MAX_STATE_BYTES
    {
        return Err(InstallError::BudgetExceeded.into());
    }
    let new_artifact = position.is_none();
    let mut layout = files::Layout::open_for_tool_consumers(store, true)?
        .ok_or(InstallError::ProfileRequired)?;
    if layout.has_artifact(&record)? == new_artifact {
        return Err(InstallError::ArtifactInvalid.into());
    }
    usage(
        store,
        if new_artifact {
            record.payload.manifest.size + record.payload.archive.size + 512
        } else {
            0
        },
        usize::from(new_artifact),
    )?;
    let mut pending = Journal {
        format_version: 1,
        id: if new_artifact {
            record.artifact_id.clone()
        } else {
            uuid::Uuid::new_v4().to_string()
        },
        before_digest: digest(&state)?,
        companions_digest: original_companions,
        scope_digest: original_scope,
        record,
        new_artifact,
        prepared: false,
    };
    let initial = ForeignGuard::capture(&store.connection)?;
    let expected = digest(&pending)?;
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    initial.check(&tx)?;
    if digest(&inventory_on(&tx, &state.profile_digest)?)? != digest(&state)? {
        return Err(InstallError::InventoryChanged.into());
    }
    if read::<serde_json::Value>(&tx, PENDING)?.is_some() {
        return Err(InstallError::RecoveryPending.into());
    }
    write(&tx, PENDING, &pending)?;
    initial.check(&tx)?;
    guard_on(&tx, &state, &expected)?;
    tx.commit().map_err(storage)?;
    hook(ConsumerInstallPhase::JournalCommitted, store)?;
    initial.check(&store.connection)?;
    guard(store, &state, &pending, &expected, true)?;
    if new_artifact {
        layout.stage_bytes(&pending, bytes.manifest, bytes.archive)?;
    }
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    initial.check(&tx)?;
    guard_on(&tx, &state, &expected)?;
    pending.prepared = true;
    write(&tx, PENDING, &pending)?;
    let prepared = digest(&pending)?;
    initial.check(&tx)?;
    guard_on(&tx, &state, &prepared)?;
    tx.commit().map_err(storage)?;
    hook(ConsumerInstallPhase::StagePrepared, store)?;
    initial.check(&store.connection)?;
    guard(store, &state, &pending, &prepared, true)?;
    if new_artifact {
        layout.publish(&pending)?;
    }
    hook(ConsumerInstallPhase::ArtifactPublished, store)?;
    hook(ConsumerInstallPhase::BeforeCommit, store)?;
    guard(store, &state, &pending, &prepared, true)?;
    let fresh = super::admit(trust, store, bytes, compatibility, clock())?;
    let final_resolved = resolve(
        trust,
        store,
        &fresh,
        ResolveScope {
            state: &state,
            pending: Some(&pending),
            compatibility,
        },
        &mut clock,
        &mut source,
    )?;
    if final_resolved.plan.lock() != &pending.record.lock {
        return Err(InstallError::InventoryChanged.into());
    }
    guard(store, &state, &pending, &prepared, true)?;
    layout.read(&pending.record)?;
    layout.assert_owned(&state.records, Some(&pending))?;
    usage(store, 0, 0)?;
    let final_guard = ForeignGuard::capture(&store.connection)?;
    // Do not mutate the original pending record before comparing its exact SQL
    // digest. Current trust/time belongs only in the committed record.
    let mut committed = pending.record.clone();
    committed.root_digest = final_resolved.plan.root_digest().into();
    committed.installed_at = final_resolved.plan.verified_at();
    if let Some(position) = position {
        state.records[position] = committed.clone();
    } else {
        state.records.push(committed.clone());
    }
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    final_guard.check(&tx)?;
    if journal(&tx, &inventory_on(&tx, &state.profile_digest)?)?
        .as_ref()
        .map(digest)
        .transpose()?
        .as_deref()
        != Some(prepared.as_str())
    {
        return Err(InstallError::InventoryChanged.into());
    }
    write(&tx, KEY, &state)?;
    // Inventory-write triggers must not replace the exact pending record and
    // have that replacement erased by the following DELETE.
    final_guard.check(&tx)?;
    if read::<Journal>(&tx, PENDING)?
        .as_ref()
        .map(digest)
        .transpose()?
        .as_deref()
        != Some(prepared.as_str())
    {
        return Err(InstallError::InventoryChanged.into());
    }
    super::metadata::remove_pending(&tx, PENDING)?;
    final_guard.check(&tx)?;
    if read::<serde_json::Value>(&tx, PENDING)?.is_some()
        || digest(&inventory_on(&tx, &state.profile_digest)?)? != digest(&state)?
    {
        return Err(InstallError::InventoryChanged.into());
    }
    tx.commit().map_err(storage)?;
    hook(ConsumerInstallPhase::Committed, store)?;
    drop(resolved);
    drop(final_resolved);
    Ok(InstalledToolConsumer { record: committed })
}
fn inventory_on(connection: &rusqlite::Connection, profile: &str) -> Result<Inventory> {
    let state = read::<Inventory>(connection, KEY)?.unwrap_or(Inventory {
        format_version: 1,
        profile_digest: profile.into(),
        records: vec![],
    });
    if state.format_version != 1
        || state.profile_digest != profile
        || state.records.len() > MAX_RECORDS
    {
        return Err(invalid());
    }
    for record in &state.records {
        validate_record(record)?;
    }
    Ok(state)
}

/// Metadata inspection never creates/adopts a package or executable identity.
pub fn inspect_inert_tool_consumers(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<Vec<InstalledToolConsumer>> {
    companions_idle(&store.connection)?;
    ensure_idle(&store.connection)?;
    consumer_sources::validate_inventory(trust, store)?;
    Ok(inventory(store)?
        .records
        .into_iter()
        .map(|record| InstalledToolConsumer { record })
        .collect())
}
pub fn abort_inert_tool_consumer_install(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<bool> {
    reconciliation::ensure_idle(&store.connection)?;
    companions_idle(&store.connection)?;
    // Current trust must be usable after any restore/copy recovery. Explicit
    // abort needs provenance/bytes, not renewed permission to use a revoked tool.
    consumer_sources::validate_inventory(trust, store)?;
    let state = inventory(store)?;
    let Some(pending) = journal(&store.connection, &state)? else {
        return Ok(false);
    };
    let expected = digest(&pending)?;
    guard(store, &state, &pending, &expected, false)?;
    let frozen = ForeignGuard::capture(&store.connection)?;
    if let Some(mut layout) = files::Layout::open_for_tool_consumers(store, false)? {
        layout.assert_owned(&state.records, Some(&pending))?;
        usage(store, 0, 0)?;
        frozen.check(&store.connection)?;
        layout.quarantine(&pending)?;
    } else if pending.prepared {
        return Err(InstallError::ArtifactInvalid.into());
    }
    frozen.check(&store.connection)?;
    guard(store, &state, &pending, &expected, false)?;
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    frozen.check(&tx)?;
    guard_on(&tx, &state, &expected)?;
    super::metadata::remove_pending(&tx, PENDING)?;
    frozen.check(&tx)?;
    if read::<serde_json::Value>(&tx, PENDING)?.is_some()
        || digest(&inventory_on(&tx, &state.profile_digest)?)? != digest(&state)?
    {
        return Err(InstallError::InventoryChanged.into());
    }
    tx.commit().map_err(storage)?;
    Ok(true)
}
#[cfg(test)]
pub(crate) fn install_fixture_with_hook(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    bytes: InertPackageBytes<'_>,
    compatibility: &ManifestCompatibility,
    clock: impl FnMut() -> u64,
    hook: impl FnMut(ConsumerInstallPhase, &mut DataStore) -> Result<()>,
) -> Result<InstalledToolConsumer> {
    install_with_hook(
        trust,
        store,
        bytes,
        compatibility,
        clock,
        consumer_sources::read_fixture,
        hook,
    )
}
