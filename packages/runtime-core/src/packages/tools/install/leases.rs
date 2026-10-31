//! T0-only accepted artifact retention for fixed T1 catalog jobs. These held
//! references do not grant tool execution, expose paths or activate a package.
use super::*;
use crate::{
    catalog::BuiltinCatalog,
    dependencies::{DependencyLock, ToolCandidate, ToolPin},
    protocol::{ErrorCode, JobSnapshot, JobState},
    services::ServiceRegistry,
};
use images::lease_source::{self, HeldImage, ImageReference};
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};

const PROFILE: &str = "accepted-t1-tool-artifacts-v1";
#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixtureExit {
    BeforeAcceptCommit,
    AcceptCommitted,
    TerminalCommitted,
    ReleaseCommitted,
}
#[cfg(test)]
std::thread_local! {
    static FIXTURE_EXIT: std::cell::Cell<Option<FixtureExit>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(crate) fn exit_fixture_at(point: FixtureExit) {
    FIXTURE_EXIT.set(Some(point));
}
#[cfg(test)]
fn fixture_barrier(point: FixtureExit) {
    if FIXTURE_EXIT.get() == Some(point) {
        std::process::exit(85);
    }
}
const ROWS: &str = "key GLOB 'package-trust-*' OR key='recovery-generation' OR key IN ('package-install-inventory-v1','package-install-pending-v1','package-reconciliation-pending-v1','tool-archive-inventory-v1','tool-archive-install-pending-v1','tool-archive-reconciliation-pending-v1','tool-image-inventory-v1','tool-image-materialization-pending-v1','tool-image-reconciliation-pending-v1','tool-consumer-inventory-v1','tool-consumer-install-pending-v1','tool-consumer-reconciliation-pending-v1')";
fn failed(error: ToolInstallError) -> ErrorCode {
    match error {
        ToolInstallError::Storage(InstallError::RecoveryPending)
        | ToolInstallError::Admission(ToolAdmissionError::Package(
            packages::PackageError::TrustRecoveryRequired,
        )) => ErrorCode::RecoveryPending,
        ToolInstallError::Storage(InstallError::Storage)
        | ToolInstallError::Admission(ToolAdmissionError::Package(
            packages::PackageError::TrustStorageFailed,
        )) => ErrorCode::StorageFailed,
        ToolInstallError::Storage(InstallError::BudgetExceeded) => ErrorCode::BudgetExceeded,
        _ => ErrorCode::DependencyArtifactMismatch,
    }
}
fn generation(store: &DataStore) -> std::result::Result<Option<String>, ErrorCode> {
    let generation: Option<String> = store
        .connection
        .query_row(
            "SELECT value FROM core_metadata WHERE key='recovery-generation'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::StorageFailed)?;
    if generation
        .as_ref()
        .is_some_and(|value| value.is_empty() || value.len() > 128)
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    Ok(generation)
}
fn image_matches(tool: &ToolPin, image: &ImageReference) -> bool {
    let pin = image.pin();
    tool.publisher == pin.publisher
        && tool.id == pin.id
        && tool.version == pin.version
        && tool.build_flavor == pin.build_flavor
        && tool.digest == pin.archive_digest
        && serde_json::to_value(&tool.target).ok() == serde_json::to_value(&pin.target).ok()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StoredToolLease {
    format_version: u16,
    profile: String,
    digest: String,
    profile_digest: String,
    recovery_generation: Option<String>,
    run_id: String,
    caller: String,
    plugin_id: String,
    command_id: String,
    package_digest: String,
    dependency_lock: String,
    grant_epoch: u32,
    deadline: f64,
    retained: bool,
    images: Vec<ImageReference>,
}
impl StoredToolLease {
    fn digest(&self) -> std::result::Result<String, ErrorCode> {
        let mut value = serde_json::to_value(self).map_err(|_| ErrorCode::StoreCorrupt)?;
        value
            .as_object_mut()
            .ok_or(ErrorCode::StoreCorrupt)?
            .remove("digest");
        Ok(crate::catalog::digest(&value))
    }
    pub(crate) fn validate(
        &self,
        job: &JobSnapshot,
        lock: &DependencyLock,
    ) -> std::result::Result<(), ErrorCode> {
        if self.format_version != 1
            || self.profile != PROFILE
            || self.digest()? != self.digest
            || !packages::valid_digest(&self.profile_digest)
            || !packages::valid_digest(&self.package_digest)
            || !packages::valid_digest(&self.dependency_lock)
            || self.caller.is_empty()
            || self.caller.len() > 256
            || !crate::dependencies::valid_dependency_id(&self.plugin_id)
            || !crate::dependencies::valid_dependency_id(&self.command_id)
            || self
                .recovery_generation
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 128)
            || self.run_id != job.run_id
            || !uuid(&self.run_id)
            || self.caller != job.root_caller
            || self.plugin_id != job.plugin_id
            || self.command_id != job.command_id
            || self.package_digest != job.package_digest
            || self.grant_epoch != job.grant_epoch
            || !self.deadline.is_finite()
            || self.deadline != job.deadline
            || self.dependency_lock != job.dependency_lock
            || self.dependency_lock != lock.digest
            || lock
                .verified_digest()
                .map_err(|_| ErrorCode::StoreCorrupt)?
                != lock.digest
            || self.images.iter().any(|image| !image.valid())
            || self.images.is_empty()
            || self.images.len() > MAX_RECORDS
            || lock.tools.is_empty()
            || (!self.retained
                && !matches!(
                    job.state,
                    JobState::Succeeded | JobState::Failed | JobState::Cancelled
                ))
            || lock.tools.iter().any(|tool| {
                !lock
                    .packages
                    .iter()
                    .any(|pin| pin.identity() == tool.consumer)
                    || !self.images.iter().any(|image| image_matches(tool, image))
            })
            || self
                .images
                .iter()
                .any(|image| !lock.tools.iter().any(|tool| image_matches(tool, image)))
        {
            return Err(ErrorCode::StoreCorrupt);
        }
        let mut digests = BTreeSet::new();
        if self
            .images
            .iter()
            .any(|image| !digests.insert(image.pin().archive_digest.as_str()))
        {
            return Err(ErrorCode::StoreCorrupt);
        }
        Ok(())
    }
    pub(crate) fn retained(&self) -> bool {
        self.retained
    }
    pub(crate) fn references(&self) -> &[ImageReference] {
        &self.images
    }
    pub(crate) fn matches_scope(&self, store: &DataStore) -> std::result::Result<bool, ErrorCode> {
        Ok(self.profile_digest == profile(store).map_err(failed)?
            && self.recovery_generation == generation(store)?)
    }
}
pub(crate) struct ToolLease {
    stored: StoredToolLease,
    _held: Vec<Arc<HeldImage>>,
}
impl ToolLease {
    pub(crate) fn dormant(stored: StoredToolLease) -> Self {
        Self {
            stored,
            _held: Vec::new(),
        }
    }
    pub(crate) fn stored(&self) -> &StoredToolLease {
        &self.stored
    }
    pub(crate) fn mark_released(&mut self) -> std::result::Result<(), ErrorCode> {
        self.stored.retained = false;
        self.stored.digest = self.stored.digest()?;
        Ok(())
    }
    pub(crate) fn retain(&mut self) -> std::result::Result<(), ErrorCode> {
        self.stored.retained = true;
        self.stored.digest = self.stored.digest()?;
        Ok(())
    }
    pub(crate) fn drop_released_handles(&mut self) {
        if !self.stored.retained {
            self._held.clear();
        }
    }
}
pub(crate) struct PreparedArtifacts {
    lock: DependencyLock,
    profile_digest: String,
    recovery_generation: Option<String>,
    held: Vec<Arc<HeldImage>>,
    metadata_stamp: String,
}
impl PreparedArtifacts {
    pub(crate) fn lock(&self) -> &DependencyLock {
        &self.lock
    }
    pub(crate) fn metadata_stamp(&self) -> &str {
        &self.metadata_stamp
    }
    pub(crate) fn bind(self, job: &JobSnapshot) -> std::result::Result<ToolLease, ErrorCode> {
        let mut stored = StoredToolLease {
            format_version: 1,
            profile: PROFILE.into(),
            digest: String::new(),
            profile_digest: self.profile_digest,
            recovery_generation: self.recovery_generation,
            run_id: job.run_id.clone(),
            caller: job.root_caller.clone(),
            plugin_id: job.plugin_id.clone(),
            command_id: job.command_id.clone(),
            package_digest: job.package_digest.clone(),
            dependency_lock: self.lock.digest.clone(),
            grant_epoch: job.grant_epoch,
            deadline: job.deadline,
            retained: true,
            images: self
                .held
                .iter()
                .map(|held| held.reference().clone())
                .collect(),
        };
        stored.digest = stored.digest()?;
        stored.validate(job, &self.lock)?;
        Ok(ToolLease {
            stored,
            _held: self.held,
        })
    }
}
pub(crate) struct ToolAuthority {
    trust: PackageTrustStore,
    held: BTreeMap<String, Weak<HeldImage>>,
    #[cfg(test)]
    fixture_clock: Option<Arc<std::sync::atomic::AtomicU64>>,
}
impl ToolAuthority {
    pub(crate) fn new(trust: PackageTrustStore) -> Self {
        Self {
            trust,
            held: BTreeMap::new(),
            #[cfg(test)]
            fixture_clock: None,
        }
    }
    #[cfg(test)]
    pub(crate) fn fixture(
        trust: PackageTrustStore,
        clock: Arc<std::sync::atomic::AtomicU64>,
    ) -> Self {
        Self {
            trust,
            held: BTreeMap::new(),
            fixture_clock: Some(clock),
        }
    }
    fn clock(&self) -> std::result::Result<Box<dyn FnMut() -> u64>, ErrorCode> {
        #[cfg(test)]
        if let Some(clock) = self.fixture_clock.clone() {
            return Ok(Box::new(move || {
                clock.load(std::sync::atomic::Ordering::SeqCst)
            }));
        }
        // Signed-package clocks are UTC seconds, separate from job milliseconds.
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ErrorCode::DependencyArtifactMismatch)?;
        Ok(Box::new(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs())
        }))
    }
    fn sources(
        &self,
        store: &mut DataStore,
        exact: Option<&[ToolArchivePin]>,
    ) -> std::result::Result<consumer_sources::HeldToolArchives, ErrorCode> {
        images::require_no_pending(&store.connection).map_err(|e| failed(e.into()))?;
        let mut clock = self.clock()?;
        #[cfg(test)]
        if self.fixture_clock.is_some() {
            return match exact {
                Some(pins) => {
                    consumer_sources::read_exact_fixture(&self.trust, store, pins, &mut clock)
                }
                None => consumer_sources::read_fixture(&self.trust, store, &mut clock),
            }
            .map_err(failed);
        }
        match exact {
            Some(pins) => consumer_sources::read_exact(&self.trust, store, pins, &mut clock),
            None => consumer_sources::read(&self.trust, store, &mut clock),
        }
        .map_err(failed)
    }
    fn candidates(
        sources: &consumer_sources::HeldToolArchives,
    ) -> std::result::Result<Vec<ToolCandidate>, ErrorCode> {
        sources
            .archives
            .iter()
            .map(|archive| {
                let receipt = archive.archive().descriptor();
                let payload = receipt.payload();
                Ok(ToolCandidate {
                    publisher: payload.publisher.clone(),
                    id: payload.id.clone(),
                    version: payload.version.clone(),
                    target: serde_json::from_value(
                        serde_json::to_value(&payload.target)
                            .map_err(|_| ErrorCode::DependencyInvalid)?,
                    )
                    .map_err(|_| ErrorCode::DependencyInvalid)?,
                    build_flavor: payload.build_flavor.clone(),
                    digest: receipt.package_digest().into(),
                })
            })
            .collect()
    }
    pub(crate) fn set_catalog(
        &self,
        store: &mut DataStore,
        services: &mut ServiceRegistry,
        catalog: &BuiltinCatalog,
    ) -> std::result::Result<(), ErrorCode> {
        let sources = self.sources(store, None)?;
        services.set_catalog_with_tools(catalog, &Self::candidates(&sources)?)
    }
    pub(crate) fn plan(
        &self,
        store: &mut DataStore,
        catalog: &BuiltinCatalog,
        ids: &[String],
    ) -> std::result::Result<crate::dependencies::DependencyPlan, ErrorCode> {
        let sources = self.sources(store, None)?;
        ServiceRegistry::dependency_plan_with_tools(catalog, ids, &Self::candidates(&sources)?)
    }
    pub(crate) fn prepare(
        &mut self,
        store: &mut DataStore,
        catalog: &BuiltinCatalog,
        ids: &[String],
    ) -> std::result::Result<PreparedArtifacts, ErrorCode> {
        let sources = self.sources(store, None)?;
        let lock = ServiceRegistry::plan_with_tools(catalog, ids, &Self::candidates(&sources)?)?;
        self.hold(store, lock, sources)
    }
    fn hold(
        &mut self,
        store: &mut DataStore,
        lock: DependencyLock,
        sources: consumer_sources::HeldToolArchives,
    ) -> std::result::Result<PreparedArtifacts, ErrorCode> {
        if lock.tools.is_empty() {
            return Err(ErrorCode::DependencyInvalid);
        }
        self.held.retain(|_, held| held.strong_count() != 0);
        let mut held = Vec::new();
        let mut selected = BTreeSet::new();
        for tool in &lock.tools {
            if !selected.insert(tool.digest.as_str()) {
                continue;
            }
            let admitted = sources
                .archives
                .iter()
                .find(|archive| archive.archive().descriptor().package_digest() == tool.digest)
                .ok_or(ErrorCode::DependencyArtifactMismatch)?;
            let current = lease_source::hold_existing(&self.trust, store, admitted, self.clock()?)
                .map_err(failed)?;
            if !image_matches(tool, current.reference()) {
                return Err(ErrorCode::DependencyArtifactMismatch);
            }
            let key = value_digest(current.reference()).map_err(failed)?;
            let current = match self.held.get(&key).and_then(Weak::upgrade) {
                Some(existing) => existing,
                None => {
                    let current = Arc::new(current);
                    self.held.insert(key, Arc::downgrade(&current));
                    current
                }
            };
            held.push(current);
        }
        // Recheck all supplied candidates, including unused ones, at a fresh T0
        // clock after the actual image reads and before recording acceptance.
        let receipts = sources
            .archives
            .iter()
            .map(|a| a.archive().descriptor())
            .collect::<Vec<_>>();
        let target = ManifestCompatibility::embedded()
            .map_err(|_| ErrorCode::DependencyPlatformMismatch)?
            .target;
        let mut clock = self.clock()?;
        self.trust
            .check_receipts(store, &receipts, &target, clock())
            .map_err(|e| failed(ToolInstallError::Admission(e.into())))?;
        images::require_no_pending(&store.connection).map_err(|e| failed(e.into()))?;
        Ok(PreparedArtifacts {
            lock,
            profile_digest: profile(store).map_err(failed)?,
            recovery_generation: generation(store)?,
            held,
            metadata_stamp: metadata_stamp(&store.connection)?,
        })
    }
    pub(crate) fn restore(
        &mut self,
        store: &mut DataStore,
        job: &JobSnapshot,
        lock: &DependencyLock,
        stored: &StoredToolLease,
    ) -> std::result::Result<ToolLease, ErrorCode> {
        stored.validate(job, lock)?;
        if stored.profile_digest != profile(store).map_err(failed)?
            || stored.recovery_generation != generation(store)?
        {
            return Err(ErrorCode::RecoveryPending);
        }
        let pins = stored
            .images
            .iter()
            .map(|image| image.pin().clone())
            .collect::<Vec<_>>();
        let sources = self.sources(store, Some(&pins))?;
        let prepared = self.hold(store, lock.clone(), sources)?;
        let mut restored = prepared.bind(job)?;
        if restored.stored.images != stored.images {
            return Err(ErrorCode::DependencyArtifactMismatch);
        }
        restored.stored = stored.clone();
        Ok(restored)
    }
    pub(crate) fn validate_retained(
        &self,
        store: &mut DataStore,
        stored: &StoredToolLease,
    ) -> std::result::Result<(), ErrorCode> {
        if !stored.matches_scope(store)? {
            return Err(ErrorCode::RecoveryPending);
        }
        lease_source::validate_references(&self.trust, store, stored.references()).map_err(failed)
    }
}
pub(crate) fn metadata_stamp(
    connection: &rusqlite::Connection,
) -> std::result::Result<String, ErrorCode> {
    let mut statement = connection.prepare(&format!("SELECT key,CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=16777216 THEN value ELSE NULL END FROM core_metadata WHERE {ROWS} ORDER BY key"))
        .map_err(|_| ErrorCode::StorageFailed)?;
    let mut rows = statement.query([]).map_err(|_| ErrorCode::StorageFailed)?;
    let (mut values, mut bytes) = (Vec::new(), 0usize);
    while let Some(row) = rows.next().map_err(|_| ErrorCode::StorageFailed)? {
        let key: String = row.get(0).map_err(|_| ErrorCode::StoreCorrupt)?;
        let value: String = row.get(1).map_err(|_| ErrorCode::StoreCorrupt)?;
        bytes = bytes
            .checked_add(key.len())
            .and_then(|n| n.checked_add(value.len()))
            .ok_or(ErrorCode::BudgetExceeded)?;
        if values.len() >= 32 || bytes > 67_108_864 {
            return Err(ErrorCode::BudgetExceeded);
        }
        values.push((key, packages::hash(value.as_bytes())));
    }
    value_digest(&values).map_err(failed)
}
pub(crate) fn persist_job(
    store: &DataStore,
    run: &str,
    value: &serde_json::Value,
    expected: Option<&str>,
) -> std::result::Result<(), ErrorCode> {
    let encoded = serde_json::to_string(value).map_err(|_| ErrorCode::StorageFailed)?;
    // Preserve the existing restore bound, rather than writing a row that the
    // next Host cannot read. No raw inputs enter this metadata-only record.
    if encoded.len() > 32_768 {
        return Err(ErrorCode::BudgetExceeded);
    }
    let tx =
        rusqlite::Transaction::new_unchecked(&store.connection, TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::StorageFailed)?;
    write_job_row(&tx, run, &encoded, expected)?;
    #[cfg(test)]
    if value["snapshot"]["state"] == "accepted" {
        fixture_barrier(FixtureExit::BeforeAcceptCommit);
    }
    tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
    #[cfg(test)]
    match value["snapshot"]["state"].as_str() {
        Some("accepted") => fixture_barrier(FixtureExit::AcceptCommitted),
        Some("succeeded") if value["toolArtifacts"]["retained"] == true => {
            fixture_barrier(FixtureExit::TerminalCommitted)
        }
        Some("succeeded") if value["toolArtifacts"]["retained"] == false => {
            fixture_barrier(FixtureExit::ReleaseCommitted)
        }
        _ => {}
    }
    Ok(())
}
// Supported restore already holds DataStore's outer transaction. Do not start
// another writer or discard historical roots when its recovery scope changes.
pub(crate) fn persist_restored_job(
    connection: &rusqlite::Connection,
    run: &str,
    value: &serde_json::Value,
) -> std::result::Result<(), ErrorCode> {
    let encoded = serde_json::to_string(value).map_err(|_| ErrorCode::StorageFailed)?;
    write_job_row(connection, run, &encoded, None)
}
fn write_job_row(
    connection: &rusqlite::Connection,
    run: &str,
    encoded: &str,
    expected: Option<&str>,
) -> std::result::Result<(), ErrorCode> {
    if connection.is_autocommit() {
        return Err(ErrorCode::StorageFailed);
    }
    if encoded.len() > 32_768 {
        return Err(ErrorCode::BudgetExceeded);
    }
    let before = metadata_stamp(connection)?;
    if expected.is_some_and(|expected| expected != before) {
        return Err(ErrorCode::DependencyArtifactMismatch);
    }
    let changes = connection.total_changes();
    let affected = connection.execute("INSERT INTO jobs(run_id,metadata) VALUES(?1,?2) ON CONFLICT(run_id) DO UPDATE SET metadata=excluded.metadata",
        rusqlite::params![run, encoded]).map_err(|_| ErrorCode::StorageFailed)?;
    let written: String = connection
        .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [run], |r| {
            r.get(0)
        })
        .map_err(|_| ErrorCode::StorageFailed)?;
    if affected != 1
        || changes.checked_add(1) != Some(connection.total_changes())
        || written != encoded
        || metadata_stamp(connection)? != before
    {
        return Err(ErrorCode::StorageFailed);
    }
    Ok(())
}
