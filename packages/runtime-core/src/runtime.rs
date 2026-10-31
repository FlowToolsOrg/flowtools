#[cfg(windows)]
use crate::packages::tools::install::leases::{
    self, PreparedArtifacts, StoredToolLease, ToolAuthority, ToolLease,
};
use crate::{
    broker::{BrokerSession, CapabilityBroker, CapabilityOperation, CommandIdentity, Grant, Scope},
    catalog::{digest, BuiltinCatalog},
    data::DataStore,
    dependencies::DependencyLock,
    policy::PolicyStore,
    private_jobs::PrivateJobs,
    protocol::*,
    services::{ServiceCallDiagnostic, ServiceRegistry, ServiceTarget},
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

#[path = "runtime_scheduling.rs"]
mod scheduling;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderJournal {
    format_version: u16,
    phase: String,
    active_catalog: String,
    pending_catalog: Option<String>,
    backup_id: String,
    disabled: Vec<crate::dependencies::DependencyIdentity>,
}

pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("System time")
        .as_millis() as f64
}

struct Session {
    connection: String,
    caller: String,
}
struct PlannedDependencies {
    lock: DependencyLock,
    #[cfg(windows)]
    artifacts: Option<PreparedArtifacts>,
}
struct Job {
    snapshot: JobSnapshot,
    owner_connection: String,
    background: bool,
    accepted_at: f64,
    started_at: f64,
    input_summary: InputSummary,
    action_digest: String,
    events: Vec<JobEvent>,
    broker_session: Option<BrokerSession>,
    idempotency_key: String,
    dependency_lock: Option<DependencyLock>,
    service_calls: Vec<ServiceCallDiagnostic>,
    #[cfg(windows)]
    tool_lease: Option<ToolLease>,
}
fn retains_tool_artifacts(job: &Job) -> bool {
    #[cfg(windows)]
    {
        job.tool_lease
            .as_ref()
            .is_some_and(|lease| lease.stored().retained())
    }
    #[cfg(not(windows))]
    {
        let _ = job;
        false
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredJob {
    format_version: u16,
    snapshot: JobSnapshot,
    background: bool,
    accepted_at: f64,
    started_at: f64,
    input_summary: InputSummary,
    action_digest: String,
    idempotency_key: String,
    events: Vec<JobEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dependency_lock: Option<DependencyLock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    service_calls: Vec<ServiceCallDiagnostic>,
    #[cfg(windows)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_artifacts: Option<StoredToolLease>,
}
impl StoredJob {
    #[cfg(windows)]
    fn validate_tool_artifacts(&self) -> Result<(), ErrorCode> {
        match (&self.tool_artifacts, &self.dependency_lock) {
            (Some(lease), Some(lock)) => lease.validate(&self.snapshot, lock),
            (None, Some(lock)) if lock.tools.is_empty() => Ok(()),
            (None, None) => Ok(()),
            _ => Err(ErrorCode::StoreCorrupt),
        }
    }
    fn from_job(job: &Job) -> Self {
        let mut snapshot = job.snapshot.clone();
        if let Some(ref mut result) = snapshot.result {
            if let ExecutionOutcome::Success { data, .. } = &mut result.outcome {
                *data = Value::Null;
            }
        }
        Self {
            format_version: 1,
            snapshot,
            background: job.background,
            accepted_at: job.accepted_at,
            started_at: job.started_at,
            input_summary: job.input_summary.clone(),
            action_digest: job.action_digest.clone(),
            idempotency_key: job.idempotency_key.clone(),
            events: job.events.clone(),
            dependency_lock: job.dependency_lock.clone(),
            service_calls: job.service_calls.clone(),
            #[cfg(windows)]
            tool_artifacts: job.tool_lease.as_ref().map(|lease| lease.stored().clone()),
        }
    }
}
fn persist_stored_job(
    store: &DataStore,
    stored: &StoredJob,
    expected: Option<&str>,
) -> Result<(), ErrorCode> {
    let value = serde_json::to_value(stored).map_err(|_| ErrorCode::StorageFailed)?;
    #[cfg(windows)]
    if stored.tool_artifacts.is_some() {
        return leases::persist_job(store, &stored.snapshot.run_id, &value, expected);
    }
    #[cfg(not(windows))]
    let _ = expected;
    store.save_job(&stored.snapshot.run_id, &value)
}
// Strict read-only retention for T0 package/image cleanup. Corrupt rows block
// cleanup; reading historical roots never creates a runnable lease or grant.
#[cfg(windows)]
pub(crate) fn retained_tool_leases(store: &DataStore) -> Result<Vec<StoredToolLease>, ErrorCode> {
    let mut leases = Vec::new();
    for value in store.load_jobs()? {
        let stored: StoredJob =
            serde_json::from_value(value).map_err(|_| ErrorCode::StoreCorrupt)?;
        if stored.format_version != 1
            || stored.snapshot.format_version != 1
            || uuid::Uuid::parse_str(&stored.snapshot.run_id).is_err()
            || !valid_key(&stored.idempotency_key)
        {
            return Err(ErrorCode::StoreCorrupt);
        }
        stored.validate_tool_artifacts()?;
        if let Some(lease) = stored.tool_artifacts.filter(StoredToolLease::retained) {
            leases.push(lease);
        }
    }
    Ok(leases)
}

/// Payload is transient runner input, never serializable diagnostics/history.
#[derive(Clone)]
pub struct RunSpec {
    pub run_id: String,
    pub plugin_id: String,
    pub command_id: String,
    pub input: Value,
    pub deadline: f64,
    pub package_digest: String,
    pub data: Option<DataSnapshot>,
}

struct ServiceInvocation {
    root: String,
    session: BrokerSession,
    spec: RunSpec,
    target: ServiceTarget,
    failure: Option<ErrorCode>,
}
pub struct ServiceRunSpec {
    pub spec: RunSpec,
    pub target: ServiceTarget,
    pub root_run_id: String,
    pub parent_run_id: String,
}

pub struct RuntimeCore {
    instance_id: String,
    tokens: HashMap<String, String>,
    sessions: HashMap<String, Session>,
    catalog: BuiltinCatalog,
    jobs: BTreeMap<String, Job>,
    keys: HashMap<(String, String), String>,
    pending: HashMap<String, RunSpec>,
    broker: CapabilityBroker,
    data: DataStore,
    policy: Option<PolicyStore>,
    management_only: bool,
    cold_started: bool,
    stopping: bool,
    private_jobs: Option<PrivateJobs>,
    services: ServiceRegistry,
    service_invocations: HashMap<String, ServiceInvocation>,
    retained_catalogs: Vec<BuiltinCatalog>,
    provider_candidates: HashMap<String, BuiltinCatalog>,
    #[cfg(windows)]
    tools: Option<ToolAuthority>,
}

impl RuntimeCore {
    // Host binds credentials to caller identities. Neither comes from the wire payload.
    pub fn validation(cli_token: String, desktop_token: String) -> Self {
        Self {
            instance_id: Uuid::new_v4().to_string(),
            tokens: HashMap::from([
                (cli_token, "validation-cli".into()),
                (desktop_token, "validation-desktop".into()),
            ]),
            sessions: HashMap::new(),
            catalog: BuiltinCatalog::embedded(),
            jobs: BTreeMap::new(),
            keys: HashMap::new(),
            pending: HashMap::new(),
            broker: CapabilityBroker::default(),
            data: DataStore::memory().expect("Validation data schema"),
            policy: None,
            management_only: false,
            cold_started: false,
            stopping: false,
            private_jobs: None,
            services: ServiceRegistry::default(),
            service_invocations: HashMap::new(),
            retained_catalogs: Vec::new(),
            provider_candidates: HashMap::new(),
            #[cfg(windows)]
            tools: None,
        }
    }

    pub fn with_data_store(mut self, store: DataStore) -> Self {
        self.data = store;
        self
    }

    pub fn managed(
        tokens: HashMap<String, String>,
        store: DataStore,
        management_only: bool,
        cold_started: bool,
    ) -> Result<Self, ErrorCode> {
        Self::managed_catalog(
            tokens,
            store,
            management_only,
            cold_started,
            BuiltinCatalog::embedded(),
        )
    }

    /// Build-owned T0 snapshots only. No wire method can import a catalog.
    pub fn managed_catalog(
        tokens: HashMap<String, String>,
        store: DataStore,
        management_only: bool,
        cold_started: bool,
        catalog: BuiltinCatalog,
    ) -> Result<Self, ErrorCode> {
        Self::managed_catalog_inner(
            tokens,
            store,
            management_only,
            cold_started,
            catalog,
            |core| core.refresh_service_catalog(),
        )
    }
    /// Explicit build-owned T0 wiring only. Ordinary Runtime Hosts use managed()
    /// and never select pins, reviewed builds or a tool authority from IPC/flags.
    /// This constructor does not install, activate or authorize native execution.
    #[cfg(windows)]
    pub fn managed_tool_catalog(
        tokens: HashMap<String, String>,
        store: DataStore,
        management_only: bool,
        cold_started: bool,
        catalog: BuiltinCatalog,
        trust: crate::packages::trust::PackageTrustStore,
    ) -> Result<Self, ErrorCode> {
        Self::managed_catalog_inner(
            tokens,
            store,
            management_only,
            cold_started,
            catalog,
            move |core| {
                core.tools = Some(ToolAuthority::new(trust));
                core.refresh_service_catalog()
            },
        )
    }
    #[cfg(all(windows, test))]
    pub(crate) fn managed_fixture_tool_catalog(
        tokens: HashMap<String, String>,
        store: DataStore,
        management_only: bool,
        catalog: BuiltinCatalog,
        trust: crate::packages::trust::PackageTrustStore,
        clock: std::sync::Arc<std::sync::atomic::AtomicU64>,
    ) -> Result<Self, ErrorCode> {
        Self::managed_catalog_inner(
            tokens,
            store,
            management_only,
            false,
            catalog,
            move |core| {
                core.tools = Some(ToolAuthority::fixture(trust, clock));
                core.refresh_service_catalog()
            },
        )
    }
    #[cfg(all(windows, test))]
    pub(crate) fn with_tool_fixture_store<T>(
        &mut self,
        action: impl FnOnce(&mut DataStore) -> T,
    ) -> T {
        action(&mut self.data)
    }
    fn managed_catalog_inner(
        tokens: HashMap<String, String>,
        mut store: DataStore,
        management_only: bool,
        cold_started: bool,
        catalog: BuiltinCatalog,
        setup: impl FnOnce(&mut Self) -> Result<(), ErrorCode>,
    ) -> Result<Self, ErrorCode> {
        let journal: Option<String> = store
            .connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='service-catalog-state-v1'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| ErrorCode::StorageFailed)?;
        let disabled = if let Some(text) = journal {
            let journal: ProviderJournal =
                serde_json::from_str(&text).map_err(|_| ErrorCode::StoreCorrupt)?;
            if journal.format_version != 1
                || journal.disabled.len() > 128
                || uuid::Uuid::parse_str(&journal.backup_id).is_err()
            {
                return Err(ErrorCode::StoreCorrupt);
            }
            if journal.phase != "committed"
                || journal.pending_catalog.is_some()
                || journal.active_catalog != catalog.snapshot_digest()
            {
                return Err(ErrorCode::RecoveryPending);
            }
            if journal.disabled.iter().any(|identity| {
                !catalog
                    .list()
                    .any(|(id, m)| id == &identity.id && m["publisher"] == identity.publisher)
            }) {
                return Err(ErrorCode::StoreCorrupt);
            }
            journal.disabled
        } else {
            Vec::new()
        };
        let policy =
            PolicyStore::load_snapshot(&mut store, &catalog.snapshot_digest(), catalog.clone())?;
        if cold_started && !policy.cold_start {
            return Err(ErrorCode::ColdStartDenied);
        }
        let mut core =
            Self::validation("unused-cli".into(), "unused-desktop".into()).with_data_store(store);
        core.tokens = tokens;
        core.catalog = catalog;
        setup(&mut core)?;
        core.services.restore_disabled(disabled);
        core.management_only = management_only;
        core.cold_started = cold_started;
        for record in &policy.records {
            core.broker.restore_policy(
                record.identity.clone(),
                record.epoch,
                record.grant.as_ref().map(PolicyStore::capability),
            )?;
        }
        core.policy = Some(policy);
        core.private_jobs = core.data.profile().map(PrivateJobs::open).transpose()?;
        core.restore_jobs()?;
        // Claimed-but-unreported attempts are never blindly replayed on restart.
        // Restore/copy mismatch blocks scheduling while ordinary recovery remains usable.
        if let Err(code) = crate::scheduling::store::ScheduleStore::recover(&mut core.data) {
            if code != ErrorCode::RecoveryPending {
                return Err(code);
            }
        }
        Ok(core)
    }

    fn persist_job(&self, job: &Job) -> Result<(), ErrorCode> {
        self.persist_job_inner(job, None)
    }
    fn persist_job_inner(&self, job: &Job, expected: Option<&str>) -> Result<(), ErrorCode> {
        if self.private_jobs.is_some() {
            persist_stored_job(&self.data, &StoredJob::from_job(job), expected)?;
        }
        Ok(())
    }
    fn refresh_service_catalog(&mut self) -> Result<(), ErrorCode> {
        match self.services.set_catalog(&self.catalog) {
            Ok(()) => Ok(()),
            Err(code) => {
                #[cfg(windows)]
                if let Some(tools) = self.tools.as_ref() {
                    return tools.set_catalog(&mut self.data, &mut self.services, &self.catalog);
                }
                Err(code)
            }
        }
    }
    fn plan_catalog(
        &mut self,
        catalog: &BuiltinCatalog,
        ids: &[String],
    ) -> Result<crate::dependencies::DependencyPlan, ErrorCode> {
        match ServiceRegistry::dependency_plan_with_tools(catalog, ids, &[]) {
            Ok(lock) => Ok(lock),
            Err(code) => {
                #[cfg(windows)]
                if let Some(tools) = self.tools.as_ref() {
                    return tools.plan(&mut self.data, catalog, ids);
                }
                Err(code)
            }
        }
    }
    fn plan_job_dependencies(&mut self, id: &str) -> Result<PlannedDependencies, ErrorCode> {
        let ids = [id.to_owned()];
        match ServiceRegistry::plan(&self.catalog, &ids) {
            Ok(lock) => Ok(PlannedDependencies {
                lock,
                #[cfg(windows)]
                artifacts: None,
            }),
            Err(code) => {
                #[cfg(windows)]
                if let Some(tools) = self.tools.as_mut() {
                    let artifacts = tools.prepare(&mut self.data, &self.catalog, &ids)?;
                    return Ok(PlannedDependencies {
                        lock: artifacts.lock().clone(),
                        artifacts: Some(artifacts),
                    });
                }
                Err(code)
            }
        }
    }
    fn restore_jobs(&mut self) -> Result<(), ErrorCode> {
        if self.private_jobs.is_none() {
            return Ok(());
        }
        for value in self.data.load_jobs()? {
            let mut stored: StoredJob =
                serde_json::from_value(value).map_err(|_| ErrorCode::StoreCorrupt)?;
            if stored.format_version != 1
                || uuid::Uuid::parse_str(&stored.snapshot.run_id).is_err()
                || !valid_key(&stored.idempotency_key)
            {
                return Err(ErrorCode::StoreCorrupt);
            }
            #[cfg(windows)]
            stored.validate_tool_artifacts()?;
            #[cfg(windows)]
            if let (Some(tools), Some(lease)) =
                (self.tools.as_ref(), stored.tool_artifacts.as_ref())
            {
                if lease.retained() {
                    tools.validate_retained(&mut self.data, lease)?;
                }
            }
            if let Some(lock) = &stored.dependency_lock {
                if lock
                    .verified_digest()
                    .map_err(|_| ErrorCode::StoreCorrupt)?
                    != lock.digest
                    || stored.snapshot.dependency_lock != lock.digest
                {
                    return Err(ErrorCode::StoreCorrupt);
                }
            } else if stored.snapshot.dependency_lock != "t1-no-dependencies-v1" {
                return Err(ErrorCode::StoreCorrupt);
            }
            if stored.service_calls.len() > 64
                || stored.service_calls.iter().any(|call| {
                    call.root_run_id != stored.snapshot.run_id
                        || call.root_caller != stored.snapshot.root_caller
                        || call.dependency_lock != stored.snapshot.dependency_lock
                        || !call.deadline.is_finite()
                        || call.deadline > stored.snapshot.deadline
                        || call.run_id == stored.snapshot.run_id
                        || call.deadline <= 0.0
                        || uuid::Uuid::parse_str(&call.run_id).is_err()
                        || uuid::Uuid::parse_str(&call.parent_run_id).is_err()
                        || !matches!(
                            call.state.as_str(),
                            "queued" | "running" | "succeeded" | "failed" | "interrupted"
                        )
                        || !crate::dependencies::valid_dependency_id(&call.service)
                        || !crate::dependencies::valid_dependency_id(&call.operation)
                })
            {
                return Err(ErrorCode::StoreCorrupt);
            }
            let run = stored.snapshot.run_id.clone();
            let calls: HashMap<_, _> = stored
                .service_calls
                .iter()
                .map(|call| (call.run_id.as_str(), call))
                .collect();
            if calls.len() != stored.service_calls.len() {
                return Err(ErrorCode::StoreCorrupt);
            }
            for call in &stored.service_calls {
                let lock = stored
                    .dependency_lock
                    .as_ref()
                    .ok_or(ErrorCode::StoreCorrupt)?;
                let parent = if call.parent_run_id == stored.snapshot.run_id {
                    let root = lock
                        .roots
                        .iter()
                        .find(|root| root.id == stored.snapshot.plugin_id)
                        .ok_or(ErrorCode::StoreCorrupt)?;
                    if call.consumer != *root {
                        return Err(ErrorCode::StoreCorrupt);
                    }
                    None
                } else {
                    let parent = calls
                        .get(call.parent_run_id.as_str())
                        .ok_or(ErrorCode::StoreCorrupt)?;
                    if call.consumer != parent.provider.identity()
                        || call.deadline > parent.deadline
                    {
                        return Err(ErrorCode::StoreCorrupt);
                    }
                    Some(*parent)
                };
                if !lock.services.iter().any(|pin| {
                    pin.consumer == call.consumer
                        && pin.provider == call.provider
                        && pin.service == call.service
                }) {
                    return Err(ErrorCode::StoreCorrupt);
                }
                let mut cursor = parent;
                let mut depth = 1;
                while let Some(ancestor) = cursor {
                    depth += 1;
                    if depth > 16 || ancestor.run_id == call.run_id {
                        return Err(ErrorCode::StoreCorrupt);
                    }
                    cursor = if ancestor.parent_run_id == stored.snapshot.run_id {
                        None
                    } else {
                        Some(
                            *calls
                                .get(ancestor.parent_run_id.as_str())
                                .ok_or(ErrorCode::StoreCorrupt)?,
                        )
                    };
                }
            }
            for call in &mut stored.service_calls {
                if matches!(call.state.as_str(), "queued" | "running") {
                    call.state = "interrupted".into();
                    call.failure_code = Some(ErrorCode::ExecutionInterrupted);
                }
            }
            let key = (
                stored.snapshot.root_caller.clone(),
                stored.idempotency_key.clone(),
            );
            if self.keys.insert(key, run.clone()).is_some() {
                return Err(ErrorCode::StoreCorrupt);
            }
            self.jobs.insert(
                run.clone(),
                Job {
                    snapshot: stored.snapshot,
                    owner_connection: String::new(),
                    background: stored.background,
                    accepted_at: stored.accepted_at,
                    started_at: stored.started_at,
                    input_summary: stored.input_summary,
                    action_digest: stored.action_digest,
                    idempotency_key: stored.idempotency_key,
                    events: stored.events,
                    broker_session: None,
                    dependency_lock: stored.dependency_lock,
                    service_calls: stored.service_calls,
                    #[cfg(windows)]
                    tool_lease: stored.tool_artifacts.map(ToolLease::dormant),
                },
            );
            self.persist_job(&self.jobs[&run])?;
            let job = &self.jobs[&run];
            if job.snapshot.state.terminal() {
                if retains_tool_artifacts(job) {
                    continue;
                }
                if now() >= job.accepted_at + 86_400_000.0 {
                    self.private_jobs.as_ref().unwrap().remove(&run, "output")?;
                }
                self.private_jobs.as_ref().unwrap().remove(&run, "input")?;
                continue;
            }
            if self.management_only {
                continue;
            }
            let snapshot = job.snapshot.clone();
            let background = job.background;
            let action_digest = job.action_digest.clone();
            let dependency_lock = job.dependency_lock.clone();
            #[cfg(windows)]
            let stored_artifacts = job.tool_lease.as_ref().map(|lease| lease.stored().clone());
            let catalog = self.catalog.clone();
            let contract = catalog.command(&snapshot.plugin_id, &snapshot.command_id);
            let authorization = contract.and_then(|(manifest, command)| {
                let identity =
                    CommandIdentity::from_manifest(&snapshot.root_caller, manifest, command);
                if digest(manifest) != snapshot.package_digest {
                    return Err(ErrorCode::GrantRevoked);
                }
                self.broker.reauthorize_accepted(
                    &identity,
                    command,
                    snapshot.grant_epoch,
                    now(),
                )?;
                self.managed_authorization(&identity, background)
                    .map_err(|code| {
                        if code == ErrorCode::ApprovalRequired && snapshot.grant_epoch != 0 {
                            ErrorCode::GrantRevoked
                        } else {
                            code
                        }
                    })?;
                if snapshot.deadline <= now() {
                    return Err(ErrorCode::Timeout);
                }
                if let Some(lock) = &dependency_lock {
                    for pin in &lock.packages {
                        if !catalog.list().any(|(id, m)| {
                            id == &pin.id
                                && m["publisher"] == pin.publisher
                                && m["version"] == pin.version
                                && digest(m) == pin.digest
                        }) {
                            return Err(ErrorCode::DependencyArtifactMismatch);
                        }
                    }
                }
                Ok(())
            });
            // Queued has never entered a runner. Running Base64 is proven pure and
            // deterministic; every other running operation needs manual review.
            let retryable = matches!(snapshot.state, JobState::Accepted | JobState::Queued)
                || (snapshot.state == JobState::Running
                    && snapshot.plugin_id == "plugin-base64-encoder");
            if background && retryable && authorization.is_ok() {
                #[cfg(windows)]
                if let Some(stored) = stored_artifacts {
                    let restored = match (self.tools.as_mut(), dependency_lock.as_ref()) {
                        (Some(tools), Some(lock)) => {
                            tools.restore(&mut self.data, &snapshot, lock, &stored)
                        }
                        _ => Err(ErrorCode::DependencyArtifactMismatch),
                    };
                    match restored {
                        Ok(lease) => self.jobs.get_mut(&run).unwrap().tool_lease = Some(lease),
                        Err(code) => {
                            self.finish_internal(&run, Err(code), Some(JobState::Interrupted));
                            continue;
                        }
                    }
                }
                if let Some(lock) = &dependency_lock {
                    self.services.accept(
                        &run,
                        lock.clone(),
                        snapshot.resources["maxOutputBytes"].as_u64().unwrap_or(0) as usize,
                    )?;
                }
                let input = self.private_jobs.as_ref().unwrap().read(&run, "input")?;
                let prepared = self.catalog.prepare_input_mode(
                    &snapshot.plugin_id,
                    &snapshot.command_id,
                    input.clone(),
                    true,
                )?;
                if prepared != input
                    || digest(
                        &json!({"package":snapshot.package_digest,"command":snapshot.command_id,"lock":snapshot.dependency_lock,"input":input,"background":background,"deadline":snapshot.deadline}),
                    ) != action_digest
                {
                    return Err(ErrorCode::StoreCorrupt);
                }
                self.private_jobs.as_ref().unwrap().remove(&run, "output")?;
                self.pending.insert(
                    run.clone(),
                    RunSpec {
                        run_id: run.clone(),
                        plugin_id: snapshot.plugin_id,
                        command_id: snapshot.command_id,
                        input,
                        deadline: snapshot.deadline,
                        package_digest: snapshot.package_digest,
                        data: None,
                    },
                );
                let job = self.jobs.get_mut(&run).unwrap();
                job.snapshot.state = JobState::Queued;
                job.snapshot.sequence += 1;
                job.events.push(JobEvent {
                    run_id: run.clone(),
                    sequence: job.snapshot.sequence,
                    state: JobState::Queued,
                });
                self.persist_job(&self.jobs[&run])?;
            } else {
                self.finish_internal(
                    &run,
                    Err(authorization
                        .err()
                        .unwrap_or(ErrorCode::ExecutionInterrupted)),
                    Some(JobState::Interrupted),
                );
                if self.stopping {
                    return Err(ErrorCode::StorageFailed);
                }
            }
        }
        let mut retained: std::collections::HashSet<String> = self.jobs.keys().cloned().collect();
        retained.extend(crate::scheduling::store::ScheduleStore::retained_input_ids(
            &self.data,
        )?);
        retained
            .extend(crate::file_workflows::store::FilePlanStore::retained_input_ids(&self.data)?);
        self.private_jobs
            .as_ref()
            .unwrap()
            .remove_orphans(&retained)?;
        Ok(())
    }
    pub fn pending_runs(&self) -> Vec<String> {
        self.pending.keys().cloned().collect()
    }
    fn job_snapshot(&self, run: &str) -> Result<JobSnapshot, ErrorCode> {
        let job = self.jobs.get(run).ok_or(ErrorCode::JobNotFound)?;
        let mut snapshot = job.snapshot.clone();
        if let Some(ref payloads) = self.private_jobs {
            if let Some(ref mut result) = snapshot.result {
                if let ExecutionOutcome::Success { data, .. } = &mut result.outcome {
                    if now() >= job.accepted_at + 86_400_000.0
                        || self
                            .data
                            .connection
                            .query_row(
                                "SELECT count(*) FROM core_metadata WHERE key=?1",
                                [format!("recovery-output:{}", run)],
                                |row| row.get::<_, u32>(0),
                            )
                            .map_err(|_| ErrorCode::StorageFailed)?
                            != 0
                    {
                        return Err(ErrorCode::ResultExpired);
                    }
                    *data = payloads.read(run, "output")?;
                    if !self
                        .catalog
                        .output_valid(&snapshot.plugin_id, &snapshot.command_id, data)
                    {
                        return Err(ErrorCode::StoreCorrupt);
                    }
                }
            }
        }
        Ok(snapshot)
    }

    pub fn stopping(&self) -> bool {
        self.stopping
    }

    fn managed_authorization(
        &self,
        identity: &CommandIdentity,
        background: bool,
    ) -> Result<(), ErrorCode> {
        if let Some(ref policy) = self.policy {
            self.broker.require_command_grant(identity, now())?;
            let grant = policy
                .records
                .iter()
                .find(|r| r.identity == *identity)
                .and_then(|r| r.grant.as_ref())
                .ok_or(ErrorCode::ApprovalRequired)?;
            if (background && !grant.background) || (self.cold_started && !grant.cold_start) {
                return Err(ErrorCode::ApprovalRequired);
            }
        }
        Ok(())
    }

    /// Explicit disposable-profile fixture only. Not a production user grant.
    pub fn validation_data(cli_token: String, desktop_token: String, store: DataStore) -> Self {
        let mut core = Self::validation(cli_token, desktop_token).with_data_store(store);
        let (manifest, command) = core.catalog.command("plugin-todo-list", "run").unwrap();
        for caller in ["validation-cli", "validation-desktop"] {
            core.broker
                .approve(
                    CommandIdentity::from_manifest(caller, manifest, command),
                    Grant {
                        effects: vec!["data-read".into(), "data-write".into()],
                        scopes: vec![Scope::PluginData {
                            key_prefix: "todos".into(),
                        }],
                        expires_at: now() + 3_600_000.0,
                        max_calls: 16,
                    },
                )
                .expect("Fixed validation policy");
        }
        core
    }

    pub fn handle(&mut self, connection: &str, request: Request) -> Response {
        let request_id = request.request_id.clone();
        let outcome = self
            .dispatch(connection, request)
            .unwrap_or_else(|code| Outcome::Error(RuntimeError { code }));
        Response {
            version: PROTOCOL_MAJOR,
            request_id,
            outcome,
        }
    }

    fn dispatch(&mut self, connection: &str, request: Request) -> Result<Outcome, ErrorCode> {
        if request.version != PROTOCOL_MAJOR {
            return Err(ErrorCode::ProtocolMismatch);
        }
        if !valid_key(&request.request_id) {
            return Err(ErrorCode::InvalidRequest);
        }
        if let Call::Open(open) = request.call {
            if request.session.is_some() {
                return Err(ErrorCode::InvalidRequest);
            }
            if open.client_version != CLIENT_VERSION {
                return Err(ErrorCode::ClientIncompatible);
            }
            if open
                .expected_instance_id
                .as_ref()
                .is_some_and(|id| id != &self.instance_id)
            {
                return Err(ErrorCode::InstanceMismatch);
            }
            if self.sessions.len() >= 64
                || self.sessions.values().any(|s| s.connection == connection)
            {
                return Err(ErrorCode::RuntimeBusy);
            }
            let caller = self
                .tokens
                .get(&open.token)
                .ok_or(ErrorCode::SessionInvalid)?
                .clone();
            let proof = SessionProof {
                session_id: Uuid::new_v4().to_string(),
                instance_id: self.instance_id.clone(),
            };
            self.sessions.insert(
                proof.session_id.clone(),
                Session {
                    connection: connection.into(),
                    caller,
                },
            );
            return Ok(Outcome::Session(proof));
        }
        let proof = request.session.ok_or(ErrorCode::SessionInvalid)?;
        if proof.instance_id != self.instance_id {
            return Err(ErrorCode::InstanceMismatch);
        }
        let session = self
            .sessions
            .get(&proof.session_id)
            .ok_or(ErrorCode::SessionInvalid)?;
        if session.connection != connection {
            return Err(ErrorCode::SessionInvalid);
        }
        let caller = session.caller.clone();
        let management = caller == "local-manager" && self.policy.is_some();
        if self.stopping {
            return Err(ErrorCode::RuntimeBusy);
        }
        if matches!(
            &request.call,
            Call::Permissions
                | Call::Grant(_)
                | Call::Revoke(_)
                | Call::Policy(_)
                | Call::PolicyImport(_)
                | Call::Stop
                | Call::ScheduleCreate(_)
                | Call::ScheduleCancel(_)
                | Call::ScheduleReplace(_)
                | Call::ScheduleRecovery
                | Call::ScheduleDiscard(_)
        ) && !management
        {
            return Err(ErrorCode::SessionInvalid);
        }
        if (management || self.management_only)
            && matches!(
                &request.call,
                Call::Submit(_)
                    | Call::DataRead(_)
                    | Call::DataWrite(_)
                    | Call::DataTransaction(_)
                    | Call::DataImport(_)
            )
        {
            return Err(ErrorCode::ApprovalRequired);
        }
        match request.call {
            Call::ScheduleResolve(input) => {
                crate::scheduling::time::resolve_time(input).map(Outcome::ScheduleTime)
            }
            Call::Schedules => {
                if self.policy.is_none() {
                    return Err(ErrorCode::ApprovalRequired);
                }
                Ok(Outcome::Schedules(
                    crate::scheduling::store::ScheduleStore::list(&self.data)?,
                ))
            }
            Call::ScheduleCreate(create) => self
                .create_schedule(create)
                .map(|record| Outcome::Schedule(Box::new(record))),
            Call::ScheduleCancel(key) => self
                .cancel_schedule(key)
                .map(|record| Outcome::Schedule(Box::new(record))),
            Call::ScheduleReplace(replace) => self
                .replace_schedule(replace)
                .map(|record| Outcome::Schedule(Box::new(record))),
            Call::ScheduleRecovery => crate::scheduling::store::ScheduleStore::recovery(&self.data)
                .map(Outcome::ScheduleRecovery),
            Call::ScheduleDiscard(key) => {
                crate::scheduling::store::ScheduleStore::discard_restored(
                    &mut self.data,
                    &key.challenge,
                )
                .map(Outcome::Schedules)
            }
            Call::PolicyImport(import) => {
                let policy = self.policy.as_mut().unwrap();
                policy.import(&mut self.data, import)?;
                for record in &policy.records {
                    self.broker.restore_policy(
                        record.identity.clone(),
                        record.epoch,
                        record.grant.as_ref().map(PolicyStore::capability),
                    )?;
                }
                Ok(Outcome::Permissions(policy.records.clone()))
            }
            Call::Permissions => Ok(Outcome::Permissions(
                self.policy.as_ref().unwrap().records.clone(),
            )),
            Call::Grant(grant) => {
                let record = self.policy.as_mut().unwrap().grant(&mut self.data, grant)?;
                self.broker.restore_policy(
                    record.identity.clone(),
                    record.epoch,
                    record.grant.as_ref().map(PolicyStore::capability),
                )?;
                Ok(Outcome::Permissions(vec![record]))
            }
            Call::Revoke(key) => {
                let policy = self.policy.as_mut().unwrap();
                policy.revoke(&mut self.data, key)?;
                for record in &policy.records {
                    self.broker.restore_policy(
                        record.identity.clone(),
                        record.epoch,
                        record.grant.as_ref().map(PolicyStore::capability),
                    )?;
                }
                Ok(Outcome::Permissions(policy.records.clone()))
            }
            Call::Policy(policy) => {
                self.policy
                    .as_mut()
                    .unwrap()
                    .set(&mut self.data, policy.clone())?;
                Ok(Outcome::Policy(policy))
            }
            Call::Stop => {
                self.stopping = true;
                // Stop admission and effects before the transport waits to deliver
                // its receipt. The later process drain is independently bounded.
                self.shutdown();
                Ok(Outcome::Stopping)
            }
            Call::Status => Ok(Outcome::Status(RuntimeStatus {
                instance_id: self.instance_id.clone(),
                mode: if self.policy.is_some() {
                    if self.management_only {
                        "management"
                    } else {
                        "managed"
                    }
                } else {
                    "validation"
                }
                .into(),
                jobs: self.jobs.len() as u32,
                active_jobs: self
                    .jobs
                    .values()
                    .filter(|job| !job.snapshot.state.terminal())
                    .count() as u32,
            })),
            Call::Plugins => Ok(Outcome::Plugins(
                self.catalog
                    .list()
                    .map(|(id, manifest)| PluginStatus {
                        plugin_id: id.clone(),
                        version: manifest["version"].as_str().unwrap().into(),
                        package_digest: digest(manifest),
                        installed: true,
                        enabled: self
                            .services
                            .enabled(&crate::dependencies::DependencyIdentity {
                                publisher: manifest["publisher"].as_str().unwrap().into(),
                                id: id.clone(),
                            }),
                        running: self.jobs.values().any(|job| {
                            job.snapshot.plugin_id == *id
                                && matches!(
                                    job.snapshot.state,
                                    JobState::Running | JobState::Cancelling
                                )
                        }),
                    })
                    .collect(),
            )),
            Call::DependencyPlan(request) => {
                if request.plugin_ids.is_empty() {
                    return Err(ErrorCode::DependencyInvalid);
                }
                if request.plugin_ids.len() > 64 {
                    return Err(ErrorCode::DependencyBudgetExceeded);
                }
                if request
                    .plugin_ids
                    .iter()
                    .any(|id| !crate::dependencies::valid_dependency_id(id))
                    || request
                        .plugin_ids
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        != request.plugin_ids.len()
                {
                    return Err(ErrorCode::DependencyInvalid);
                }
                let catalog = self.catalog.clone();
                Ok(Outcome::DependencyPlan(
                    self.plan_catalog(&catalog, &request.plugin_ids)?,
                ))
            }
            Call::ProviderUnloadPlan(key) => Ok(Outcome::ProviderChangePlan(
                self.provider_unload_plan(&key.plugin_id)?,
            )),
            Call::ServiceCalls(key) => Ok(Outcome::ServiceCalls(
                self.service_diagnostics(&key.run_id)?,
            )),
            Call::Submit(submit) => self
                .submit(connection, &caller, submit)
                .map(Outcome::Receipt),
            Call::Lookup(key) => {
                if !valid_key(&key.idempotency_key) {
                    return Err(ErrorCode::InvalidRequest);
                }
                let run = self
                    .keys
                    .get(&(caller.clone(), key.idempotency_key))
                    .ok_or_else(|| {
                        let restored = self.data.connection.query_row("SELECT count(*) FROM core_metadata WHERE key='recovery-generation'", [], |row| row.get::<_, u32>(0)).unwrap_or(1) != 0;
                        if restored { ErrorCode::AcceptanceUnknown } else { ErrorCode::JobNotFound }
                    })?;
                Ok(Outcome::Receipt(self.receipt(run)))
            }
            Call::Diagnose(key) => {
                let job = self.jobs.get(&key.run_id).ok_or(ErrorCode::JobNotFound)?;
                let snapshot = &job.snapshot;
                let result = snapshot.result.as_ref();
                let failure_code = result.and_then(|result| match &result.outcome {
                    ExecutionOutcome::Failure { error, .. } => Some(error.code.clone()),
                    _ => None,
                });
                let restored: bool = self
                    .data
                    .connection
                    .query_row(
                        "SELECT count(*) FROM core_metadata WHERE key=?1",
                        [format!("recovery-output:{}", key.run_id)],
                        |row| row.get::<_, u32>(0),
                    )
                    .map_err(|_| ErrorCode::StorageFailed)?
                    != 0;
                let result_expired = snapshot.state == JobState::Succeeded
                    && (restored || now() >= job.accepted_at + 86_400_000.0);
                let effects = self
                    .catalog
                    .command(&snapshot.plugin_id, &snapshot.command_id)
                    .is_ok_and(|(_, command)| {
                        command["effects"]
                            .as_array()
                            .is_some_and(|effects| !effects.is_empty())
                    });
                Ok(Outcome::Diagnostic(RunDiagnostic {
                    format_version: 1,
                    run_id: snapshot.run_id.clone(),
                    parent_run_id: snapshot.parent_run_id.clone(),
                    root_caller: snapshot.root_caller.clone(),
                    plugin_id: snapshot.plugin_id.clone(),
                    command_id: snapshot.command_id.clone(),
                    package_version: snapshot.package_version.clone(),
                    package_digest: snapshot.package_digest.clone(),
                    dependency_lock: snapshot.dependency_lock.clone(),
                    grant_epoch: snapshot.grant_epoch,
                    state: snapshot.state,
                    sequence: snapshot.sequence,
                    accepted_at: job.accepted_at,
                    started_at: (job.started_at > 0.0).then_some(job.started_at),
                    finished_at: result.map(|result| result.finished_at),
                    duration_ms: if job.started_at > 0.0 {
                        result.map(|result| result.duration_ms)
                    } else {
                        None
                    },
                    background: job.background,
                    failure_code: failure_code.clone(),
                    result_expired,
                    requires_review: snapshot.state == JobState::Interrupted
                        || (effects
                            && matches!(snapshot.state, JobState::Failed | JobState::Cancelled))
                        || failure_code == Some(ErrorCode::AcceptanceUnknown),
                }))
            }
            Call::Job(key) => Ok(Outcome::Job(Box::new(self.job_snapshot(&key.run_id)?))),
            Call::Jobs => {
                let mut jobs: Vec<&Job> = self.jobs.values().collect();
                jobs.sort_by(|a, b| {
                    a.snapshot
                        .state
                        .terminal()
                        .cmp(&b.snapshot.state.terminal())
                        .then_with(|| b.accepted_at.total_cmp(&a.accepted_at))
                        .then_with(|| a.snapshot.run_id.cmp(&b.snapshot.run_id))
                });
                Ok(Outcome::Jobs(
                    jobs.into_iter()
                        .take(128)
                        .map(|job| {
                            let mut snapshot = job.snapshot.clone();
                            // Listings are bounded metadata, never a private output query.
                            snapshot.result = None;
                            snapshot
                        })
                        .collect(),
                ))
            }
            Call::Cancel(key) => {
                let job = self.jobs.get(&key.run_id).ok_or(ErrorCode::JobNotFound)?;
                let shared_user = self.policy.is_some()
                    && matches!(caller.as_str(), "local-cli" | "local-desktop")
                    && matches!(
                        job.snapshot.root_caller.as_str(),
                        "local-cli" | "local-desktop"
                    );
                if job.snapshot.root_caller != caller && !shared_user {
                    return Err(ErrorCode::SessionInvalid);
                }
                self.cancel(&key.run_id);
                Ok(Outcome::Job(Box::new(self.job_snapshot(&key.run_id)?)))
            }
            Call::Events(cursor) => {
                let job = self
                    .jobs
                    .get(&cursor.run_id)
                    .ok_or(ErrorCode::JobNotFound)?;
                Ok(Outcome::Events(
                    job.events
                        .iter()
                        .filter(|event| event.sequence > cursor.after_sequence)
                        .cloned()
                        .collect(),
                ))
            }
            Call::DataRead(read) => {
                self.data_call(&caller, &read.plugin_id, |broker, session, store| {
                    broker
                        .invoke(
                            session,
                            &CapabilityOperation::DataRead {
                                key: read.key.clone(),
                            },
                            now(),
                            |identity| store.read(&namespace(identity), &read.key),
                        )
                        .map(Outcome::Data)
                })
            }
            Call::DataWrite(write) => {
                self.data_call(&caller, &write.plugin_id, |broker, session, store| {
                    broker
                        .invoke(
                            session,
                            &CapabilityOperation::DataWrite {
                                key: write.mutation.key.clone(),
                            },
                            now(),
                            |identity| store.write(&namespace(identity), &write.mutation),
                        )
                        .map(Outcome::Data)
                })
            }
            Call::DataTransaction(transaction) => {
                self.data_call(&caller, &transaction.plugin_id, |broker, session, store| {
                    let Some((last, prior)) = transaction.mutations.split_last() else {
                        return Err(ErrorCode::InvalidRequest);
                    };
                    // All checks and SQLite commit execute under the same Runtime lock.
                    for mutation in prior {
                        broker.invoke(
                            session,
                            &CapabilityOperation::DataWrite {
                                key: mutation.key.clone(),
                            },
                            now(),
                            |_| Ok(()),
                        )?;
                    }
                    broker
                        .invoke(
                            session,
                            &CapabilityOperation::DataWrite {
                                key: last.key.clone(),
                            },
                            now(),
                            |identity| {
                                store.transaction(&namespace(identity), &transaction.mutations)
                            },
                        )
                        .map(Outcome::DataBatch)
                })
            }
            Call::DataImport(import) => {
                self.data_call(&caller, &import.plugin_id, |broker, session, store| {
                    if import.plugin_id != "plugin-todo-list" {
                        return Err(ErrorCode::PluginNotFound);
                    }
                    broker
                        .invoke(
                            session,
                            &CapabilityOperation::DataWrite {
                                key: "todos".into(),
                            },
                            now(),
                            |identity| store.import_todos(&namespace(identity), &import.import),
                        )
                        .map(Outcome::Data)
                })
            }
            Call::Open(_) => unreachable!(),
        }
    }

    fn data_call(
        &mut self,
        caller: &str,
        plugin_id: &str,
        action: impl FnOnce(
            &mut CapabilityBroker,
            &BrokerSession,
            &mut DataStore,
        ) -> Result<Outcome, ErrorCode>,
    ) -> Result<Outcome, ErrorCode> {
        let (manifest, command) = self.catalog.command(plugin_id, "run")?;
        let identity = CommandIdentity::from_manifest(caller, manifest, command);
        self.managed_authorization(&identity, false)?;
        let at = now();
        let session = self.broker.bind(
            identity,
            command.clone(),
            at + command["resources"]["timeoutMs"].as_f64().unwrap_or(0.0),
            at,
        )?;
        let result = action(&mut self.broker, &session, &mut self.data);
        self.broker.close(&session);
        result
    }

    fn submit(
        &mut self,
        connection: &str,
        caller: &str,
        submit: SubmitJob,
    ) -> Result<JobReceipt, ErrorCode> {
        if !valid_key(&submit.idempotency_key) || !submit.deadline.is_finite() {
            return Err(ErrorCode::InvalidRequest);
        }
        let catalog = self.catalog.clone();
        let (manifest, command) = catalog.command(&submit.plugin_id, &submit.command_id)?;
        let identity = CommandIdentity::from_manifest(caller, manifest, command);
        self.managed_authorization(&identity, submit.background)?;
        let grant_epoch = self.broker.authorize_command(&identity, command, now())?;
        let input = self.catalog.prepare_input_mode(
            &submit.plugin_id,
            &submit.command_id,
            submit.input,
            self.policy.is_some(),
        )?;
        let (manifest, command) = catalog.command(&submit.plugin_id, &submit.command_id)?;
        let accepted_at = now();
        let package_digest = digest(manifest);
        let key = (caller.to_owned(), submit.idempotency_key.clone());
        // Previously accepted keys keep their immutable lock across updates/recovery.
        let existing = self
            .keys
            .get(&key)
            .map(|run| self.jobs[run].snapshot.dependency_lock.clone());
        if existing.is_none() {
            if submit.deadline <= accepted_at {
                return Err(ErrorCode::Timeout);
            }
            if submit.deadline
                > accepted_at + command["resources"]["timeoutMs"].as_f64().unwrap_or(0.0)
            {
                return Err(ErrorCode::InvalidRequest);
            }
            if self.jobs.len() >= 1024
                || self
                    .jobs
                    .values()
                    .filter(|job| !job.snapshot.state.terminal())
                    .count()
                    >= 128
            {
                return Err(ErrorCode::RuntimeBusy);
            }
        }
        let planned = if existing.is_none() {
            Some(self.plan_job_dependencies(&submit.plugin_id)?)
        } else {
            None
        };
        let dependency_lock = planned.as_ref().map(|planned| planned.lock.clone());
        let lock_digest =
            existing.unwrap_or_else(|| dependency_lock.as_ref().unwrap().digest.clone());
        let action_digest = digest(
            &json!({ "package":package_digest, "command":submit.command_id, "lock":lock_digest, "input":input, "background":submit.background, "deadline":submit.deadline }),
        );
        if let Some(run_id) = self.keys.get(&key) {
            let job = &self.jobs[run_id];
            if job.action_digest != action_digest {
                return Err(ErrorCode::IdempotencyConflict);
            }
            return Ok(self.receipt(run_id));
        }
        // Dependency IO may outlive an earlier approval or deadline. It does
        // not extend the requested command budget or reuse a cached epoch.
        let final_at = now();
        if submit.deadline <= final_at {
            return Err(ErrorCode::Timeout);
        }
        self.broker
            .reauthorize_accepted(&identity, command, grant_epoch, final_at)?;
        self.managed_authorization(&identity, submit.background)?;
        let accepted_at = final_at;
        let run_id = Uuid::new_v4().to_string();
        let snapshot = JobSnapshot {
            format_version: 1,
            run_id: run_id.clone(),
            parent_run_id: None,
            root_caller: caller.into(),
            plugin_id: submit.plugin_id.clone(),
            command_id: submit.command_id.clone(),
            package_version: manifest["version"].as_str().unwrap().into(),
            package_digest: package_digest.clone(),
            dependency_lock: lock_digest,
            deadline: submit.deadline,
            grant_epoch,
            resources: command["resources"].clone(),
            state: JobState::Accepted,
            sequence: 0,
            result: None,
        };
        let input_summary = InputSummary {
            kind: "object".into(),
            size: input.as_object().map_or(0, |v| v.len() as u32),
        };
        #[cfg(windows)]
        let (tool_lease, artifact_stamp) = match planned.and_then(|planned| planned.artifacts) {
            Some(artifacts) => {
                let stamp = artifacts.metadata_stamp().to_owned();
                (Some(artifacts.bind(&snapshot)?), Some(stamp))
            }
            None => (None, None),
        };
        #[cfg(not(windows))]
        let artifact_stamp: Option<String> = None;
        let job = Job {
            snapshot,
            owner_connection: connection.into(),
            background: submit.background,
            accepted_at,
            started_at: accepted_at,
            input_summary,
            action_digest,
            events: Vec::new(),
            broker_session: None,
            idempotency_key: submit.idempotency_key,
            dependency_lock,
            service_calls: Vec::new(),
            #[cfg(windows)]
            tool_lease,
        };
        if retains_tool_artifacts(&job)
            && serde_json::to_vec(&StoredJob::from_job(&job))
                .map_err(|_| ErrorCode::StorageFailed)?
                .len()
                > 32_768
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        self.services.accept(
            &run_id,
            job.dependency_lock.as_ref().unwrap().clone(),
            command["resources"]["maxOutputBytes"].as_u64().unwrap_or(0) as usize,
        )?;
        self.jobs.insert(run_id.clone(), job);
        if let Some(ref payloads) = self.private_jobs {
            if let Err(error) = payloads.write(&run_id, "input", &input) {
                self.jobs.remove(&run_id);
                self.services.release_root(&run_id);
                return Err(error);
            }
            if self
                .persist_job_inner(&self.jobs[&run_id], artifact_stamp.as_deref())
                .is_err()
            {
                // Do not assume an IO error proves absence of a durable record.
                self.stopping = true;
                return Err(ErrorCode::AcceptanceUnknown);
            }
        }
        self.pending.insert(
            run_id.clone(),
            RunSpec {
                run_id: run_id.clone(),
                plugin_id: submit.plugin_id,
                command_id: submit.command_id,
                input,
                deadline: submit.deadline,
                package_digest,
                data: None,
            },
        );
        self.keys.insert(key, run_id.clone());
        self.transition(&run_id, JobState::Queued)
            .map_err(|_| ErrorCode::AcceptanceUnknown)?;
        Ok(self.receipt(&run_id))
    }

    fn receipt(&self, run_id: &str) -> JobReceipt {
        JobReceipt {
            format_version: 1,
            receipt_type: "job".into(),
            run_id: run_id.into(),
            instance_id: self.instance_id.clone(),
            accepted_at: self.jobs[run_id].accepted_at,
        }
    }

    fn transition(&mut self, run_id: &str, next: JobState) -> Result<(), ErrorCode> {
        let job = self.jobs.get_mut(run_id).ok_or(ErrorCode::JobNotFound)?;
        let valid = matches!(
            (job.snapshot.state, next),
            (JobState::Accepted, JobState::Queued)
                | (
                    JobState::Queued,
                    JobState::Running | JobState::Cancelling | JobState::Failed
                )
                | (
                    JobState::Running,
                    JobState::Succeeded
                        | JobState::Failed
                        | JobState::Cancelling
                        | JobState::Interrupted
                )
                | (JobState::Cancelling, JobState::Cancelled)
        );
        if !valid {
            return Err(ErrorCode::InvalidRequest);
        }
        let previous = job.snapshot.clone();
        job.snapshot.state = next;
        job.snapshot.sequence += 1;
        job.events.push(JobEvent {
            run_id: run_id.into(),
            sequence: job.snapshot.sequence,
            state: next,
        });
        let stored = StoredJob::from_job(job);
        if self.private_jobs.is_some() {
            if let Err(code) = persist_stored_job(&self.data, &stored, None) {
                job.snapshot = previous;
                job.events.pop();
                self.stopping = true;
                return Err(code);
            }
        }
        Ok(())
    }

    pub fn take_run(&mut self, run_id: &str) -> Option<RunSpec> {
        if self.stopping {
            return None;
        }
        let mut run = self.pending.remove(run_id)?;
        if self.jobs[run_id].snapshot.state != JobState::Queued {
            return None;
        }
        let snapshot = self.jobs[run_id].snapshot.clone();
        #[cfg(windows)]
        if let Some(stored) = self.jobs[run_id]
            .tool_lease
            .as_ref()
            .map(|lease| lease.stored().clone())
        {
            let lock = self.jobs[run_id].dependency_lock.clone();
            let restored = match (self.tools.as_mut(), lock.as_ref()) {
                (Some(tools), Some(lock)) => {
                    tools.restore(&mut self.data, &snapshot, lock, &stored)
                }
                _ => Err(ErrorCode::DependencyArtifactMismatch),
            };
            match restored {
                Ok(lease) => self.jobs.get_mut(run_id)?.tool_lease = Some(lease),
                Err(code) => {
                    self.finish_internal(run_id, Err(code), Some(JobState::Interrupted));
                    return None;
                }
            }
        }
        let (manifest, command) = self
            .catalog
            .command(&snapshot.plugin_id, &snapshot.command_id)
            .ok()?;
        let identity = CommandIdentity::from_manifest(&snapshot.root_caller, manifest, command);
        let dispatched_at = now();
        let session = self
            .broker
            .reauthorize_accepted(&identity, command, snapshot.grant_epoch, dispatched_at)
            .and_then(|()| {
                self.broker
                    .bind(identity, command.clone(), snapshot.deadline, dispatched_at)
            });
        match session {
            Ok(session) => self.jobs.get_mut(run_id)?.broker_session = Some(session),
            Err(code) => {
                self.finish(run_id, Err(code));
                return None;
            }
        }
        self.transition(run_id, JobState::Running).ok()?;
        self.jobs.get_mut(run_id)?.started_at = now();
        if run.plugin_id == "plugin-todo-list" {
            let session = self.jobs[run_id].broker_session.as_ref().unwrap();
            match self.broker.invoke(
                session,
                &CapabilityOperation::DataRead {
                    key: "todos".into(),
                },
                now(),
                |identity| self.data.read(&namespace(identity), "todos"),
            ) {
                Ok(snapshot) => run.data = Some(snapshot),
                Err(code) => {
                    self.finish(run_id, Err(code));
                    return None;
                }
            }
        }
        Some(run)
    }

    pub fn commit_mutations(
        &mut self,
        run_id: &str,
        mutations: &[DataMutation],
    ) -> Result<(), ErrorCode> {
        self.check_run(run_id)?;
        if mutations.len() > 16 {
            return Err(ErrorCode::BudgetExceeded);
        }
        if mutations.is_empty() {
            return Ok(());
        }
        let job = &self.jobs[run_id];
        let session = job.broker_session.as_ref().unwrap();
        for mutation in &mutations[..mutations.len() - 1] {
            self.broker.invoke(
                session,
                &CapabilityOperation::DataWrite {
                    key: mutation.key.clone(),
                },
                now(),
                |_| Ok(()),
            )?;
        }
        self.broker.invoke(
            session,
            &CapabilityOperation::DataWrite {
                key: mutations.last().unwrap().key.clone(),
            },
            now(),
            |identity| {
                self.data
                    .transaction(&namespace(identity), mutations)
                    .map(|_| ())
            },
        )
    }

    pub fn state(&self, run_id: &str) -> Option<JobState> {
        self.jobs.get(run_id).map(|j| j.snapshot.state)
    }

    pub fn check_run(&self, run_id: &str) -> Result<(), ErrorCode> {
        if let Some(call) = self.service_invocations.get(run_id) {
            if matches!(
                self.state(&call.root),
                Some(JobState::Cancelling | JobState::Cancelled)
            ) {
                return Err(ErrorCode::Aborted);
            }
            self.check_run(&call.root)?;
            return self.broker.check_session(&call.session, now());
        }
        let job = self.jobs.get(run_id).ok_or(ErrorCode::JobNotFound)?;
        // A failed cancellation commit must not leave a live effect window.
        if self.stopping {
            return Err(ErrorCode::Aborted);
        }
        if job.snapshot.state == JobState::Cancelling {
            return Err(ErrorCode::Aborted);
        }
        if job.snapshot.state != JobState::Running {
            return Err(ErrorCode::SessionInvalid);
        }
        self.broker.check_session(
            job.broker_session
                .as_ref()
                .ok_or(ErrorCode::SessionInvalid)?,
            now(),
        )
    }

    pub fn prepare_service(
        &mut self,
        parent: &str,
        target: ServiceTarget,
        input: Value,
    ) -> Result<ServiceRunSpec, ErrorCode> {
        self.check_run(parent)?;
        target.validate()?;
        let (root, consumer, parent_session, parent_deadline) =
            if let Some(call) = self.service_invocations.get(parent) {
                (
                    call.root.clone(),
                    crate::dependencies::DependencyIdentity {
                        publisher: self
                            .catalog
                            .service_operation(
                                &call.spec.plugin_id,
                                &call.target.service,
                                &call.target.operation,
                            )?
                            .0["publisher"]
                            .as_str()
                            .unwrap()
                            .into(),
                        id: call.spec.plugin_id.clone(),
                    },
                    call.session.clone(),
                    call.spec.deadline,
                )
            } else {
                let job = &self.jobs[parent];
                let manifest = self
                    .catalog
                    .command(&job.snapshot.plugin_id, &job.snapshot.command_id)?
                    .0;
                (
                    parent.to_owned(),
                    crate::dependencies::DependencyIdentity {
                        publisher: manifest["publisher"].as_str().unwrap().into(),
                        id: job.snapshot.plugin_id.clone(),
                    },
                    job.broker_session.as_ref().unwrap().clone(),
                    job.snapshot.deadline,
                )
            };
        let provider = self.services.provider(&root, &consumer, &target)?;
        let (manifest, operation) =
            self.catalog
                .service_operation(&provider.id, &target.service, &target.operation)?;
        if manifest["publisher"] != provider.publisher
            || manifest["version"] != provider.version
            || digest(manifest) != provider.digest
        {
            return Err(ErrorCode::DependencyArtifactMismatch);
        }
        if operation["headless"] != true || operation["interaction"] != "none" {
            return Err(ErrorCode::InteractionRequired);
        }
        let input = BuiltinCatalog::prepare_operation_input(operation, input)?;
        let mut identity = CommandIdentity::from_manifest(
            &self.jobs[&root].snapshot.root_caller,
            manifest,
            operation,
        );
        identity.command_id = format!("svc:{}:{}", target.service, target.operation);
        self.managed_authorization(&identity, self.jobs[&root].background)?;
        let deadline = parent_deadline
            .min(now() + operation["resources"]["timeoutMs"].as_f64().unwrap_or(0.0));
        let session = self.broker.delegate(
            &parent_session,
            identity,
            operation.clone(),
            deadline,
            now(),
        )?;
        let run_id = Uuid::new_v4().to_string();
        let diagnostic = ServiceCallDiagnostic {
            run_id: run_id.clone(),
            parent_run_id: parent.into(),
            root_run_id: root.clone(),
            root_caller: self.jobs[&root].snapshot.root_caller.clone(),
            consumer,
            provider: provider.clone(),
            service: target.service.clone(),
            operation: target.operation.clone(),
            dependency_lock: self.services.lock(&root)?.digest.clone(),
            deadline,
            state: "queued".into(),
            failure_code: None,
        };
        if let Err(code) = self.services.add_call(diagnostic.clone()) {
            self.broker.close(&session);
            return Err(code);
        }
        self.jobs
            .get_mut(&root)
            .unwrap()
            .service_calls
            .push(diagnostic);
        if let Err(code) = self.persist_job(&self.jobs[&root]) {
            self.broker.close(&session);
            let _ = self.services.finish_call(&run_id, Err(code.clone()));
            self.stopping = true;
            return Err(code);
        }
        let spec = RunSpec {
            run_id: run_id.clone(),
            plugin_id: provider.id,
            command_id: format!("svc:{}:{}", target.service, target.operation),
            input: input.clone(),
            deadline,
            package_digest: provider.digest,
            data: None,
        };
        let result = ServiceRunSpec {
            spec: RunSpec {
                run_id: run_id.clone(),
                plugin_id: spec.plugin_id.clone(),
                command_id: spec.command_id.clone(),
                input,
                deadline,
                package_digest: spec.package_digest.clone(),
                data: None,
            },
            target: target.clone(),
            root_run_id: root.clone(),
            parent_run_id: parent.into(),
        };
        self.service_invocations.insert(
            run_id,
            ServiceInvocation {
                root,
                session,
                spec,
                target,
                failure: None,
            },
        );
        Ok(result)
    }

    pub fn start_service(&mut self, id: &str) -> Result<(), ErrorCode> {
        self.check_run(id)?;
        self.services.start_call(id)?;
        let root = &self.service_invocations[id].root;
        self.jobs.get_mut(root).unwrap().service_calls = self.services.diagnostics(root);
        self.persist_job(&self.jobs[root])
    }

    pub fn runner_output_valid(&self, id: &str, output: &Value) -> bool {
        if self.service_invocations.contains_key(id) {
            self.service_output_valid(id, output)
        } else {
            self.jobs.get(id).is_some_and(|job| {
                self.catalog
                    .output_valid(&job.snapshot.plugin_id, &job.snapshot.command_id, output)
            })
        }
    }

    pub fn service_output_valid(&self, id: &str, output: &Value) -> bool {
        let Some(call) = self.service_invocations.get(id) else {
            return false;
        };
        self.catalog
            .service_operation(
                &call.spec.plugin_id,
                &call.target.service,
                &call.target.operation,
            )
            .is_ok_and(|(_, op)| {
                serde_json::to_vec(output).is_ok_and(|bytes| {
                    bytes.len() as u64 <= op["resources"]["maxOutputBytes"].as_u64().unwrap_or(0)
                }) && jsonschema::is_valid(&op["outputSchema"], output)
            })
    }

    pub fn finish_service(
        &mut self,
        id: &str,
        mut result: Result<Value, ErrorCode>,
    ) -> Result<Value, ErrorCode> {
        if result.is_err() {
            if let Some(code) = self
                .service_invocations
                .get(id)
                .and_then(|call| call.failure.clone())
            {
                result = Err(code);
            }
        }
        if result.is_ok() {
            result = self.check_run(id).and(result);
            if result
                .as_ref()
                .is_ok_and(|output| !self.service_output_valid(id, output))
            {
                result = Err(ErrorCode::OutputInvalid);
            }
        }
        let budget = result
            .as_ref()
            .map(|output| serde_json::to_vec(output).map_or(usize::MAX, |b| b.len()))
            .map_err(Clone::clone);
        let finished = self.services.finish_call(id, budget);
        if let Some(call) = self.service_invocations.remove(id) {
            self.broker.close(&call.session);
            self.jobs.get_mut(&call.root).unwrap().service_calls =
                self.services.diagnostics(&call.root);
            if self.persist_job(&self.jobs[&call.root]).is_err() {
                self.stopping = true;
                return Err(ErrorCode::StorageFailed);
            }
            #[cfg(windows)]
            self.release_tool_handles_if_drained(&call.root)?;
        }
        finished.and(result)
    }

    /// Logical capability descriptors only. IO stays inside a T0 adapter callback.
    pub fn invoke_service<T>(
        &mut self,
        id: &str,
        operation: &CapabilityOperation,
        adapter: impl FnOnce(&CommandIdentity) -> Result<T, ErrorCode>,
    ) -> Result<T, ErrorCode> {
        self.check_run(id)?;
        let call = self
            .service_invocations
            .get(id)
            .ok_or(ErrorCode::SessionInvalid)?;
        let result = self.broker.invoke(&call.session, operation, now(), adapter);
        if let Err(code) = &result {
            self.service_invocations.get_mut(id).unwrap().failure = Some(code.clone());
        }
        result
    }

    pub fn service_diagnostics(&self, root: &str) -> Result<Vec<ServiceCallDiagnostic>, ErrorCode> {
        if !self.jobs.contains_key(root) {
            return Err(ErrorCode::JobNotFound);
        }
        Ok(self.jobs[root].service_calls.clone())
    }

    pub fn active_service_calls(&self) -> usize {
        self.service_invocations.len()
    }

    pub fn validate_runner_spec(&self, spec: &RunSpec) -> Result<(), ErrorCode> {
        self.check_run(&spec.run_id)?;
        if !spec.deadline.is_finite() || spec.deadline <= now() {
            return Err(ErrorCode::Timeout);
        }
        let valid = if let Some(call) = self.service_invocations.get(&spec.run_id) {
            spec.plugin_id == call.spec.plugin_id
                && spec.command_id == call.spec.command_id
                && spec.package_digest == call.spec.package_digest
                && spec.input == call.spec.input
                && spec.data.is_none()
                && spec.deadline <= call.spec.deadline
        } else if let Some(job) = self.jobs.get(&spec.run_id) {
            spec.plugin_id == job.snapshot.plugin_id
                && spec.command_id == job.snapshot.command_id
                && spec.package_digest == job.snapshot.package_digest
                && spec.deadline <= job.snapshot.deadline
                && digest(
                    &json!({"package":spec.package_digest,"command":spec.command_id,"lock":job.snapshot.dependency_lock,"input":spec.input,"background":job.background,"deadline":job.snapshot.deadline}),
                ) == job.action_digest
        } else {
            false
        };
        if valid {
            Ok(())
        } else {
            Err(ErrorCode::ExecutionFailed)
        }
    }

    pub fn provider_unload_plan(
        &mut self,
        id: &str,
    ) -> Result<crate::services::ProviderChangePlan, ErrorCode> {
        self.refresh_service_catalog()?;
        let manifest = self
            .catalog
            .list()
            .find(|(key, _)| key.as_str() == id)
            .ok_or(ErrorCode::DependencyMissing)?
            .1;
        self.services.change_plan(
            &crate::dependencies::PackagePin {
                publisher: manifest["publisher"].as_str().unwrap().into(),
                id: id.into(),
                version: manifest["version"].as_str().unwrap().into(),
                digest: digest(manifest),
            },
            None,
        )
    }

    /// T0 fixed-snapshot evaluation only. Real signed installation remains G5.
    pub fn stage_provider_update(
        &mut self,
        id: &str,
        candidate: BuiltinCatalog,
    ) -> Result<crate::services::ProviderChangePlan, ErrorCode> {
        if self.provider_candidates.len() >= 64 || self.retained_catalogs.len() >= 64 {
            return Err(ErrorCode::BudgetExceeded);
        }
        let mut plan = self.provider_unload_plan(id)?;
        if candidate.list().count() != self.catalog.list().count()
            || self
                .catalog
                .list()
                .any(|(key, m)| key != id && !candidate.list().any(|(k, n)| k == key && n == m))
        {
            return Err(ErrorCode::DependencyInvalid);
        }
        self.plan_catalog(
            &candidate,
            &candidate
                .list()
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>(),
        )?;
        let manifest = candidate
            .list()
            .find(|(key, _)| key.as_str() == id)
            .ok_or(ErrorCode::DependencyMissing)?
            .1;
        let replacement = crate::dependencies::PackagePin {
            publisher: manifest["publisher"].as_str().unwrap().into(),
            id: id.into(),
            version: manifest["version"].as_str().unwrap().into(),
            digest: digest(manifest),
        };
        plan = self
            .services
            .change_plan(&plan.provider, Some(replacement))?;
        self.provider_candidates
            .insert(plan.digest.clone(), candidate);
        Ok(plan)
    }

    pub fn begin_provider_change(
        &mut self,
        plan: crate::services::ProviderChangePlan,
        confirmation: &str,
        cascade: bool,
    ) -> Result<(), ErrorCode> {
        if self.stopping {
            return Err(ErrorCode::RuntimeBusy);
        }
        if plan.replacement.is_some() && !self.provider_candidates.contains_key(&plan.digest) {
            return Err(ErrorCode::ApprovalRequired);
        }
        self.services.begin_change(plan, confirmation, cascade)
    }
    pub fn abort_provider_change(&mut self, ticket: &str) {
        self.services.abort_change(ticket);
        self.provider_candidates.remove(ticket);
    }

    /// T0 backup/transaction/drain boundary. No user-supplied SQL or paths on IPC.
    pub fn commit_provider_change(
        &mut self,
        ticket: &str,
        migrate: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        if self.stopping {
            return Err(ErrorCode::RuntimeBusy);
        }
        if !self.services.drained(ticket)? {
            return Err(ErrorCode::RuntimeBusy);
        }
        let plan = self.services.presented_plan(ticket)?;
        let backup = match self.data.backup() {
            Ok(backup) => backup,
            Err(code) => {
                self.abort_provider_change(ticket);
                return Err(code);
            }
        };
        let backup_id = backup
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("runtime-backup-"))
            .ok_or(ErrorCode::StorageFailed)?
            .to_owned();
        let previous: Option<String> = self
            .data
            .connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='service-catalog-state-v1'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| ErrorCode::StorageFailed)?;
        let pending = self
            .provider_candidates
            .get(ticket)
            .map(BuiltinCatalog::snapshot_digest);
        let mut journal = ProviderJournal {
            format_version: 1,
            phase: "prepared".into(),
            active_catalog: self.catalog.snapshot_digest(),
            pending_catalog: pending,
            backup_id,
            disabled: self.services.disabled(),
        };
        let write_journal = |connection: &rusqlite::Connection,
                             journal: &ProviderJournal|
         -> Result<(), ErrorCode> {
            connection.execute("INSERT INTO core_metadata(key,value) VALUES('service-catalog-state-v1',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(journal).map_err(|_|ErrorCode::StorageFailed)?]).map_err(|_|ErrorCode::StorageFailed)?;
            Ok(())
        };
        let result = (|| {
            write_journal(&self.data.connection, &journal).map_err(|code| (code, false))?;
            let tx = self
                .data
                .connection
                .transaction()
                .map_err(|_| (ErrorCode::StorageFailed, true))?;
            let result = migrate(&tx).and_then(|()| {
                if plan.replacement.is_none() {
                    journal.disabled.extend(plan.consumers.clone());
                    journal.disabled.push(plan.provider.identity());
                    journal.disabled.sort();
                    journal.disabled.dedup();
                }
                if let Some(active) = journal.pending_catalog.take() {
                    journal.active_catalog = active;
                }
                journal.phase = "committed".into();
                write_journal(&tx, &journal)
            });
            match result {
                Ok(()) => tx.commit().map_err(|_| (ErrorCode::StorageFailed, false)),
                Err(code) => match tx.rollback() {
                    Ok(()) => Err((code, true)),
                    Err(_) => Err((ErrorCode::StorageFailed, false)),
                },
            }
        })();
        if let Err((code, rollback_confirmed)) = result {
            // An uncertain commit/rollback cannot reopen old admission.
            if !rollback_confirmed {
                self.stopping = true;
                self.shutdown();
                return Err(code);
            }
            let restored = if let Some(previous) = previous {
                self.data.connection.execute(
                    "UPDATE core_metadata SET value=?1 WHERE key='service-catalog-state-v1'",
                    [previous],
                )
            } else {
                self.data.connection.execute(
                    "DELETE FROM core_metadata WHERE key='service-catalog-state-v1'",
                    [],
                )
            };
            if restored.is_err() {
                self.stopping = true;
                self.shutdown();
                return Err(ErrorCode::StorageFailed);
            }
            self.abort_provider_change(ticket);
            return Err(code);
        }
        if let Err(code) = self.services.commit_change(ticket, || Ok(())) {
            self.stopping = true;
            self.shutdown();
            return Err(code);
        }
        if let Some(candidate) = self.provider_candidates.remove(ticket) {
            self.retained_catalogs.push(self.catalog.clone());
            self.catalog = candidate;
            if let Err(code) = self.refresh_service_catalog() {
                self.stopping = true;
                self.shutdown();
                return Err(code);
            }
            if let Some(policy) = self.policy.as_mut() {
                policy.set_catalog(self.catalog.clone());
            }
        }
        Ok(())
    }

    pub fn finish(&mut self, run_id: &str, result: Result<Value, ErrorCode>) {
        self.finish_internal(run_id, result, None);
    }

    fn finish_internal(
        &mut self,
        run_id: &str,
        mut result: Result<Value, ErrorCode>,
        recovered: Option<JobState>,
    ) {
        if self.state(run_id).is_none_or(JobState::terminal) {
            return;
        }
        let cancelled = self.state(run_id) == Some(JobState::Cancelling);
        if cancelled {
            result = Err(ErrorCode::Aborted);
        }
        if let Ok(ref data) = result {
            if let Err(code) = self.services.charge_output(
                run_id,
                serde_json::to_vec(data).map_or(usize::MAX, |b| b.len()),
            ) {
                result = Err(code);
            }
        }
        if let Ok(ref data) = result {
            let snapshot = &self.jobs[run_id].snapshot;
            if !self
                .catalog
                .output_valid(&snapshot.plugin_id, &snapshot.command_id, data)
            {
                result = Err(ErrorCode::OutputInvalid);
            }
        }
        if let (Some(payloads), Ok(data)) = (&self.private_jobs, &result) {
            if let Err(code) = payloads.write(run_id, "output", data) {
                result = Err(code);
            }
        }
        let state = recovered.unwrap_or(if cancelled {
            JobState::Cancelled
        } else if result.is_ok() {
            JobState::Succeeded
        } else {
            JobState::Failed
        });
        let outcome = match result {
            Ok(data) => ExecutionOutcome::Success {
                success: true,
                data: if self.private_jobs.is_some() {
                    Value::Null
                } else {
                    data
                },
            },
            Err(code) => ExecutionOutcome::Failure {
                success: false,
                error: RuntimeError { code },
            },
        };
        let job = self.jobs.get_mut(run_id).unwrap();
        self.services.release_root(run_id);
        if let Some(session) = job.broker_session.take() {
            self.broker.close(&session);
        }
        let finished_at = now().max(job.started_at);
        job.snapshot.state = state;
        job.snapshot.sequence += 1;
        job.events.push(JobEvent {
            run_id: run_id.into(),
            sequence: job.snapshot.sequence,
            state,
        });
        job.snapshot.result = Some(ExecutionResult {
            format_version: 1,
            run_id: run_id.into(),
            plugin_id: job.snapshot.plugin_id.clone(),
            plugin_version: job.snapshot.package_version.clone(),
            started_at: job.started_at,
            finished_at,
            duration_ms: finished_at - job.started_at,
            input_summary: job.input_summary.clone(),
            outcome,
        });
        // Terminal state and result commit together, never a terminal record without an envelope.
        if self.persist_job(&self.jobs[run_id]).is_err() {
            self.stopping = true;
            let job = self.jobs.get_mut(run_id).unwrap();
            job.snapshot.state = JobState::Interrupted;
            if let Some(ref mut envelope) = job.snapshot.result {
                envelope.outcome = ExecutionOutcome::Failure {
                    success: false,
                    error: RuntimeError {
                        code: ErrorCode::StorageFailed,
                    },
                };
            }
            // Preserve input when the terminal commit is ambiguous. Restart
            // decides from durable metadata, never from this in-memory state.
            return;
        }
        #[cfg(windows)]
        if self.release_tool_handles_if_drained(run_id).is_err() {
            self.stopping = true;
            return;
        }
        // Unknown/interrupted tool-bearing work keeps its private bytes and
        // artifact roots until an explicit recovery/disposal decision.
        if state == JobState::Interrupted && retains_tool_artifacts(&self.jobs[run_id]) {
            return;
        }
        if let Some(ref payloads) = self.private_jobs {
            let cleanup = payloads.remove(run_id, "input").and_then(|_| {
                if state != JobState::Succeeded {
                    payloads.remove(run_id, "output")
                } else {
                    Ok(())
                }
            });
            if cleanup.is_err() {
                self.stopping = true;
            }
        }
    }

    #[cfg(windows)]
    fn release_tool_handles_if_drained(&mut self, run: &str) -> Result<(), ErrorCode> {
        let job = self.jobs.get_mut(run).ok_or(ErrorCode::JobNotFound)?;
        if !matches!(
            job.snapshot.state,
            JobState::Succeeded | JobState::Failed | JobState::Cancelled
        ) || !self.services.root_drained(run)
            || !retains_tool_artifacts(job)
        {
            return Ok(());
        }
        job.tool_lease.as_mut().unwrap().mark_released()?;
        let stored = StoredJob::from_job(job);
        if let Err(code) = persist_stored_job(&self.data, &stored, None) {
            self.jobs
                .get_mut(run)
                .unwrap()
                .tool_lease
                .as_mut()
                .unwrap()
                .retain()?;
            self.stopping = true;
            return Err(code);
        }
        self.jobs
            .get_mut(run)
            .unwrap()
            .tool_lease
            .as_mut()
            .unwrap()
            .drop_released_handles();
        Ok(())
    }
    pub fn cancel(&mut self, run_id: &str) {
        if matches!(
            self.state(run_id),
            Some(JobState::Queued | JobState::Running)
        ) {
            if self.transition(run_id, JobState::Cancelling).is_err() {
                return;
            }
            if self.pending.remove(run_id).is_some() {
                self.finish(run_id, Err(ErrorCode::Aborted));
            }
        }
    }

    pub fn disconnect(&mut self, connection: &str) {
        self.sessions
            .retain(|_, session| session.connection != connection);
        let runs: Vec<String> = self
            .jobs
            .iter()
            .filter(|(_, job)| {
                job.owner_connection == connection
                    && !job.background
                    && !job.snapshot.state.terminal()
            })
            .map(|(id, _)| id.clone())
            .collect();
        for run in runs {
            self.cancel(&run);
        }
    }

    pub fn shutdown(&mut self) {
        // A transient policy Host never owns restored business execution.
        if self.management_only {
            self.sessions.clear();
            return;
        }
        let runs: Vec<String> = self.jobs.keys().cloned().collect();
        for run in runs {
            self.cancel(&run);
        }
        self.sessions.clear();
    }
    /// T0 fatal runner/storage boundary; no wire or plugin-owned stop switch.
    pub fn fail_closed(&mut self) {
        self.stopping = true;
        self.shutdown();
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && key
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || v == b'-' || v == b'_')
}

fn namespace(identity: &CommandIdentity) -> String {
    format!("{}:{}", identity.publisher, identity.plugin_id)
}

/// Offline recovery never replays accepted work; preserve its metadata for review.
pub(crate) fn interrupt_restored_jobs(connection: &rusqlite::Connection) -> Result<(), ErrorCode> {
    let rows = connection
        .prepare("SELECT run_id,CASE WHEN typeof(metadata)='text' AND length(CAST(metadata AS BLOB))<=32768 THEN metadata ELSE NULL END FROM jobs LIMIT 1025")
        .map_err(|_| ErrorCode::StoreCorrupt)?
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|_| ErrorCode::StoreCorrupt)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ErrorCode::StoreCorrupt)?;
    if rows.len() > 1024 {
        return Err(ErrorCode::BudgetExceeded);
    }
    // Preflight the whole bounded batch before any restored-row mutation.
    let rows = rows
        .into_iter()
        .map(|(run, value)| {
            let value = value.ok_or(ErrorCode::StoreCorrupt)?;
            let stored: StoredJob = serde_json::from_value(
                crate::packages::strict_json(value.as_bytes())
                    .map_err(|_| ErrorCode::StoreCorrupt)?,
            )
            .map_err(|_| ErrorCode::StoreCorrupt)?;
            if stored.snapshot.run_id != run
                || stored.format_version != 1
                || stored.snapshot.format_version != 1
                || value.len() > 32768
                || uuid::Uuid::parse_str(&run).is_err()
                || !valid_key(&stored.idempotency_key)
            {
                return Err(ErrorCode::StoreCorrupt);
            }
            #[cfg(windows)]
            stored.validate_tool_artifacts()?;
            Ok((run, stored))
        })
        .collect::<Result<Vec<_>, ErrorCode>>()?;
    for (run, mut stored) in rows {
        if stored.snapshot.state == JobState::Succeeded {
            connection
                .execute(
                    "INSERT OR REPLACE INTO core_metadata(key,value) VALUES(?1,'true')",
                    [format!("recovery-output:{run}")],
                )
                .map_err(|_| ErrorCode::StorageFailed)?;
        }
        if !stored.snapshot.state.terminal() {
            stored.snapshot.state = JobState::Interrupted;
            stored.snapshot.sequence = stored
                .snapshot
                .sequence
                .checked_add(1)
                .ok_or(ErrorCode::BudgetExceeded)?;
            stored.events.push(JobEvent {
                run_id: run.clone(),
                sequence: stored.snapshot.sequence,
                state: JobState::Interrupted,
            });
            let finished = now();
            stored.snapshot.result = Some(ExecutionResult {
                format_version: 1,
                run_id: run.clone(),
                plugin_id: stored.snapshot.plugin_id.clone(),
                plugin_version: stored.snapshot.package_version.clone(),
                started_at: stored.started_at.max(stored.accepted_at),
                finished_at: finished,
                duration_ms: finished - stored.started_at.max(stored.accepted_at),
                input_summary: stored.input_summary.clone(),
                outcome: ExecutionOutcome::Failure {
                    success: false,
                    error: RuntimeError {
                        code: ErrorCode::ExecutionInterrupted,
                    },
                },
            });
            #[cfg(windows)]
            if stored.tool_artifacts.is_some() {
                leases::persist_restored_job(
                    connection,
                    &run,
                    &serde_json::to_value(&stored).map_err(|_| ErrorCode::StorageFailed)?,
                )?;
                continue;
            }
            connection
                .execute(
                    "UPDATE jobs SET metadata=?1 WHERE run_id=?2",
                    rusqlite::params![
                        serde_json::to_string(&stored).map_err(|_| ErrorCode::StorageFailed)?,
                        run
                    ],
                )
                .map_err(|_| ErrorCode::StorageFailed)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "runtime_service_tests.rs"]
mod service_recovery_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dependency_plans_bind_host_inventory_and_do_not_submit_or_grant() {
        let mut core = RuntimeCore::validation("cli-token".into(), "desktop-token".into());
        let session = open(&mut core, "cli", "cli-token");
        let before_jobs = core.jobs.len();
        let before_pending = core.pending.len();
        let call = || {
            Call::DependencyPlan(DependencyPlanRequest {
                plugin_ids: vec!["plugin-base64-encoder".into()],
            })
        };
        let Outcome::DependencyPlan(first) = core
            .handle("cli", request(call(), Some(session.clone())))
            .outcome
        else {
            panic!("Expected a Host-generated plan");
        };
        let Outcome::DependencyPlan(second) = core
            .handle("cli", request(call(), Some(session.clone())))
            .outcome
        else {
            panic!("Expected a deterministic plan");
        };
        assert_eq!(
            serde_json::to_value(first).unwrap(),
            serde_json::to_value(second).unwrap()
        );
        assert_eq!(core.jobs.len(), before_jobs);
        assert_eq!(core.pending.len(), before_pending);
        for ids in [
            vec![],
            vec!["other-publisher/service".into()],
            vec!["missing-plugin".into()],
            vec!["plugin-base64-encoder".into(); 2],
            vec!["plugin-base64-encoder".into(); 65],
        ] {
            assert!(matches!(
                core.handle(
                    "cli",
                    request(
                        Call::DependencyPlan(DependencyPlanRequest { plugin_ids: ids }),
                        Some(session.clone())
                    )
                )
                .outcome,
                Outcome::Error(RuntimeError {
                    code: ErrorCode::DependencyMissing
                        | ErrorCode::DependencyInvalid
                        | ErrorCode::DependencyBudgetExceeded
                })
            ));
        }
        // No caller can provide the catalog, publisher, platform or artifact path.
        for extra in ["catalog", "publisher", "target", "path"] {
            let mut payload = json!({"pluginIds":["plugin-base64-encoder"]});
            payload[extra] = json!("untrusted");
            assert!(serde_json::from_value::<Call>(
                json!({"method":"dependencies.plan","payload":payload})
            )
            .is_err());
        }
        assert!(matches!(
            core.handle("other-connection", request(call(), Some(session)))
                .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::SessionInvalid
            })
        ));
    }

    #[test]
    fn authorized_stop_cancels_queued_and_running_work_before_receipt_delivery() {
        for persist_failure in [false, true] {
            let profile = std::env::temp_dir()
                .join(format!("flowtools-validation-stop-data-{}", Uuid::new_v4()));
            std::fs::create_dir(&profile).unwrap();
            let mut core = RuntimeCore::managed(
                HashMap::from([
                    ("cli".into(), "local-cli".into()),
                    ("admin".into(), "local-manager".into()),
                ]),
                DataStore::open(&profile.join("runtime.sqlite")).unwrap(),
                false,
                false,
            )
            .unwrap();
            let admin = open(&mut core, "admin", "admin");
            let (manifest, _) = core.catalog.command("plugin-todo-list", "run").unwrap();
            let grant = PermissionGrant {
                plugin_id: "plugin-todo-list".into(),
                command_id: "run".into(),
                target: GrantTarget::Cli,
                package_digest: digest(manifest),
                effects: vec!["data-read".into(), "data-write".into()],
                scopes: vec![Scope::PluginData {
                    key_prefix: "todos".into(),
                }],
                expires_at: now() + 60_000.0,
                max_calls: 16,
                cold_start: false,
                background: true,
            };
            assert!(matches!(
                core.handle("admin", request(Call::Grant(grant), Some(admin.clone())))
                    .outcome,
                Outcome::Permissions(_)
            ));
            let cli = open(&mut core, "cli", "cli");
            let mut runs = Vec::new();
            for key in ["running", "queued"] {
                let Outcome::Receipt(receipt) = core
                    .handle(
                        "cli",
                        request(
                            Call::Submit(SubmitJob {
                                plugin_id: "plugin-todo-list".into(),
                                command_id: "run".into(),
                                input: json!({"todo":"stop-fixture"}),
                                idempotency_key: key.into(),
                                background: true,
                                deadline: now() + 20_000.0,
                            }),
                            Some(cli.clone()),
                        ),
                    )
                    .outcome
                else {
                    panic!("Receipt");
                };
                runs.push(receipt.run_id);
            }
            core.take_run(&runs[0]).unwrap();
            if persist_failure {
                core.data.connection.execute_batch(
                "CREATE TRIGGER stop_storage_failure BEFORE UPDATE ON jobs BEGIN SELECT RAISE(FAIL,'fixture'); END;"
            ).unwrap();
            }
            assert!(matches!(
                core.handle("cli", request(Call::Stop, Some(cli))).outcome,
                Outcome::Error(RuntimeError {
                    code: ErrorCode::SessionInvalid
                })
            ));
            assert!(!core.stopping());
            assert_eq!(core.check_run(&runs[0]), Ok(()));
            assert!(matches!(
                core.handle("admin", request(Call::Stop, Some(admin)))
                    .outcome,
                Outcome::Stopping
            ));
            assert!(core.stopping());
            assert_eq!(
                core.state(&runs[1]),
                Some(if persist_failure {
                    JobState::Queued
                } else {
                    JobState::Cancelled
                })
            );
            assert!(core.take_run(&runs[1]).is_none());
            assert_eq!(core.check_run(&runs[0]), Err(ErrorCode::Aborted));
            assert_eq!(
                core.commit_mutations(
                    &runs[0],
                    &[DataMutation {
                        key: "todos".into(),
                        expected_revision: 0,
                        value: json!([{"todo":"must-not-commit","deadline":""}]),
                    }],
                ),
                Err(ErrorCode::Aborted)
            );
            assert_eq!(
                core.data
                    .read("flowtools:plugin-todo-list", "todos")
                    .unwrap()
                    .revision,
                0
            );
            core.shutdown();
            assert_eq!(
                core.state(&runs[0]),
                Some(if persist_failure {
                    JobState::Running
                } else {
                    JobState::Cancelling
                })
            );
        }
    }

    #[test]
    fn managed_user_clients_share_metadata_and_cancel_but_not_management_roles() {
        let mut core = RuntimeCore::managed(
            HashMap::from([
                ("cli".into(), "local-cli".into()),
                ("desktop".into(), "local-desktop".into()),
                ("admin".into(), "local-manager".into()),
            ]),
            DataStore::memory().unwrap(),
            false,
            false,
        )
        .unwrap();
        let admin = open(&mut core, "admin", "admin");
        let (manifest, _) = core
            .catalog
            .command("plugin-base64-encoder", "run")
            .unwrap();
        let grant = PermissionGrant {
            plugin_id: "plugin-base64-encoder".into(),
            command_id: "run".into(),
            target: GrantTarget::Cli,
            package_digest: digest(manifest),
            effects: vec![],
            scopes: vec![],
            expires_at: now() + 60_000.0,
            max_calls: 16,
            cold_start: false,
            background: true,
        };
        assert!(matches!(
            core.handle("admin", request(Call::Grant(grant), Some(admin.clone())))
                .outcome,
            Outcome::Permissions(_)
        ));
        let cli = open(&mut core, "cli", "cli");
        let desktop = open(&mut core, "desktop", "desktop");
        let mut operation = submit();
        operation.input = json!({"text":"SHARED_PRIVATE_OUTPUT_CANARY"});
        operation.background = true;
        let Outcome::Receipt(receipt) = core
            .handle("cli", request(Call::Submit(operation), Some(cli.clone())))
            .outcome
        else {
            panic!("Receipt")
        };
        let listed = core
            .handle("desktop", request(Call::Jobs, Some(desktop.clone())))
            .outcome;
        assert!(
            matches!(&listed, Outcome::Jobs(jobs) if jobs.len() == 1 && jobs[0].run_id == receipt.run_id && jobs[0].result.is_none())
        );
        assert!(!serde_json::to_string(&listed)
            .unwrap()
            .contains("SHARED_PRIVATE_OUTPUT_CANARY"));
        assert!(
            matches!(core.handle("desktop", request(Call::Status, Some(desktop.clone()))).outcome, Outcome::Status(status) if status.active_jobs == 1)
        );
        assert!(matches!(
            core.handle(
                "admin",
                request(
                    Call::Cancel(JobKey {
                        run_id: receipt.run_id.clone()
                    }),
                    Some(admin)
                )
            )
            .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::SessionInvalid
            })
        ));
        assert!(
            matches!(core.handle("desktop", request(Call::Cancel(JobKey { run_id: receipt.run_id }), Some(desktop))).outcome, Outcome::Job(snapshot) if snapshot.state == JobState::Cancelled)
        );
    }
    #[test]
    fn durable_receipt_recovery_keeps_keys_private_and_never_replays_running_writes() {
        let profile =
            std::env::temp_dir().join(format!("flowtools-validation-durable-{}", Uuid::new_v4()));
        std::fs::create_dir(&profile).unwrap();
        let database = profile.join("runtime.sqlite");
        let reopen = || {
            RuntimeCore::managed(
                HashMap::from([
                    ("cli".into(), "local-cli".into()),
                    ("admin".into(), "local-manager".into()),
                ]),
                DataStore::open(&database).unwrap(),
                false,
                false,
            )
            .unwrap()
        };
        let mut core = reopen();
        let admin = open(&mut core, "admin", "admin");
        let (manifest, _) = core.catalog.command("plugin-todo-list", "run").unwrap();
        let grant = PermissionGrant {
            plugin_id: "plugin-todo-list".into(),
            command_id: "run".into(),
            target: GrantTarget::Cli,
            package_digest: digest(manifest),
            effects: vec!["data-read".into(), "data-write".into()],
            scopes: vec![Scope::PluginData {
                key_prefix: "todos".into(),
            }],
            expires_at: now() + 60_000.0,
            max_calls: 16,
            cold_start: false,
            background: true,
        };
        assert!(matches!(
            core.handle("admin", request(Call::Grant(grant), Some(admin)))
                .outcome,
            Outcome::Permissions(_)
        ));
        let cli = open(&mut core, "cli", "cli");
        let operation = SubmitJob {
            plugin_id: "plugin-todo-list".into(),
            command_id: "run".into(),
            input: json!({"todo":"PRIVATE_PAYLOAD_CANARY"}),
            idempotency_key: "lost-ack".into(),
            background: true,
            deadline: now() + 20_000.0,
        };
        let Outcome::Receipt(receipt) = core
            .handle("cli", request(Call::Submit(operation.clone()), Some(cli)))
            .outcome
        else {
            panic!("Receipt")
        };
        let queued_backup = core.data.backup().unwrap();
        let accepted = core.data.load_jobs().unwrap();
        assert_eq!(accepted.len(), 1);
        assert!(!serde_json::to_string(&accepted)
            .unwrap()
            .contains("PRIVATE_PAYLOAD_CANARY"));
        drop(core);
        let mut management = RuntimeCore::managed(
            HashMap::from([("admin".into(), "local-manager".into())]),
            DataStore::open(&database).unwrap(),
            true,
            false,
        )
        .unwrap();
        management.shutdown();
        assert_eq!(management.state(&receipt.run_id), Some(JobState::Queued));
        assert!(management.pending_runs().is_empty());
        drop(management);
        let mut core = reopen();
        let cli = open(&mut core, "cli", "cli");
        let Outcome::Receipt(retry) = core
            .handle(
                "cli",
                request(
                    Call::Lookup(IdempotencyKey {
                        idempotency_key: "lost-ack".into(),
                    }),
                    Some(cli.clone()),
                ),
            )
            .outcome
        else {
            panic!("Lookup")
        };
        assert_eq!(retry.run_id, receipt.run_id);
        let mut changed = operation;
        changed.input = json!({"todo":"different"});
        assert!(matches!(
            core.handle("cli", request(Call::Submit(changed), Some(cli)))
                .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::IdempotencyConflict
            })
        ));
        assert_eq!(
            core.take_run(&receipt.run_id).unwrap().input["todo"],
            "PRIVATE_PAYLOAD_CANARY"
        );
        core.commit_mutations(
            &receipt.run_id,
            &[DataMutation {
                key: "todos".into(),
                expected_revision: 0,
                value: json!([{"todo":"PRIVATE_PAYLOAD_CANARY","deadline":""}]),
            }],
        )
        .unwrap();
        drop(core); // Crash after a write but before completion/ACK.
        let core = reopen();
        assert_eq!(core.state(&receipt.run_id), Some(JobState::Interrupted));
        assert!(core.pending_runs().is_empty());
        let snapshot = core.job_snapshot(&receipt.run_id).unwrap();
        assert!(matches!(
            snapshot.result.unwrap().outcome,
            ExecutionOutcome::Failure {
                error: RuntimeError {
                    code: ErrorCode::ExecutionInterrupted
                },
                ..
            }
        ));
        assert_eq!(
            core.data
                .read("flowtools:plugin-todo-list", "todos")
                .unwrap()
                .revision,
            1
        );
        assert_eq!(
            core.jobs[&receipt.run_id].events.last().unwrap().state,
            JobState::Interrupted
        );
        drop(core);
        let backup_id = queued_backup
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("runtime-backup-")
            .unwrap()
            .strip_suffix(".sqlite")
            .unwrap()
            .to_string();
        crate::recovery::manage(&profile, StorageAction::Restore { backup_id }).unwrap();
        let mut restored = reopen();
        assert_eq!(restored.state(&receipt.run_id), Some(JobState::Interrupted));
        assert!(restored.pending_runs().is_empty());
        assert!(restored
            .policy
            .as_ref()
            .unwrap()
            .records
            .iter()
            .all(|record| record.grant.is_none()));
        let cli = open(&mut restored, "cli", "cli");
        assert!(
            matches!(restored.handle("cli",request(Call::Diagnose(JobKey{run_id:receipt.run_id}),Some(cli))).outcome,Outcome::Diagnostic(report) if report.requires_review && report.failure_code == Some(ErrorCode::ExecutionInterrupted))
        );
    }
    #[test]
    fn management_role_is_host_bound_and_cannot_execute_business() {
        let mut core = RuntimeCore::managed(
            HashMap::from([
                ("admin".into(), "local-manager".into()),
                ("cli".into(), "local-cli".into()),
            ]),
            DataStore::memory().unwrap(),
            true,
            false,
        )
        .unwrap();
        let admin = open(&mut core, "admin-connection", "admin");
        let cli = open(&mut core, "cli-connection", "cli");
        let import = Call::PolicyImport(PolicyImport {
            format_version: 1,
            cold_start: true,
            grants: vec![],
        });
        assert!(matches!(
            core.handle("cli-connection", request(import.clone(), Some(cli)))
                .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::SessionInvalid
            })
        ));
        assert!(matches!(
            core.handle("admin-connection", request(import, Some(admin.clone())))
                .outcome,
            Outcome::Permissions(_)
        ));
        assert!(matches!(
            core.handle(
                "admin-connection",
                request(
                    Call::Submit(SubmitJob {
                        plugin_id: "plugin-base64-encoder".into(),
                        command_id: "run".into(),
                        input: json!({"text":"fixture"}),
                        idempotency_key: "fixture".into(),
                        background: false,
                        deadline: now() + 1000.0
                    }),
                    Some(admin)
                )
            )
            .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::ApprovalRequired
            })
        ));
        assert!(core.jobs.is_empty());
    }
    fn request(call: Call, session: Option<SessionProof>) -> Request {
        Request {
            version: 1,
            request_id: Uuid::new_v4().to_string(),
            session,
            call,
        }
    }
    fn open(core: &mut RuntimeCore, connection: &str, token: &str) -> SessionProof {
        let Outcome::Session(proof) = core
            .handle(
                connection,
                request(
                    Call::Open(OpenSession {
                        token: token.into(),
                        client_version: CLIENT_VERSION.into(),
                        expected_instance_id: None,
                    }),
                    None,
                ),
            )
            .outcome
        else {
            panic!("Session")
        };
        proof
    }
    fn submit() -> SubmitJob {
        SubmitJob {
            plugin_id: "plugin-base64-encoder".into(),
            command_id: "run".into(),
            input: json!({"text":"hello"}),
            idempotency_key: "fixture-key".into(),
            background: false,
            deadline: now() + 10000.0,
        }
    }

    #[test]
    fn sessions_bind_connections_and_caller_is_never_payload_identity() {
        let mut core = RuntimeCore::validation("cli".into(), "desktop".into());
        let proof = open(&mut core, "conn", "cli");
        assert!(matches!(
            core.handle("spoof", request(Call::Status, Some(proof.clone())))
                .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::SessionInvalid
            })
        ));
        core.disconnect("conn");
        assert!(matches!(
            core.handle("conn", request(Call::Status, Some(proof)))
                .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::SessionInvalid
            })
        ));
    }

    #[test]
    fn receipts_are_distinct_shared_task_queries_and_retries_do_not_duplicate() {
        let mut core = RuntimeCore::validation("cli".into(), "desktop".into());
        let cli = open(&mut core, "cli-conn", "cli");
        let desktop = open(&mut core, "desktop-conn", "desktop");
        let original = submit();
        let Outcome::Receipt(receipt) = core
            .handle(
                "cli-conn",
                request(Call::Submit(original.clone()), Some(cli.clone())),
            )
            .outcome
        else {
            panic!("Receipt")
        };
        assert!(
            matches!(core.handle("desktop-conn", request(Call::Job(JobKey { run_id:receipt.run_id.clone() }), Some(desktop))).outcome, Outcome::Job(job) if job.root_caller == "validation-cli")
        );
        let Outcome::Receipt(repeated) = core
            .handle(
                "cli-conn",
                request(Call::Submit(original.clone()), Some(cli.clone())),
            )
            .outcome
        else {
            panic!("Receipt retry")
        };
        assert_eq!(repeated.run_id, receipt.run_id);
        let mut changed = original.clone();
        changed.input = json!({"text":"other"});
        assert!(matches!(
            core.handle("cli-conn", request(Call::Submit(changed), Some(cli)))
                .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::IdempotencyConflict
            })
        ));
        core.take_run(&receipt.run_id).unwrap();
        core.finish(&receipt.run_id, Ok(json!({"type":"json","value":{"result":"aGVsbG8=","mode":"encode","input":"hello"}})));
        assert_eq!(core.state(&receipt.run_id), Some(JobState::Succeeded));
        core.cancel(&receipt.run_id);
        assert_eq!(core.state(&receipt.run_id), Some(JobState::Succeeded));
    }

    #[test]
    fn disconnect_cancels_temporary_tasks_but_explicit_background_survives() {
        for background in [false, true] {
            let mut core = RuntimeCore::validation("cli".into(), "desktop".into());
            let proof = open(&mut core, "conn", "cli");
            let mut submit = submit();
            submit.background = background;
            let Outcome::Receipt(receipt) = core
                .handle("conn", request(Call::Submit(submit), Some(proof)))
                .outcome
            else {
                panic!("Receipt")
            };
            core.disconnect("conn");
            assert_eq!(
                core.state(&receipt.run_id),
                Some(if background {
                    JobState::Queued
                } else {
                    JobState::Cancelled
                })
            );
        }
    }

    #[test]
    fn malformed_wire_identity_and_versions_reject() {
        let value = json!({"version":1,"requestId":"x","session":null,"call":{"method":"jobs.submit","payload":{"pluginId":"x","commandId":"run","input":{},"idempotencyKey":"x","background":false,"deadline":1,"agentId":"admin"}}});
        assert!(serde_json::from_value::<Request>(value).is_err());
        let mut core = RuntimeCore::validation("cli".into(), "desktop".into());
        let mut req = request(Call::Status, None);
        req.version = 2;
        assert!(matches!(
            core.handle("c", req).outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::ProtocolMismatch
            })
        ));
    }

    #[test]
    fn queued_todo_revocation_never_starts_a_runner_or_becomes_a_new_approval_request() {
        for mode in ["revoke", "regrant", "expire", "missing-policy"] {
            let mut core = RuntimeCore::managed(
                HashMap::from([
                    ("cli".into(), "local-cli".into()),
                    ("admin".into(), "local-manager".into()),
                ]),
                DataStore::memory().unwrap(),
                false,
                false,
            )
            .unwrap();
            let admin = open(&mut core, "admin", "admin");
            let (manifest, _) = core.catalog.command("plugin-todo-list", "run").unwrap();
            let grant = PermissionGrant {
                plugin_id: "plugin-todo-list".into(),
                command_id: "run".into(),
                target: GrantTarget::Cli,
                package_digest: digest(manifest),
                effects: vec!["data-read".into(), "data-write".into()],
                scopes: vec![Scope::PluginData {
                    key_prefix: "todos".into(),
                }],
                expires_at: now() + 60_000.0,
                max_calls: 16,
                cold_start: false,
                background: true,
            };
            assert!(matches!(
                core.handle("admin", request(Call::Grant(grant), Some(admin)))
                    .outcome,
                Outcome::Permissions(_)
            ));
            let proof = open(&mut core, "conn", "cli");
            let mut operation = submit();
            operation.plugin_id = "plugin-todo-list".into();
            operation.input = json!({"todo":"QUEUED_REVOCATION_PRIVATE_CANARY"});
            let outcome = core
                .handle("conn", request(Call::Submit(operation), Some(proof)))
                .outcome;
            let Outcome::Receipt(receipt) = outcome else {
                panic!("Receipt: {}", serde_json::to_string(&outcome).unwrap());
            };
            let job = &core.jobs[&receipt.run_id];
            assert_eq!(job.snapshot.state, JobState::Queued);
            let epoch = job.snapshot.grant_epoch;
            let original_started_at = job.started_at;
            assert_ne!(epoch, 0);
            let (manifest, command) = core.catalog.command("plugin-todo-list", "run").unwrap();
            let identity = CommandIdentity::from_manifest("local-cli", manifest, command);
            if mode == "missing-policy" {
                core.broker = CapabilityBroker::default();
            } else if mode == "expire" {
                core.broker
                    .restore_policy(
                        identity,
                        epoch,
                        Some(Grant {
                            effects: vec!["data-read".into(), "data-write".into()],
                            scopes: vec![Scope::PluginData {
                                key_prefix: "todos".into(),
                            }],
                            expires_at: now() - 1.0,
                            max_calls: 16,
                        }),
                    )
                    .unwrap();
            } else {
                core.broker.revoke(&identity).unwrap();
                if mode == "regrant" {
                    core.broker
                        .approve(
                            identity,
                            Grant {
                                effects: vec!["data-read".into(), "data-write".into()],
                                scopes: vec![Scope::PluginData {
                                    key_prefix: "todos".into(),
                                }],
                                expires_at: now() + 60_000.0,
                                max_calls: 16,
                            },
                        )
                        .unwrap();
                }
            }
            assert!(core.take_run(&receipt.run_id).is_none(), "{mode}");
            let job = &core.jobs[&receipt.run_id];
            assert_eq!(job.snapshot.state, JobState::Failed, "{mode}");
            assert!(
                matches!(
                    job.snapshot.result.as_ref().unwrap().outcome,
                    ExecutionOutcome::Failure {
                        error: RuntimeError {
                            code: ErrorCode::GrantRevoked
                        },
                        ..
                    }
                ),
                "{mode}"
            );
            assert_eq!(job.started_at, original_started_at);
            assert!(job.broker_session.is_none());
            assert!(!job
                .events
                .iter()
                .any(|event| event.state == JobState::Running));
            assert!(!core.pending.contains_key(&receipt.run_id));
            assert_eq!(
                core.data
                    .read("flowtools:plugin-todo-list", "todos")
                    .unwrap()
                    .revision,
                0
            );
            assert!(!serde_json::to_string(&job.snapshot)
                .unwrap()
                .contains("QUEUED_REVOCATION_PRIVATE_CANARY"));
        }
    }

    #[test]
    fn queued_revocation_survives_restart_without_inheriting_regrant() {
        for regrant in [false, true] {
            let profile = std::env::temp_dir().join(format!(
                "flowtools-validation-queued-revoke-{}",
                Uuid::new_v4()
            ));
            std::fs::create_dir(&profile).unwrap();
            let database = profile.join("runtime.sqlite");
            let reopen = || {
                RuntimeCore::managed(
                    HashMap::from([
                        ("cli".into(), "local-cli".into()),
                        ("admin".into(), "local-manager".into()),
                    ]),
                    DataStore::open(&database).unwrap(),
                    false,
                    false,
                )
                .unwrap()
            };
            let mut core = reopen();
            let admin = open(&mut core, "admin", "admin");
            let (manifest, _) = core.catalog.command("plugin-todo-list", "run").unwrap();
            let grant = PermissionGrant {
                plugin_id: "plugin-todo-list".into(),
                command_id: "run".into(),
                target: GrantTarget::Cli,
                package_digest: digest(manifest),
                effects: vec!["data-read".into(), "data-write".into()],
                scopes: vec![Scope::PluginData {
                    key_prefix: "todos".into(),
                }],
                expires_at: now() + 60_000.0,
                max_calls: 16,
                cold_start: false,
                background: true,
            };
            assert!(matches!(
                core.handle(
                    "admin",
                    request(Call::Grant(grant.clone()), Some(admin.clone()))
                )
                .outcome,
                Outcome::Permissions(_)
            ));
            let cli = open(&mut core, "cli", "cli");
            let operation = SubmitJob {
                plugin_id: "plugin-todo-list".into(),
                command_id: "run".into(),
                input: json!({"todo":"RESTART_REVOCATION_PRIVATE_CANARY"}),
                idempotency_key: "queued-before-revoke".into(),
                background: true,
                deadline: now() + 30_000.0,
            };
            let Outcome::Receipt(receipt) = core
                .handle("cli", request(Call::Submit(operation), Some(cli)))
                .outcome
            else {
                panic!("Receipt");
            };
            assert!(matches!(
                core.handle(
                    "admin",
                    request(
                        Call::Revoke(PermissionKey {
                            plugin_id: "plugin-todo-list".into(),
                            command_id: "run".into(),
                            target: GrantTarget::Cli,
                        }),
                        Some(admin.clone())
                    )
                )
                .outcome,
                Outcome::Permissions(_)
            ));
            if regrant {
                assert!(matches!(
                    core.handle("admin", request(Call::Grant(grant), Some(admin)))
                        .outcome,
                    Outcome::Permissions(_)
                ));
            }
            drop(core);
            let core = reopen();
            assert!(core.pending_runs().is_empty());
            let snapshot = core.job_snapshot(&receipt.run_id).unwrap();
            assert_eq!(snapshot.state, JobState::Interrupted);
            assert!(matches!(
                snapshot.result.as_ref().unwrap().outcome,
                ExecutionOutcome::Failure {
                    error: RuntimeError {
                        code: ErrorCode::GrantRevoked
                    },
                    ..
                }
            ));
            assert!(!serde_json::to_string(&snapshot)
                .unwrap()
                .contains("RESTART_REVOCATION_PRIVATE_CANARY"));
            assert_eq!(
                core.data
                    .read("flowtools:plugin-todo-list", "todos")
                    .unwrap()
                    .revision,
                0
            );
            assert!(core
                .private_jobs
                .as_ref()
                .unwrap()
                .read(&receipt.run_id, "input")
                .is_err());
        }
    }

    #[test]
    fn capability_denial_creates_no_job_runner_or_pending_payload() {
        let mut core = RuntimeCore::validation("cli".into(), "desktop".into());
        let proof = open(&mut core, "conn", "cli");
        for plugin_id in ["plugin-todo-list", "plugin-website-latency"] {
            let mut operation = submit();
            operation.plugin_id = plugin_id.into();
            operation.input = json!({});
            let reply = core.handle(
                "conn",
                request(Call::Submit(operation), Some(proof.clone())),
            );
            assert!(matches!(
                reply.outcome,
                Outcome::Error(RuntimeError {
                    code: ErrorCode::ApprovalRequired
                })
            ));
            assert!(core.jobs.is_empty());
            assert!(core.pending.is_empty());
            assert!(core.keys.is_empty());
        }
    }

    #[test]
    fn runner_session_ends_with_job_and_disconnect_cannot_preserve_foreground_access() {
        let mut core = RuntimeCore::validation("cli".into(), "desktop".into());
        let proof = open(&mut core, "conn", "cli");
        let Outcome::Receipt(receipt) = core
            .handle("conn", request(Call::Submit(submit()), Some(proof)))
            .outcome
        else {
            panic!("Receipt");
        };
        core.take_run(&receipt.run_id).unwrap();
        assert!(core.check_run(&receipt.run_id).is_ok());
        core.disconnect("conn");
        assert_eq!(core.check_run(&receipt.run_id), Err(ErrorCode::Aborted));
        core.finish(&receipt.run_id, Err(ErrorCode::Aborted));
        assert!(core.jobs[&receipt.run_id].broker_session.is_none());
        assert_eq!(
            core.check_run(&receipt.run_id),
            Err(ErrorCode::SessionInvalid)
        );
    }

    #[test]
    fn compatibility_deadline_errors_and_redacted_events_are_stable() {
        let mut core = RuntimeCore::validation("private-token".into(), "desktop".into());
        for (version, instance, expected) in [
            ("future", None, ErrorCode::ClientIncompatible),
            (
                CLIENT_VERSION,
                Some("old-instance".to_owned()),
                ErrorCode::InstanceMismatch,
            ),
        ] {
            let reply = core.handle(
                "c",
                request(
                    Call::Open(OpenSession {
                        token: "private-token".into(),
                        client_version: version.into(),
                        expected_instance_id: instance,
                    }),
                    None,
                ),
            );
            assert!(matches!(reply.outcome, Outcome::Error(RuntimeError {code}) if code==expected));
        }
        let proof = open(&mut core, "c", "private-token");
        let mut expired = submit();
        expired.deadline = now() - 1.0;
        assert!(matches!(
            core.handle("c", request(Call::Submit(expired), Some(proof.clone())))
                .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::Timeout
            })
        ));
        let mut original = submit();
        original.input = json!({"text":"private-payload-canary"});
        let Outcome::Receipt(receipt) = core
            .handle(
                "c",
                request(Call::Submit(original.clone()), Some(proof.clone())),
            )
            .outcome
        else {
            panic!("receipt");
        };
        let mut extension = original;
        extension.deadline += 1.0;
        assert!(matches!(
            core.handle("c", request(Call::Submit(extension), Some(proof.clone())))
                .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::IdempotencyConflict
            })
        ));
        core.take_run(&receipt.run_id).unwrap();
        core.finish(&receipt.run_id, Err(ErrorCode::ExecutionFailed));
        let events = core.handle(
            "c",
            request(
                Call::Events(EventCursor {
                    run_id: receipt.run_id.clone(),
                    after_sequence: 0,
                }),
                Some(proof.clone()),
            ),
        );
        let text = serde_json::to_string(&events).unwrap();
        for private in [
            "private-token",
            "private-payload-canary",
            "input",
            "result",
            "message",
        ] {
            assert!(!text.contains(private));
        }
        let Outcome::Job(job) = core
            .handle(
                "c",
                request(
                    Call::Job(JobKey {
                        run_id: receipt.run_id.clone(),
                    }),
                    Some(proof.clone()),
                ),
            )
            .outcome
        else {
            panic!("job");
        };
        assert!(matches!(
            job.result.unwrap().outcome,
            ExecutionOutcome::Failure {
                success: false,
                error: RuntimeError {
                    code: ErrorCode::ExecutionFailed
                }
            }
        ));
        core.finish(&receipt.run_id, Err(ErrorCode::Timeout));
        assert_eq!(core.state(&receipt.run_id), Some(JobState::Failed));
        let mut replacement = RuntimeCore::validation("private-token".into(), "desktop".into());
        assert!(matches!(
            replacement
                .handle("c", request(Call::Status, Some(proof)))
                .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::InstanceMismatch
            })
        ));
    }
}
