//! Actor-owned native inspection. All durable attempts use jobs.metadata's
//! existing writer and exact predecessor fence; no second writer or wire API.
use super::*;
use crate::{
    broker::ToolCallClaim,
    packages::tools::{
        install::images::inspection::{self, Inspection, Observation},
        media::MediaInspection,
    },
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// T0-only opaque actor reference. It has no public constructor, Clone or Serde.
pub struct MediaInspectionHandle {
    instance: String,
    id: String,
}
pub enum MediaInspectionState {
    Running,
    Complete(MediaInspection),
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Phase {
    Preparing,
    Running,
    Completed,
    Failed,
    Interrupted,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::runtime) struct StoredInspection {
    id: String,
    run: String,
    consumer: PackagePin,
    archive: String,
    input_bytes: u32,
    input_digest: String,
    deadline: f64,
    phase: Phase,
}
impl StoredInspection {
    pub(in crate::runtime) fn retains_artifacts(&self) -> bool {
        matches!(
            self.phase,
            Phase::Preparing | Phase::Running | Phase::Interrupted
        )
    }
    pub(in crate::runtime) fn interrupt(&mut self) {
        if matches!(self.phase, Phase::Preparing | Phase::Running) {
            self.phase = Phase::Interrupted;
        }
    }
}
pub(in crate::runtime) fn validate_stored(stored: &StoredJob) -> Result<(), ErrorCode> {
    if stored.native_inspections.is_empty() {
        return Ok(());
    }
    let lock = stored
        .dependency_lock
        .as_ref()
        .ok_or(ErrorCode::StoreCorrupt)?;
    let lease = stored
        .tool_artifacts
        .as_ref()
        .ok_or(ErrorCode::StoreCorrupt)?;
    let mut ids = BTreeSet::new();
    if stored.native_inspections.len() > 8 {
        return Err(ErrorCode::StoreCorrupt);
    }
    for call in &stored.native_inspections {
        let root = call.run == stored.snapshot.run_id;
        let consumer = if root {
            lock.packages.iter().find(|pin| {
                pin.id == stored.snapshot.plugin_id && pin.digest == stored.snapshot.package_digest
            })
        } else {
            stored
                .service_calls
                .iter()
                .find(|service| service.run_id == call.run)
                .map(|service| &service.provider)
        }
        .ok_or(ErrorCode::StoreCorrupt)?;
        if Uuid::parse_str(&call.id).is_err()
            || Uuid::parse_str(&call.id).unwrap().to_string() != call.id
            || !ids.insert(&call.id)
            || consumer != &call.consumer
            || !valid_digest(&call.archive)
            || !valid_digest(&call.input_digest)
            || call.input_bytes == 0
            || call.input_bytes > 1_048_576
            || !call.deadline.is_finite()
            || call.deadline <= 0.0
            || call.deadline > stored.snapshot.deadline
            || (!root
                && stored
                    .service_calls
                    .iter()
                    .any(|service| service.run_id == call.run && call.deadline > service.deadline))
            || !lock.tools.iter().any(|tool| {
                tool.consumer == call.consumer.identity()
                    && tool.id == "ffmpeg"
                    && tool.digest == call.archive
            })
            || (matches!(
                call.phase,
                Phase::Preparing | Phase::Running | Phase::Interrupted
            ) && !lease.retained())
        {
            return Err(ErrorCode::StoreCorrupt);
        }
    }
    Ok(())
}
pub(in crate::runtime) struct NativeCall {
    root: String,
    run: String,
    claim: ToolCallClaim,
    process: Option<Inspection>,
    outcome: Option<Result<MediaInspection, ErrorCode>>,
    #[cfg(test)]
    drained: Option<(u32, u32, bool, String, bool)>,
}
impl NativeCall {
    pub(in crate::runtime) fn holds_run(&self, run: &str) -> bool {
        self.run == run && self.process.is_some()
    }
    pub(in crate::runtime) fn holds_root(&self, root: &str) -> bool {
        self.root == root && self.process.is_some()
    }
}
impl RuntimeCore {
    #[cfg(test)]
    pub fn start_media_inspection_fixture(
        &mut self,
        run: &str,
        input: &[u8],
        after_prepare: impl FnOnce(&mut Self, (u32, u32, bool, String, bool)) -> Result<(), ErrorCode>,
    ) -> Result<MediaInspectionHandle, ErrorCode> {
        let result = self.start_inspection_inner(run, input, |core, inspection| {
            after_prepare(core, inspection.snapshot()?)
        });
        self.media_result(run, result)
    }
    #[cfg(test)]
    pub fn media_inspection_fixture_observations(
        &self,
        run: &str,
    ) -> Vec<(u32, u32, bool, String, bool)> {
        self.native_calls
            .values()
            .filter(|call| call.run == run)
            .filter_map(|call| {
                call.process
                    .as_ref()
                    .map(|process| process.snapshot().unwrap())
                    .or_else(|| call.drained.clone())
            })
            .collect()
    }
    #[cfg(test)]
    pub fn tick_media_inspections_fixture(
        &mut self,
        before_commit: impl FnMut(&mut Self),
    ) -> Result<(), ErrorCode> {
        self.tick_inspections_inner(before_commit)
    }
    /// Starts fixed ffprobe on finite T0 input bytes in a private read-only
    /// workspace. This API cannot select an executable, path, argv or tool.
    /// Ordinary Hosts have no tool authority; no client adapter is installed.
    pub fn start_media_inspection(
        &mut self,
        run: &str,
        input: &[u8],
    ) -> Result<MediaInspectionHandle, ErrorCode> {
        let result = self.start_inspection_inner(run, input, |_, _| Ok(()));
        self.media_result(run, result)
    }
    fn start_inspection_inner(
        &mut self,
        run: &str,
        input: &[u8],
        after_prepare: impl FnOnce(&mut Self, &Inspection) -> Result<(), ErrorCode>,
    ) -> Result<MediaInspectionHandle, ErrorCode> {
        if !inspection::valid_input(input) {
            return Err(ErrorCode::InputInvalid);
        }
        self.check_run(run)?;
        let root = self
            .service_invocations
            .get(run)
            .map_or(run, |call| call.root.as_str())
            .to_owned();
        let known = self.inspection_ids()?;
        if known.len() >= 64
            || self
                .native_calls
                .values()
                .filter(|call| call.process.is_some())
                .count()
                >= 4
            || self.jobs[&root].native_inspections.len() >= 8
            || self
                .native_calls
                .values()
                .any(|call| call.run == run && call.process.is_some())
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let claim = self.admit_media(
            run,
            MediaActionV1::Inspect,
            None,
            |_| Ok(()),
            |broker, session, _, _| {
                broker.claim_tool_call(
                    session,
                    &leases::media_operation(MediaActionV1::Inspect),
                    now(),
                )
            },
        )?;
        let identity = claim.identity();
        let consumer = PackagePin {
            publisher: identity.publisher.clone(),
            id: identity.plugin_id.clone(),
            version: identity.version.clone(),
            digest: identity.package_digest.clone(),
        };
        let id = Uuid::new_v4().to_string();
        let archive = self.jobs[&root]
            .dependency_lock
            .as_ref()
            .unwrap()
            .tools
            .iter()
            .find(|tool| tool.consumer == consumer.identity() && tool.id == "ffmpeg")
            .unwrap()
            .digest
            .clone();
        let deadline = self
            .service_invocations
            .get(run)
            .map_or(self.jobs[&root].snapshot.deadline, |call| {
                call.spec.deadline
            });
        let stored = StoredInspection {
            id: id.clone(),
            run: run.into(),
            consumer,
            archive,
            input_bytes: input.len() as u32,
            input_digest: format!("{:x}", Sha256::digest(input)),
            deadline,
            phase: Phase::Preparing,
        };
        self.media_predecessor(&root)?;
        self.jobs
            .get_mut(&root)
            .unwrap()
            .native_inspections
            .push(stored);
        self.persist_inspection(&root)?;
        let known = self.inspection_ids()?;
        let profile = self
            .data
            .profile()
            .ok_or(ErrorCode::StorageFailed)?
            .to_owned();
        let mut call = NativeCall {
            root: root.clone(),
            run: run.into(),
            claim,
            process: None,
            outcome: None,
            #[cfg(test)]
            drained: None,
        };
        let prepared = self.admit_media(
            run,
            MediaActionV1::Inspect,
            Some(&call.claim),
            |_| Ok(()),
            |broker, _, _, tool| {
                broker.invoke_claimed_tool(&call.claim, now(), |_| {
                    tool.prepare_inspection(&profile, &id, input, &known)
                })
            },
        );
        match prepared {
            Ok(process) => call.process = Some(process),
            Err(code) => {
                self.set_inspection_phase(&root, &id, Phase::Interrupted)?;
                // A failed native preparation may have created an owned OS
                // object whose final observation is unknown. Retain its roots
                // and refuse unrelated admission, even when Drop killed it.
                self.stopping = true;
                return Err(code);
            }
        }
        let resumed = (|| {
            after_prepare(self, call.process.as_ref().unwrap())?;
            self.set_inspection_phase(&root, &id, Phase::Running)?;
            self.admit_media(
                run,
                MediaActionV1::Inspect,
                Some(&call.claim),
                |_| Ok(()),
                |broker, _, _, _| {
                    broker.invoke_claimed_tool(&call.claim, now(), |_| {
                        call.process.as_mut().unwrap().resume()
                    })
                },
            )
        })();
        if let Err(code) = resumed {
            let stopped = self.end_inspection(&id, &mut call, Err(code.clone()));
            self.native_calls.insert(id, call);
            stopped?;
            return Err(code);
        }
        self.native_calls.insert(id.clone(), call);
        Ok(MediaInspectionHandle {
            instance: self.instance_id.clone(),
            id,
        })
    }
    fn inspection_ids(&self) -> Result<BTreeSet<String>, ErrorCode> {
        let mut ids = BTreeSet::new();
        for (value, committed) in self.data.load_job_records()? {
            let stored: StoredJob =
                serde_json::from_value(value).map_err(|_| ErrorCode::StoreCorrupt)?;
            stored.validate_tool_artifacts()?;
            let job = self
                .jobs
                .get(&stored.snapshot.run_id)
                .ok_or(ErrorCode::StoreCorrupt)?;
            if job.durable_metadata.borrow().as_deref() != Some(committed.as_str()) {
                return Err(ErrorCode::StoreCorrupt);
            }
            for call in &stored.native_inspections {
                if !ids.insert(call.id.clone()) {
                    return Err(ErrorCode::StoreCorrupt);
                }
            }
        }
        if ids.len() > 64 {
            return Err(ErrorCode::BudgetExceeded);
        }
        Ok(ids)
    }
    fn persist_inspection(&mut self, root: &str) -> Result<(), ErrorCode> {
        if let Err(code) = self.persist_job(&self.jobs[root]) {
            self.stopping = true;
            return Err(code);
        }
        Ok(())
    }
    fn set_inspection_phase(
        &mut self,
        root: &str,
        id: &str,
        phase: Phase,
    ) -> Result<(), ErrorCode> {
        self.media_predecessor(root)?;
        self.jobs
            .get_mut(root)
            .unwrap()
            .native_inspections
            .iter_mut()
            .find(|call| call.id == id)
            .ok_or(ErrorCode::StoreCorrupt)?
            .phase = phase;
        self.persist_inspection(root)
    }
    fn check_inspection(&mut self, call: &NativeCall) -> Result<(), ErrorCode> {
        self.check_run(&call.run)?;
        if self.management_only {
            return Err(ErrorCode::SessionInvalid);
        }
        self.broker
            .invoke_claimed_tool(&call.claim, now(), |_| Ok(()))?;
        self.managed_authorization(call.claim.identity(), self.jobs[&call.root].background)?;
        self.media_predecessor(&call.root)?;
        let job = &self.jobs[&call.root];
        self.tools
            .as_ref()
            .ok_or(ErrorCode::DependencyArtifactMismatch)?
            .check_active(
                &mut self.data,
                &job.snapshot,
                job.dependency_lock.as_ref().unwrap(),
                job.tool_lease.as_ref().unwrap(),
            )?;
        self.media_predecessor(&call.root)?;
        self.check_run(&call.run)?;
        self.broker
            .invoke_claimed_tool(&call.claim, now(), |_| Ok(()))?;
        self.managed_authorization(call.claim.identity(), self.jobs[&call.root].background)
    }
    fn end_inspection(
        &mut self,
        id: &str,
        call: &mut NativeCall,
        outcome: Result<MediaInspection, ErrorCode>,
    ) -> Result<(), ErrorCode> {
        if let Some(process) = call.process.as_mut() {
            if let Err(code) = process.cancel() {
                self.stopping = true;
                return Err(code);
            }
        }
        // Only positive owned process + Job drain permits dropping the physical
        // image/input guards. Unknown SQL then preserves durable artifact roots.
        #[cfg(test)]
        if let Some(process) = call.process.as_ref() {
            call.drained = Some(process.snapshot()?);
        }
        call.process.take();
        let phase = if outcome.is_ok() {
            Phase::Completed
        } else {
            Phase::Failed
        };
        self.set_inspection_phase(&call.root, id, phase)?;
        if let Err(code) = &outcome {
            let _: Result<(), _> = self.media_result(&call.run, Err(code.clone()));
        }
        call.outcome = Some(outcome);
        Ok(())
    }
    /// Bounded native maintenance; Hosts must call this while work is in flight.
    /// It never starts a tool or repeats a durable attempt.
    pub fn tick_media_inspections(&mut self) -> Result<(), ErrorCode> {
        self.tick_inspections_inner(|_| {})
    }
    /// T0 drain accounting only, with no process IDs or file handles.
    pub fn active_media_inspections(&self) -> usize {
        self.native_calls
            .values()
            .filter(|call| call.process.is_some())
            .count()
    }
    fn tick_inspections_inner(
        &mut self,
        mut before_commit: impl FnMut(&mut Self),
    ) -> Result<(), ErrorCode> {
        let ids: Vec<_> = self
            .native_calls
            .iter()
            .filter(|(_, call)| call.process.is_some())
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            let mut call = self.native_calls.remove(&id).unwrap();
            let observed = self
                .check_inspection(&call)
                .and_then(|()| call.process.as_mut().unwrap().poll());
            let ended = match observed {
                Ok(Observation::Running) => Ok(()),
                Ok(Observation::Complete(value)) => {
                    before_commit(self);
                    let committed = self.admit_media(
                        &call.run,
                        MediaActionV1::Inspect,
                        Some(&call.claim),
                        |_| Ok(()),
                        |broker, _, _, _| {
                            broker.invoke_claimed_tool(&call.claim, now(), |_| Ok(value))
                        },
                    );
                    self.end_inspection(&id, &mut call, committed)
                }
                Err(code) => self.end_inspection(&id, &mut call, Err(code)),
            };
            self.native_calls.insert(id, call);
            ended?;
        }
        Ok(())
    }
    /// Polls only this actor's opaque reference. Successful projections are
    /// transient and still require current authority when collected.
    pub fn poll_media_inspection(
        &mut self,
        handle: &MediaInspectionHandle,
    ) -> Result<MediaInspectionState, ErrorCode> {
        if handle.instance != self.instance_id {
            return Err(ErrorCode::InstanceMismatch);
        }
        self.tick_media_inspections()?;
        let call = self
            .native_calls
            .get(&handle.id)
            .ok_or(ErrorCode::SessionInvalid)?;
        if call.outcome.is_none() {
            return Ok(MediaInspectionState::Running);
        }
        let call = self.native_calls.remove(&handle.id).unwrap();
        if call.outcome.as_ref().unwrap().is_ok() {
            self.check_inspection(&call)?;
        }
        call.outcome.unwrap().map(MediaInspectionState::Complete)
    }
    pub(in crate::runtime) fn stop_native_inspections(
        &mut self,
        run: &str,
    ) -> Result<(), ErrorCode> {
        let ids: Vec<_> = self
            .native_calls
            .iter()
            .filter(|(_, call)| call.root == run || call.run == run)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            let mut call = self.native_calls.remove(&id).unwrap();
            if call.process.is_some() {
                let result = self.end_inspection(&id, &mut call, Err(ErrorCode::Aborted));
                self.native_calls.insert(id, call);
                result?;
            } else {
                self.native_calls.insert(id, call);
            }
        }
        Ok(())
    }
}
