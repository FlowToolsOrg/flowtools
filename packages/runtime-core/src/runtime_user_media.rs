//! Fixed T0 bridge from explicit local media consent to ordinary authorized jobs.
use super::*;
use crate::file_workflows::media_io::{self, StartMediaPlan, UserMediaAction};

impl RuntimeCore {
    pub(super) fn start_user_media(
        &mut self,
        connection: &str,
        key: StartMediaPlan,
    ) -> Result<JobReceipt, ErrorCode> {
        if self.management_only {
            return Err(ErrorCode::ColdStartDenied);
        }
        let plan = media_io::get(&self.data, &key.id)?;
        if plan.digest != key.digest {
            return Err(ErrorCode::RevisionConflict);
        }
        let caller = key.target.caller();
        if let Some(run) = media_io::bound_run(&self.data, &key.id)? {
            if !self.jobs.contains_key(&run) {
                return Err(ErrorCode::StoreCorrupt);
            }
            return Ok(self.receipt(&run));
        }
        if let Some(run) = self.keys.get(&(caller.into(), key.id.clone())).cloned() {
            media_io::bind(&mut self.data, &key.id, &run)?;
            return Ok(self.receipt(&run));
        }
        let id = plan.action.plugin();
        if self
            .jobs
            .values()
            .any(|j| j.snapshot.plugin_id == id && !j.snapshot.state.terminal())
        {
            return Err(ErrorCode::RuntimeBusy);
        }
        let (manifest, command) = self.catalog.command(id, "run")?;
        let mut scopes = vec![
            Scope::FileHandle(media_io::input_handle(&key.id)),
            Scope::ToolLock {
                lock: "ffmpeg".into(),
                action: plan.action.operation().into(),
            },
        ];
        if plan.action != UserMediaAction::Inspect {
            scopes.push(Scope::FileHandle(media_io::output_handle(&key.id)));
        }
        let permission = PermissionGrant {
            plugin_id: id.into(),
            command_id: "run".into(),
            target: key.target,
            package_digest: digest(manifest),
            effects: command["effects"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e.as_str().unwrap().into())
                .collect(),
            scopes,
            expires_at: now() + 60000.0,
            max_calls: 8,
            cold_start: true,
            background: true,
        };
        let record = self
            .policy
            .as_mut()
            .ok_or(ErrorCode::ApprovalRequired)?
            .grant(&mut self.data, permission)?;
        self.broker.restore_policy(
            record.identity.clone(),
            record.epoch,
            record.grant.as_ref().map(PolicyStore::capability),
        )?;
        let receipt = self.submit(
            connection,
            caller,
            SubmitJob {
                plugin_id: id.into(),
                command_id: "run".into(),
                input: json!({"planId":key.id,"planDigest":key.digest}),
                idempotency_key: key.id.clone(),
                background: true,
                deadline: now() + 29900.0,
            },
        )?;
        media_io::bind(&mut self.data, &key.id, &receipt.run_id)?;
        Ok(receipt)
    }
    fn user_media_session(
        &mut self,
        run: &str,
        id: &str,
    ) -> Result<(BrokerSession, UserMediaAction), ErrorCode> {
        self.check_run(run)?;
        let plan = media_io::get(&self.data, id)?;
        let job = self.jobs.get(run).ok_or(ErrorCode::JobNotFound)?;
        let input = self
            .private_jobs
            .as_ref()
            .ok_or(ErrorCode::StorageFailed)?
            .read(run, "input")?;
        if job.snapshot.plugin_id != plan.action.plugin()
            || input != json!({"planId":id,"planDigest":plan.digest})
        {
            return Err(ErrorCode::ScopeDenied);
        }
        Ok((
            job.broker_session
                .clone()
                .ok_or(ErrorCode::SessionInvalid)?,
            plan.action,
        ))
    }
    pub fn read_user_media(
        &mut self,
        run: &str,
        id: &str,
    ) -> Result<(UserMediaAction, Vec<u8>), ErrorCode> {
        let (session, action) = self.user_media_session(run, id)?;
        let bytes = self.broker.invoke(
            &session,
            &CapabilityOperation::FileRead {
                handle: media_io::input_handle(id),
            },
            now(),
            |_| media_io::capture(&self.data, id, run),
        )?;
        Ok((action, bytes))
    }
    pub fn publish_user_media(
        &mut self,
        run: &str,
        id: &str,
        bytes: &[u8],
    ) -> Result<(), ErrorCode> {
        let (session, action) = self.user_media_session(run, id)?;
        if action == UserMediaAction::Inspect {
            return Err(ErrorCode::ScopeDenied);
        }
        let action = if action == UserMediaAction::Audio {
            crate::packages::tools::media::MediaActionV1::ExtractAudioWav
        } else {
            crate::packages::tools::media::MediaActionV1::TranscodeWebm
        };
        let predecessor = self.jobs[run]
            .durable_metadata
            .borrow()
            .clone()
            .ok_or(ErrorCode::StoreCorrupt)?;
        let result = self.admit_media(
            run,
            action,
            None,
            None,
            |_| Ok(()),
            |broker, _, _, _, store| {
                let operation = CapabilityOperation::FileCreate {
                    handle: media_io::output_handle(id),
                };
                broker.invoke(&session, &operation, now(), |_| Ok(()))?;
                let stamp = leases::metadata_stamp(&store.connection)?;
                media_io::publish(store, id, run, bytes, |store| {
                    broker.check_effect(&session, &operation, now())?;
                    leases::check_job_predecessor(store, run, &predecessor)?;
                    if leases::metadata_stamp(&store.connection)? != stamp {
                        return Err(ErrorCode::RevisionConflict);
                    }
                    Ok(())
                })
            },
        );
        if matches!(
            result,
            Err(ErrorCode::StorageFailed | ErrorCode::StoreCorrupt | ErrorCode::AcceptanceUnknown)
        ) {
            self.fail_closed();
        }
        result
    }
    pub fn report_user_media_stage(&mut self, run: &str, stage: &str) -> Result<(), ErrorCode> {
        self.check_run(run)?;
        if !matches!(
            stage,
            "inspecting" | "converting" | "validating" | "publishing"
        ) {
            return Err(ErrorCode::InvalidRequest);
        }
        self.user_media_stages.insert(run.into(), stage.into());
        Ok(())
    }
}
