//! Explicit T0 disposition of interrupted queues that provably never started.
//! No native attempt, execution grant, input deletion, image GC or client wire.
use super::*;

/// Instance-bound confirmation for exactly one unchanged interrupted queue row.
/// It is not an execution/rollback grant and cannot cross a restart or copy.
/// ```compile_fail
/// use flowtools_runtime_core::runtime::QueuedToolDiscardPlan;
/// fn wire<T: serde::Serialize>() {}
/// wire::<QueuedToolDiscardPlan>();
/// ```
pub struct QueuedToolDiscardPlan {
    instance: String,
    run: String,
    predecessor: String,
    authority: String,
    challenge: String,
}
impl QueuedToolDiscardPlan {
    pub fn challenge(&self) -> &str {
        &self.challenge
    }
}

impl RuntimeCore {
    fn discard_queue_predecessor(&mut self, run: &str, expected: &str) -> Result<(), ErrorCode> {
        let checked = leases::check_job_predecessor(&self.data, run, expected);
        if checked.is_err() {
            self.stopping = true;
        }
        checked
    }
    fn discard_queue_material(
        &mut self,
        run: &str,
    ) -> Result<(String, String, StoredJob), ErrorCode> {
        if self.stopping || self.management_only {
            return Err(ErrorCode::ExecutionInterrupted);
        }
        let job = self.jobs.get(run).ok_or(ErrorCode::JobNotFound)?;
        // A queued job can be interrupted before dispatch by a raised floor.
        // Anything that ever ran, called a provider or prepared native IO keeps
        // its original retention and needs its separate recovery/disposal path.
        if job.snapshot.state != JobState::Interrupted
            || !retains_tool_artifacts(job)
            || job.snapshot.result.is_none()
            || job.started_at != job.accepted_at
            || job.events.is_empty()
            || job.events.iter().enumerate().any(|(index, event)| {
                event.run_id != run
                    || event.sequence as usize != index + 1
                    || !matches!(event.state, JobState::Queued | JobState::Interrupted)
            })
            || job.events.last().is_none_or(|event| {
                event.state != JobState::Interrupted || event.sequence != job.snapshot.sequence
            })
            || job.broker_session.is_some()
            || self.pending.contains_key(run)
            || !job.service_calls.is_empty()
            || !job.native_inspections.is_empty()
            || !job.native_audio.is_empty()
            || !job.native_video.is_empty()
            || !self.services.root_drained(run)
            || self.native_calls.values().any(|call| call.holds_root(run))
            || self.audio_calls.values().any(|call| call.holds_root(run))
        {
            return Err(ErrorCode::InvalidRequest);
        }
        let stored = StoredJob::from_job(job);
        stored.validate_tool_artifacts()?;
        let predecessor = job
            .durable_metadata
            .borrow()
            .clone()
            .ok_or(ErrorCode::StoreCorrupt)?;
        self.discard_queue_predecessor(run, &predecessor)?;
        let before = leases::metadata_stamp(&self.data.connection)?;
        self.tools
            .as_ref()
            .ok_or(ErrorCode::DependencyArtifactMismatch)?
            .validate_retained(&mut self.data, stored.tool_artifacts.as_ref().unwrap())?;
        self.discard_queue_predecessor(run, &predecessor)?;
        if leases::metadata_stamp(&self.data.connection)? != before {
            return Err(ErrorCode::RevisionConflict);
        }
        Ok((predecessor, before, stored))
    }

    /// Read-only T0 proposal. Does not release roots or renew the old grant.
    pub fn plan_queued_tool_discard(
        &mut self,
        run: &str,
    ) -> Result<QueuedToolDiscardPlan, ErrorCode> {
        let (predecessor, authority, _) = self.discard_queue_material(run)?;
        let challenge = digest(&json!([
            "queued-tool-discard-v1",
            self.instance_id,
            run,
            predecessor,
            authority
        ]));
        Ok(QueuedToolDiscardPlan {
            instance: self.instance_id.clone(),
            run: run.into(),
            predecessor,
            authority,
            challenge,
        })
    }

    /// Confirms cancellation and root release in the existing guarded row.
    /// Private input bytes are retained here; ordinary confirmed-terminal
    /// cleanup handles them separately. No files/images are removed or replayed.
    pub fn discard_queued_tool_job(
        &mut self,
        plan: &QueuedToolDiscardPlan,
        confirmation: &str,
    ) -> Result<(), ErrorCode> {
        if plan.instance != self.instance_id {
            return Err(ErrorCode::InstanceMismatch);
        }
        if confirmation != plan.challenge {
            return Err(ErrorCode::RevisionConflict);
        }
        let (predecessor, authority, mut stored) = self.discard_queue_material(&plan.run)?;
        if predecessor != plan.predecessor || authority != plan.authority {
            return Err(ErrorCode::RevisionConflict);
        }
        stored.snapshot.state = JobState::Cancelled;
        stored.snapshot.sequence = stored
            .snapshot
            .sequence
            .checked_add(1)
            .ok_or(ErrorCode::BudgetExceeded)?;
        let result = stored.snapshot.result.as_mut().unwrap();
        result.finished_at = now().max(result.started_at);
        result.duration_ms = result.finished_at - result.started_at;
        result.outcome = ExecutionOutcome::Failure {
            success: false,
            error: RuntimeError {
                code: ErrorCode::Aborted,
            },
        };
        stored.events.push(JobEvent {
            run_id: plan.run.clone(),
            sequence: stored.snapshot.sequence,
            state: JobState::Cancelled,
        });
        stored.tool_artifacts = Some(stored.tool_artifacts.as_ref().unwrap().released_copy()?);
        stored.validate_tool_artifacts()?;
        if let Err(code) = persist_stored_job(
            &self.data,
            &stored,
            Some(&authority),
            &self.jobs[&plan.run].durable_metadata,
        ) {
            // Unknown SQL never drops the original held images/private input.
            self.stopping = true;
            return Err(code);
        }
        let job = self.jobs.get_mut(&plan.run).unwrap();
        job.snapshot = stored.snapshot;
        job.events = stored.events;
        job.tool_lease
            .as_mut()
            .unwrap()
            .apply_confirmed_release(stored.tool_artifacts.unwrap());
        Ok(())
    }
}
