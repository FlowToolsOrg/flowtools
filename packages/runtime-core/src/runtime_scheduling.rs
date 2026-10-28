use super::*;
use crate::scheduling::{
    commands::{CreateSchedule, ReplaceSchedule, ScheduleKey},
    store::{ClaimResult, ScheduleBinding, ScheduleRecord, ScheduleStore},
};

impl RuntimeCore {
    /// T0 holds the broker and writer through the notification commit point.
    pub fn deliver_notification(
        &mut self,
        run_id: &str,
        title: &str,
        message: &str,
        adapter: impl FnOnce() -> crate::notifications::NotificationStatus,
    ) -> Result<crate::notifications::NotificationStatus, ErrorCode> {
        self.check_run(run_id)?;
        let session = if let Some(call) = self.service_invocations.get(run_id) {
            call.session.clone()
        } else {
            self.jobs
                .get(run_id)
                .and_then(|job| job.broker_session.clone())
                .ok_or(ErrorCode::SessionInvalid)?
        };
        let operation = CapabilityOperation::Notification {
            title: title.into(),
            message: message.into(),
        };
        let result = self.broker.invoke(&session, &operation, now(), |_| {
            crate::notifications::attempt(&mut self.data, run_id, title, message, adapter)
        });
        if matches!(
            result,
            Err(ErrorCode::StorageFailed | ErrorCode::StoreCorrupt)
        ) {
            self.fail_closed();
        }
        result
    }
    pub(super) fn create_schedule(
        &mut self,
        create: CreateSchedule,
    ) -> Result<ScheduleRecord, ErrorCode> {
        self.persist_schedule(create, None)
    }

    pub(super) fn replace_schedule(
        &mut self,
        replace: ReplaceSchedule,
    ) -> Result<ScheduleRecord, ErrorCode> {
        replace.previous.validate()?;
        self.persist_schedule(replace.create, Some(replace.previous))
    }

    fn persist_schedule(
        &mut self,
        create: CreateSchedule,
        previous: Option<ScheduleKey>,
    ) -> Result<ScheduleRecord, ErrorCode> {
        create.validate()?;
        let caller = create.target.caller();
        let (manifest, command) = self
            .catalog
            .command(&create.plugin_id, &create.command_id)?;
        let identity = CommandIdentity::from_manifest(caller, manifest, command);
        self.managed_authorization(&identity, true)?;
        let epoch = self.broker.authorize_command(&identity, command, now())?;
        if f64::from(create.timeout_ms) > command["resources"]["timeoutMs"].as_f64().unwrap_or(0.0)
        {
            return Err(ErrorCode::InputInvalid);
        }
        let input = self.catalog.prepare_input_mode(
            &create.plugin_id,
            &create.command_id,
            create.input,
            true,
        )?;
        let hash = if let Some(key) = &previous {
            digest(&json!([
                "schedule-replacement-v1",
                caller,
                create.request_key,
                key
            ]))
        } else {
            digest(&json!(["schedule-request-v1", caller, create.request_key]))
        };
        let id = format!(
            "{}-{}-{}-{}-{}",
            &hash[..8],
            &hash[8..12],
            &hash[12..16],
            &hash[16..20],
            &hash[20..32]
        );
        let binding = ScheduleBinding {
            identity,
            grant_epoch: epoch,
            input_digest: digest(&input),
            timeout_ms: create.timeout_ms,
        };
        if let Some(existing) = ScheduleStore::list(&self.data)?
            .into_iter()
            .find(|record| record.id == id)
        {
            if existing.binding.identity != binding.identity
                || existing.binding.grant_epoch != binding.grant_epoch
                || existing.binding.input_digest != binding.input_digest
                || existing.binding.timeout_ms != binding.timeout_ms
                || digest(
                    &serde_json::to_value(&existing.definition)
                        .map_err(|_| ErrorCode::StorageFailed)?,
                ) != digest(
                    &serde_json::to_value(&create.definition)
                        .map_err(|_| ErrorCode::InputInvalid)?,
                )
            {
                return Err(ErrorCode::IdempotencyConflict);
            }
            return Ok(existing);
        }
        if let Some(key) = &previous {
            let old = ScheduleStore::list(&self.data)?
                .into_iter()
                .find(|record| record.id == key.id)
                .ok_or(ErrorCode::JobNotFound)?;
            if old.revision != key.revision || old.cancelled {
                return Err(ErrorCode::RevisionConflict);
            }
            if old.last_claim.as_ref().is_some_and(|claim| matches!(&claim.result, ClaimResult::Submitted { run_id } if self.jobs.get(run_id).is_some_and(|job| !job.snapshot.state.terminal()))) {
                return Err(ErrorCode::RuntimeBusy);
            }
        }
        self.private_jobs
            .as_ref()
            .ok_or(ErrorCode::ApprovalRequired)?
            .write_or_verify(&id, &input)?;
        if let Some(key) = &previous {
            ScheduleStore::replace(
                &mut self.data,
                key,
                id,
                create.definition,
                binding,
                now() as u64,
            )
        } else {
            ScheduleStore::create(&mut self.data, id, create.definition, binding, now() as u64)
        }
    }

    pub(super) fn cancel_schedule(
        &mut self,
        key: ScheduleKey,
    ) -> Result<ScheduleRecord, ErrorCode> {
        let record = ScheduleStore::cancel(&mut self.data, &key.id, key.revision)?;
        if let Some(claim) = &record.last_claim {
            if let ClaimResult::Submitted { run_id } = &claim.result {
                // Use the ordinary cancellation path with a T0-bound caller.
                if self
                    .jobs
                    .get(run_id)
                    .is_some_and(|job| !job.snapshot.state.terminal())
                {
                    self.cancel(run_id);
                    if self.stopping {
                        return Err(ErrorCode::StorageFailed);
                    }
                }
            }
        }
        Ok(record)
    }

    /// Host-owned tick. It neither initializes storage nor starts another Host.
    /// Claim commit precedes submit; a crash can lose an attempt, never replay it.
    pub fn tick_schedules(&mut self) -> Result<Vec<String>, ErrorCode> {
        if self.policy.is_none() || self.management_only || self.stopping {
            return Ok(Vec::new());
        }
        let jobs = &self.jobs;
        let records = ScheduleStore::claim_due(&mut self.data, now() as u64, |record| {
            !record.last_claim.as_ref().is_some_and(|claim| matches!(&claim.result,
                ClaimResult::Submitted { run_id } if jobs.get(run_id).is_some_and(|job| !job.snapshot.state.terminal())))
        })?;
        let mut accepted = Vec::new();
        for record in records {
            if self.stopping {
                return Err(ErrorCode::RuntimeBusy);
            }
            let claim = record.last_claim.as_ref().ok_or(ErrorCode::StoreCorrupt)?;
            let result = self.submit_schedule(&record);
            let outcome = match result {
                Ok(receipt) => {
                    accepted.push(receipt.run_id.clone());
                    ClaimResult::Submitted {
                        run_id: receipt.run_id,
                    }
                }
                Err(code) => ClaimResult::Rejected { code },
            };
            if let Err(code) =
                ScheduleStore::finish(&mut self.data, &record.id, &claim.token, outcome.clone())
            {
                self.stopping = true;
                return Err(code);
            }
            if matches!(
                outcome,
                ClaimResult::Rejected {
                    code: ErrorCode::StorageFailed
                        | ErrorCode::StoreCorrupt
                        | ErrorCode::AcceptanceUnknown
                }
            ) {
                self.fail_closed();
                return Err(ErrorCode::StorageFailed);
            }
            if matches!(
                outcome,
                ClaimResult::Rejected {
                    code: ErrorCode::ApprovalRequired
                        | ErrorCode::GrantRevoked
                        | ErrorCode::PluginNotFound
                }
            ) {
                ScheduleStore::cancel(&mut self.data, &record.id, record.revision)?;
            }
        }
        Ok(accepted)
    }

    fn submit_schedule(&mut self, record: &ScheduleRecord) -> Result<JobReceipt, ErrorCode> {
        let binding = &record.binding;
        let (manifest, command) = self
            .catalog
            .command(&binding.identity.plugin_id, &binding.identity.command_id)?;
        let current = CommandIdentity::from_manifest(&binding.identity.caller, manifest, command);
        if current != binding.identity {
            return Err(ErrorCode::GrantRevoked);
        }
        self.managed_authorization(&current, true)?;
        if self.broker.authorize_command(&current, command, now())? != binding.grant_epoch {
            return Err(ErrorCode::GrantRevoked);
        }
        let input = self
            .private_jobs
            .as_ref()
            .ok_or(ErrorCode::ApprovalRequired)?
            .read(&record.id, "input")?;
        if digest(&input) != binding.input_digest {
            return Err(ErrorCode::StoreCorrupt);
        }
        let claim = record.last_claim.as_ref().ok_or(ErrorCode::StoreCorrupt)?;
        self.submit(
            "host-scheduler",
            &binding.identity.caller,
            SubmitJob {
                plugin_id: binding.identity.plugin_id.clone(),
                command_id: binding.identity.command_id.clone(),
                input,
                idempotency_key: format!("schedule-{}", claim.token),
                background: true,
                deadline: now() + f64::from(binding.timeout_ms),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduling::{store::ScheduleDefinition, MissedRunPolicy};

    fn request(call: Call, session: Option<SessionProof>) -> Request {
        Request {
            version: PROTOCOL_MAJOR,
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
            panic!("authenticated session");
        };
        proof
    }
    fn fixture() -> (RuntimeCore, std::path::PathBuf, SessionProof, SessionProof) {
        let profile =
            std::env::temp_dir().join(format!("flowtools-validation-schedule-{}", Uuid::new_v4()));
        std::fs::create_dir(&profile).unwrap();
        let mut core = RuntimeCore::managed(
            HashMap::from([
                ("manager-token".into(), "local-manager".into()),
                ("cli-token".into(), "local-cli".into()),
            ]),
            DataStore::open(&profile.join("runtime.sqlite")).unwrap(),
            false,
            false,
        )
        .unwrap();
        let manager = open(&mut core, "manager", "manager-token");
        let cli = open(&mut core, "cli", "cli-token");
        (core, profile, manager, cli)
    }
    fn approve(core: &mut RuntimeCore, manager: &SessionProof, background: bool) {
        let (manifest, command) = core
            .catalog
            .command("plugin-base64-encoder", "run")
            .unwrap();
        let identity = CommandIdentity::from_manifest("local-cli", manifest, command);
        let grant = PermissionGrant {
            plugin_id: identity.plugin_id,
            command_id: identity.command_id,
            target: GrantTarget::Cli,
            package_digest: identity.package_digest,
            effects: vec![],
            scopes: vec![],
            expires_at: now() + 60_000.0,
            max_calls: 8,
            cold_start: false,
            background,
        };
        assert!(matches!(
            core.handle(
                "manager",
                request(Call::Grant(grant), Some(manager.clone()))
            )
            .outcome,
            Outcome::Permissions(_)
        ));
    }
    fn create() -> CreateSchedule {
        CreateSchedule {
            request_key: Uuid::new_v4().to_string(),
            target: GrantTarget::Cli,
            plugin_id: "plugin-base64-encoder".into(),
            command_id: "run".into(),
            input: json!({"text":"PRIVATE_SCHEDULE_TEXT"}),
            timeout_ms: 1_000,
            definition: ScheduleDefinition {
                first_due_ms: now() as u64 - 1,
                interval_ms: None,
                occurrences: 1,
                grace_ms: 1_000,
                missed: MissedRunPolicy::CoalesceLatest,
            },
        }
    }
    fn record(
        core: &mut RuntimeCore,
        manager: &SessionProof,
        create: CreateSchedule,
    ) -> ScheduleRecord {
        let outcome = core
            .handle(
                "manager",
                request(Call::ScheduleCreate(create), Some(manager.clone())),
            )
            .outcome;
        if let Outcome::Schedule(record) = outcome {
            *record
        } else {
            panic!("schedule: {outcome:?}");
        }
    }

    #[test]
    fn notification_needs_current_policy_and_durable_claim_before_os_effect() {
        use crate::notifications::NotificationStatus;
        let (mut core, profile, manager, _) = fixture();
        let (manifest, command) = core
            .catalog
            .command("plugin-local-reminder", "run")
            .unwrap();
        let identity = CommandIdentity::from_manifest("local-cli", manifest, command);
        let grant = PermissionGrant {
            plugin_id: identity.plugin_id.clone(),
            command_id: "run".into(),
            target: GrantTarget::Cli,
            package_digest: identity.package_digest,
            effects: vec!["notification".into()],
            scopes: vec![Scope::Notification],
            expires_at: now() + 60_000.0,
            max_calls: 8,
            cold_start: false,
            background: true,
        };
        assert!(matches!(
            core.handle(
                "manager",
                request(Call::Grant(grant), Some(manager.clone()))
            )
            .outcome,
            Outcome::Permissions(_)
        ));
        let mut create = create();
        create.plugin_id = "plugin-local-reminder".into();
        create.input = json!({"title":"Private reminder", "message":"Private body"});
        record(&mut core, &manager, create);
        let id = core.tick_schedules().unwrap().remove(0);
        core.take_run(&id).unwrap();
        assert_eq!(
            core.deliver_notification(&id, "Private reminder", "Private body", || {
                NotificationStatus::Requested
            })
            .unwrap(),
            NotificationStatus::Requested
        );
        assert_eq!(
            core.deliver_notification(&id, "Private reminder", "Private body", || panic!(
                "no duplicate OS call"
            ))
            .unwrap(),
            NotificationStatus::Requested
        );
        core.handle(
            "manager",
            request(
                Call::Revoke(PermissionKey {
                    plugin_id: identity.plugin_id,
                    command_id: "run".into(),
                    target: GrantTarget::Cli,
                }),
                Some(manager),
            ),
        );
        assert!(core
            .deliver_notification(&id, "Private reminder", "Private body", || panic!(
                "revoked callback"
            ))
            .is_err());
        drop(core);
        std::fs::remove_dir_all(profile).unwrap();
    }

    #[test]
    fn scheduling_requires_management_and_existing_background_authorization() {
        let (mut core, profile, manager, cli) = fixture();
        let create = create();
        assert!(matches!(
            core.handle(
                "cli",
                request(Call::ScheduleCreate(create.clone()), Some(cli))
            )
            .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::SessionInvalid
            })
        ));
        approve(&mut core, &manager, false);
        assert!(matches!(
            core.handle(
                "manager",
                request(Call::ScheduleCreate(create.clone()), Some(manager.clone()))
            )
            .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::ApprovalRequired
            })
        ));
        assert!(ScheduleStore::list(&core.data).unwrap().is_empty());
        approve(&mut core, &manager, true);
        let record = record(&mut core, &manager, create);
        assert_eq!(record.binding.identity.caller, "local-cli");
        assert!(!serde_json::to_string(&record)
            .unwrap()
            .contains("PRIVATE_SCHEDULE_TEXT"));
        drop(core);
        std::fs::remove_dir_all(profile).unwrap();
    }

    #[test]
    fn scheduled_acceptance_uses_private_inputs_and_survives_client_disconnect_without_replay() {
        let (mut core, profile, manager, _) = fixture();
        approve(&mut core, &manager, true);
        let record = record(&mut core, &manager, create());
        core.disconnect("manager");
        core.disconnect("cli");
        let jobs = core.tick_schedules().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(core.jobs[&jobs[0]].snapshot.root_caller, "local-cli");
        assert!(core.jobs[&jobs[0]].background);
        assert!(core.tick_schedules().unwrap().is_empty());
        let spec = core.take_run(&jobs[0]).unwrap();
        assert_eq!(spec.input["text"], "PRIVATE_SCHEDULE_TEXT");
        core.finish(&jobs[0], Err(ErrorCode::Aborted));
        drop(core);
        let mut restored = RuntimeCore::managed(
            HashMap::new(),
            DataStore::open(&profile.join("runtime.sqlite")).unwrap(),
            false,
            false,
        )
        .unwrap();
        assert!(restored.tick_schedules().unwrap().is_empty());
        assert_eq!(
            ScheduleStore::list(&restored.data).unwrap()[0].id,
            record.id
        );
        drop(restored);
        std::fs::remove_dir_all(profile).unwrap();
    }

    #[test]
    fn grant_epoch_changes_cancel_existing_schedules_instead_of_resurrecting_them() {
        let (mut core, profile, manager, _) = fixture();
        approve(&mut core, &manager, true);
        record(&mut core, &manager, create());
        approve(&mut core, &manager, true);
        assert!(core.tick_schedules().unwrap().is_empty());
        let records = ScheduleStore::list(&core.data).unwrap();
        assert!(records[0].cancelled);
        assert_eq!(
            records[0].last_claim.as_ref().unwrap().result,
            ClaimResult::Rejected {
                code: ErrorCode::GrantRevoked
            }
        );
        assert!(core.jobs.is_empty());
        assert!(core.tick_schedules().unwrap().is_empty());
        drop(core);
        std::fs::remove_dir_all(profile).unwrap();
    }

    #[test]
    fn create_retry_is_bound_to_exact_input_and_cancel_does_not_rearm() {
        let (mut core, profile, manager, _) = fixture();
        approve(&mut core, &manager, true);
        let create = create();
        let first = record(&mut core, &manager, create.clone());
        assert_eq!(record(&mut core, &manager, create.clone()).id, first.id);
        let mut changed = create;
        changed.input = json!({"text":"changed"});
        assert!(matches!(
            core.handle(
                "manager",
                request(Call::ScheduleCreate(changed), Some(manager.clone()))
            )
            .outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::IdempotencyConflict
            })
        ));
        let outcome = core
            .handle(
                "manager",
                request(
                    Call::ScheduleCancel(ScheduleKey {
                        id: first.id,
                        revision: 1,
                    }),
                    Some(manager),
                ),
            )
            .outcome;
        assert!(matches!(outcome, Outcome::Schedule(record) if record.cancelled));
        assert!(core.tick_schedules().unwrap().is_empty());
        drop(core);
        std::fs::remove_dir_all(profile).unwrap();
    }
}
