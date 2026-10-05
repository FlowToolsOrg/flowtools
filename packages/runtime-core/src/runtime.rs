use crate::{
    broker::{BrokerSession, CapabilityBroker, CapabilityOperation, CommandIdentity, Grant, Scope},
    catalog::{digest, BuiltinCatalog},
    data::DataStore,
    policy::PolicyStore,
    protocol::*,
};
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
}

/// Payload is transient runner input, never serializable diagnostics/history.
pub struct RunSpec {
    pub run_id: String,
    pub plugin_id: String,
    pub command_id: String,
    pub input: Value,
    pub deadline: f64,
    pub package_digest: String,
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
        Ok(core)
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
            })),
            Call::Plugins => Ok(Outcome::Plugins(
                self.catalog
                    .list()
                    .map(|(id, manifest)| PluginStatus {
                        plugin_id: id.clone(),
                        version: manifest["version"].as_str().unwrap().into(),
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
            Call::Job(key) => Ok(Outcome::Job(Box::new(
                self.jobs
                    .get(&key.run_id)
                    .ok_or(ErrorCode::JobNotFound)?
                    .snapshot
                    .clone(),
            ))),
            Call::Cancel(key) => {
                let job = self.jobs.get(&key.run_id).ok_or(ErrorCode::JobNotFound)?;
                if job.snapshot.root_caller != caller {
                    return Err(ErrorCode::SessionInvalid);
                }
                self.cancel(&key.run_id);
                Ok(Outcome::Job(Box::new(
                    self.jobs[&key.run_id].snapshot.clone(),
                )))
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
        let input =
            self.catalog
                .prepare_input(&submit.plugin_id, &submit.command_id, submit.input)?;
        let (manifest, command) = self
            .catalog
            .command(&submit.plugin_id, &submit.command_id)?;
        let accepted_at = now();
        let package_digest = digest(manifest);
        let action_digest = digest(
            &json!({ "package":package_digest, "command":submit.command_id, "input":input, "background":submit.background, "deadline":submit.deadline }),
        );
        let key = (caller.to_owned(), submit.idempotency_key);
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
        if self.jobs.len() >= 128 {
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
            },
        );
        self.pending.insert(
            run_id.clone(),
            RunSpec {
                run_id: run_id.clone(),
                plugin_id: submit.plugin_id,
                command_id: submit.command_id,
                input,
                deadline: submit.deadline,
                package_digest,
            },
        );
        self.transition(&run_id, JobState::Queued)
            .expect("Accepted -> queued");
        self.keys.insert(key, run_id.clone());
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
        job.snapshot.state = next;
        job.snapshot.sequence += 1;
        job.events.push(JobEvent {
            run_id: run_id.into(),
            sequence: job.snapshot.sequence,
            state: next,
        });
        Ok(())
    }

    pub fn take_run(&mut self, run_id: &str) -> Option<RunSpec> {
        let run = self.pending.remove(run_id)?;
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
        Some(run)
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
        if self.state(run_id).is_none_or(JobState::terminal) {
            return;
        }
        let cancelled = self.state(run_id) == Some(JobState::Cancelling);
        let (state, outcome) = if cancelled {
            (
                JobState::Cancelled,
                ExecutionOutcome::Failure {
                    success: false,
                    error: RuntimeError {
                        code: ErrorCode::Aborted,
                    },
                },
            )
        } else {
            match result {
                Ok(data) => {
                    let snapshot = &self.jobs[run_id].snapshot;
                    if self
                        .catalog
                        .output_valid(&snapshot.plugin_id, &snapshot.command_id, &data)
                    {
                        (
                            JobState::Succeeded,
                            ExecutionOutcome::Success {
                                success: true,
                                data,
                            },
                        )
                    } else {
                        (
                            JobState::Failed,
                            ExecutionOutcome::Failure {
                                success: false,
                                error: RuntimeError {
                                    code: ErrorCode::OutputInvalid,
                                },
                            },
                        )
                    }
                }
                Err(code) => (
                    JobState::Failed,
                    ExecutionOutcome::Failure {
                        success: false,
                        error: RuntimeError { code },
                    },
                ),
            }
        };
        if self.transition(run_id, state).is_err() {
            return;
        }
        let job = self.jobs.get_mut(run_id).unwrap();
        if let Some(session) = job.broker_session.take() {
            self.broker.close(&session);
        }
        let finished_at = now().max(job.started_at);
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
    }

    pub fn cancel(&mut self, run_id: &str) {
        if matches!(
            self.state(run_id),
            Some(JobState::Queued | JobState::Running)
        ) {
            self.transition(run_id, JobState::Cancelling)
                .expect("Cancellable state");
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

#[cfg(test)]
mod tests {
    use super::*;
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
