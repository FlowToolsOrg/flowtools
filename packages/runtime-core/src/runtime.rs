use crate::{
    catalog::{digest, BuiltinCatalog},
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
        }
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
        match request.call {
            Call::Status => Ok(Outcome::Status(RuntimeStatus {
                instance_id: self.instance_id.clone(),
                mode: "validation".into(),
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
            Call::Open(_) => unreachable!(),
        }
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
        let input =
            self.catalog
                .prepare_input(&submit.plugin_id, &submit.command_id, submit.input)?;
        let (manifest, command) = self
            .catalog
            .command(&submit.plugin_id, &submit.command_id)?;
        let accepted_at = now();
        if submit.deadline <= accepted_at {
            return Err(ErrorCode::Timeout);
        }
        if submit.deadline > accepted_at + command["resources"]["timeoutMs"].as_f64().unwrap_or(0.0)
        {
            return Err(ErrorCode::InvalidRequest);
        }
        let package_digest = digest(manifest);
        let action_digest = digest(
            &json!({ "package":package_digest, "command":submit.command_id, "input":input, "background":submit.background }),
        );
        let key = (caller.to_owned(), submit.idempotency_key);
        if let Some(run_id) = self.keys.get(&key) {
            let job = &self.jobs[run_id];
            if job.action_digest != action_digest {
                return Err(ErrorCode::IdempotencyConflict);
            }
            return Ok(self.receipt(run_id));
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
            grant_epoch: 0,
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
                | (JobState::Queued, JobState::Running | JobState::Cancelling)
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
        self.transition(run_id, JobState::Running).ok()?;
        self.jobs.get_mut(run_id)?.started_at = now();
        Some(run)
    }

    pub fn state(&self, run_id: &str) -> Option<JobState> {
        self.jobs.get(run_id).map(|j| j.snapshot.state)
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

#[cfg(test)]
mod tests {
    use super::*;
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
        let Outcome::Receipt(receipt) = core
            .handle(
                "cli-conn",
                request(Call::Submit(submit()), Some(cli.clone())),
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
                request(Call::Submit(submit()), Some(cli.clone())),
            )
            .outcome
        else {
            panic!("Receipt retry")
        };
        assert_eq!(repeated.run_id, receipt.run_id);
        let mut changed = submit();
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
}
