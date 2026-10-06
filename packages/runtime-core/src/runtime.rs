use crate::{
    broker::{BrokerSession, CapabilityBroker, CapabilityOperation, CommandIdentity, Grant, Scope},
    catalog::{digest, BuiltinCatalog},
    data::DataStore,
    policy::PolicyStore,
    private_jobs::PrivateJobs,
    protocol::*,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

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
}
impl StoredJob {
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
        }
    }
}

/// Payload is transient runner input, never serializable diagnostics/history.
pub struct RunSpec {
    pub run_id: String,
    pub plugin_id: String,
    pub command_id: String,
    pub input: Value,
    pub deadline: f64,
    pub package_digest: String,
    pub data: Option<DataSnapshot>,
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
        }
    }

    pub fn with_data_store(mut self, store: DataStore) -> Self {
        self.data = store;
        self
    }

    pub fn managed(
        tokens: HashMap<String, String>,
        mut store: DataStore,
        management_only: bool,
        cold_started: bool,
    ) -> Result<Self, ErrorCode> {
        let policy = PolicyStore::load(&mut store, &PolicyStore::catalog_digest())?;
        if cold_started && !policy.cold_start {
            return Err(ErrorCode::ColdStartDenied);
        }
        let mut core =
            Self::validation("unused-cli".into(), "unused-desktop".into()).with_data_store(store);
        core.tokens = tokens;
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
        Ok(core)
    }

    fn persist_job(&self, job: &Job) -> Result<(), ErrorCode> {
        if self.private_jobs.is_some() {
            self.data.save_job(
                &job.snapshot.run_id,
                &serde_json::to_value(StoredJob::from_job(job))
                    .map_err(|_| ErrorCode::StorageFailed)?,
            )?;
        }
        Ok(())
    }
    fn restore_jobs(&mut self) -> Result<(), ErrorCode> {
        if self.private_jobs.is_none() {
            return Ok(());
        }
        for value in self.data.load_jobs()? {
            let stored: StoredJob =
                serde_json::from_value(value).map_err(|_| ErrorCode::StoreCorrupt)?;
            if stored.format_version != 1
                || uuid::Uuid::parse_str(&stored.snapshot.run_id).is_err()
                || !valid_key(&stored.idempotency_key)
            {
                return Err(ErrorCode::StoreCorrupt);
            }
            let run = stored.snapshot.run_id.clone();
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
                },
            );
            let job = &self.jobs[&run];
            if job.snapshot.state.terminal() {
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
            let catalog = self.catalog.clone();
            let contract = catalog.command(&snapshot.plugin_id, &snapshot.command_id);
            let authorization = contract.and_then(|(manifest, command)| {
                let identity =
                    CommandIdentity::from_manifest(&snapshot.root_caller, manifest, command);
                self.managed_authorization(&identity, job.background)?;
                if digest(manifest) != snapshot.package_digest
                    || self.broker.authorize_command(&identity, command, now())?
                        != snapshot.grant_epoch
                {
                    return Err(ErrorCode::GrantRevoked);
                }
                if snapshot.deadline <= now() {
                    return Err(ErrorCode::Timeout);
                }
                Ok(())
            });
            // Queued has never entered a runner. Running Base64 is proven pure and
            // deterministic; every other running operation needs manual review.
            let retryable = matches!(snapshot.state, JobState::Accepted | JobState::Queued)
                || (snapshot.state == JobState::Running
                    && snapshot.plugin_id == "plugin-base64-encoder");
            if job.background && retryable && authorization.is_ok() {
                let input = self.private_jobs.as_ref().unwrap().read(&run, "input")?;
                let prepared = self.catalog.prepare_input_mode(
                    &snapshot.plugin_id,
                    &snapshot.command_id,
                    input.clone(),
                    true,
                )?;
                if prepared != input
                    || digest(
                        &json!({"package":snapshot.package_digest,"command":snapshot.command_id,"lock":snapshot.dependency_lock,"input":input,"background":job.background,"deadline":snapshot.deadline}),
                    ) != job.action_digest
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
        self.private_jobs
            .as_ref()
            .unwrap()
            .remove_orphans(&self.jobs.keys().cloned().collect())?;
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
                        enabled: true,
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
        let (manifest, command) = self
            .catalog
            .command(&submit.plugin_id, &submit.command_id)?;
        let identity = CommandIdentity::from_manifest(caller, manifest, command);
        self.managed_authorization(&identity, submit.background)?;
        let grant_epoch = self.broker.authorize_command(&identity, command, now())?;
        let input = self.catalog.prepare_input_mode(
            &submit.plugin_id,
            &submit.command_id,
            submit.input,
            self.policy.is_some(),
        )?;
        let (manifest, command) = self
            .catalog
            .command(&submit.plugin_id, &submit.command_id)?;
        let accepted_at = now();
        let package_digest = digest(manifest);
        let action_digest = digest(
            &json!({ "package":package_digest, "command":submit.command_id, "lock":"t1-no-dependencies-v1", "input":input, "background":submit.background, "deadline":submit.deadline }),
        );
        let key = (caller.to_owned(), submit.idempotency_key.clone());
        if let Some(run_id) = self.keys.get(&key) {
            let job = &self.jobs[run_id];
            if job.action_digest != action_digest {
                return Err(ErrorCode::IdempotencyConflict);
            }
            return Ok(self.receipt(run_id));
        }
        if submit.deadline <= accepted_at {
            return Err(ErrorCode::Timeout);
        }
        if submit.deadline > accepted_at + command["resources"]["timeoutMs"].as_f64().unwrap_or(0.0)
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
            dependency_lock: "t1-no-dependencies-v1".into(),
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
        self.jobs.insert(
            run_id.clone(),
            Job {
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
            },
        );
        if let Some(ref payloads) = self.private_jobs {
            if let Err(error) = payloads.write(&run_id, "input", &input) {
                self.jobs.remove(&run_id);
                return Err(error);
            }
            if self.persist_job(&self.jobs[&run_id]).is_err() {
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
        let stored =
            serde_json::to_value(StoredJob::from_job(job)).map_err(|_| ErrorCode::StorageFailed)?;
        if self.private_jobs.is_some() {
            if let Err(code) = self.data.save_job(run_id, &stored) {
                job.snapshot = previous;
                job.events.pop();
                self.stopping = true;
                return Err(code);
            }
        }
        Ok(())
    }

    pub fn take_run(&mut self, run_id: &str) -> Option<RunSpec> {
        let mut run = self.pending.remove(run_id)?;
        if self.jobs[run_id].snapshot.state != JobState::Queued {
            return None;
        }
        let snapshot = &self.jobs[run_id].snapshot;
        let (manifest, command) = self
            .catalog
            .command(&snapshot.plugin_id, &snapshot.command_id)
            .ok()?;
        let identity = CommandIdentity::from_manifest(&snapshot.root_caller, manifest, command);
        let session = self
            .broker
            .authorize_command(&identity, command, now())
            .and_then(|epoch| {
                if epoch != snapshot.grant_epoch {
                    return Err(ErrorCode::GrantRevoked);
                }
                self.broker
                    .bind(identity, command.clone(), snapshot.deadline, now())
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
        let job = self.jobs.get(run_id).ok_or(ErrorCode::JobNotFound)?;
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
        .prepare("SELECT run_id,metadata FROM jobs LIMIT 1025")
        .map_err(|_| ErrorCode::StoreCorrupt)?
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| ErrorCode::StoreCorrupt)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ErrorCode::StoreCorrupt)?;
    if rows.len() > 1024 {
        return Err(ErrorCode::BudgetExceeded);
    }
    for (run, value) in rows {
        let mut stored: StoredJob =
            serde_json::from_str(&value).map_err(|_| ErrorCode::StoreCorrupt)?;
        if stored.snapshot.run_id != run
            || stored.format_version != 1
            || value.len() > 32768
            || uuid::Uuid::parse_str(&run).is_err()
            || !valid_key(&stored.idempotency_key)
        {
            return Err(ErrorCode::StoreCorrupt);
        }
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
mod tests {
    use super::*;
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
