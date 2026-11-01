//! T0-only fixed media effect admission. No process or user file adapter.
use super::*;
use crate::{dependencies::PackagePin, packages::tools::media::MediaActionV1};

impl RuntimeCore {
    /// Enters a current broker effect with the exact accepted consumer/tool pin
    /// and a freshly checked held image. Default product Hosts have no authority.
    /// There is no executable/path/capability getter, IPC operation or SDK adapter.
    /// The callback may not defer an effect; async work needs another current
    /// check at its resume/commit point. Diagnostic pins are not permission.
    ///
    /// A borrowed context cannot escape the callback:
    /// ```compile_fail
    /// use flowtools_runtime_core::{runtime::RuntimeCore, packages::tools::media::MediaActionV1};
    /// fn leak(core: &mut RuntimeCore) {
    ///     let _ticket = core.invoke_media_tool("run", MediaActionV1::Inspect, |tool| Ok(tool));
    /// }
    /// ```
    pub fn invoke_media_tool<T>(
        &mut self,
        run_id: &str,
        action: MediaActionV1,
        adapter: impl FnOnce(&AdmittedMediaTool<'_>) -> Result<T, ErrorCode>,
    ) -> Result<T, ErrorCode> {
        let result = self.invoke_media_inner(run_id, action, adapter, |_| Ok(()));
        self.media_result(run_id, result)
    }
    #[cfg(test)]
    pub(crate) fn invoke_media_fixture_with_hook<T>(
        &mut self,
        run_id: &str,
        action: MediaActionV1,
        adapter: impl FnOnce(&AdmittedMediaTool<'_>) -> Result<T, ErrorCode>,
        after_artifacts: impl FnOnce(&mut Self) -> Result<(), ErrorCode>,
    ) -> Result<T, ErrorCode> {
        let result = self.invoke_media_inner(run_id, action, adapter, after_artifacts);
        self.media_result(run_id, result)
    }
    fn media_result<T>(
        &mut self,
        run_id: &str,
        result: Result<T, ErrorCode>,
    ) -> Result<T, ErrorCode> {
        if let Err(code) = &result {
            if let Some(call) = self.service_invocations.get_mut(run_id) {
                // Preserve ordinary service semantics: a running provider may
                // not ignore a rejected capability and later report success.
                if self
                    .services
                    .diagnostics(&call.root)
                    .iter()
                    .any(|diagnostic| diagnostic.run_id == run_id && diagnostic.state == "running")
                {
                    call.failure = Some(code.clone());
                }
            }
        }
        result
    }
    fn media_predecessor(&mut self, root: &str) -> Result<(), ErrorCode> {
        let expected = self
            .jobs
            .get(root)
            .ok_or(ErrorCode::JobNotFound)?
            .durable_metadata
            .borrow()
            .clone();
        let result = expected
            .ok_or(ErrorCode::StoreCorrupt)
            .and_then(|expected| leases::check_job_predecessor(&self.data, root, &expected));
        if result.is_err() {
            self.stopping = true;
        }
        result
    }
    fn invoke_media_inner<T>(
        &mut self,
        run_id: &str,
        action: MediaActionV1,
        adapter: impl FnOnce(&AdmittedMediaTool<'_>) -> Result<T, ErrorCode>,
        after_artifacts: impl FnOnce(&mut Self) -> Result<(), ErrorCode>,
    ) -> Result<T, ErrorCode> {
        self.admit_media(
            run_id,
            action,
            None,
            after_artifacts,
            |broker, session, identity, admitted| {
                broker.invoke(
                    session,
                    &leases::media_operation(action),
                    now(),
                    |current| {
                        if current != identity {
                            return Err(ErrorCode::GrantRevoked);
                        }
                        adapter(admitted)
                    },
                )
            },
        )
    }
    fn admit_media<T>(
        &mut self,
        run_id: &str,
        action: MediaActionV1,
        claim: Option<&crate::broker::ToolCallClaim>,
        after_artifacts: impl FnOnce(&mut Self) -> Result<(), ErrorCode>,
        adapter: impl FnOnce(
            &mut CapabilityBroker,
            &BrokerSession,
            &CommandIdentity,
            &AdmittedMediaTool<'_>,
        ) -> Result<T, ErrorCode>,
    ) -> Result<T, ErrorCode> {
        self.check_run(run_id)?;
        if self.management_only {
            return Err(ErrorCode::SessionInvalid);
        }
        let (root, session) = if let Some(call) = self.service_invocations.get(run_id) {
            if !self
                .services
                .diagnostics(&call.root)
                .iter()
                .any(|diagnostic| diagnostic.run_id == run_id && diagnostic.state == "running")
            {
                return Err(ErrorCode::SessionInvalid);
            }
            (call.root.clone(), call.session.clone())
        } else {
            (
                run_id.to_owned(),
                self.jobs[run_id]
                    .broker_session
                    .as_ref()
                    .ok_or(ErrorCode::SessionInvalid)?
                    .clone(),
            )
        };
        let operation = leases::media_operation(action);
        let identity = match claim {
            None => self.broker.check_effect(&session, &operation, now())?,
            Some(claim) => self
                .broker
                .invoke_claimed_tool(claim, now(), |identity| Ok(identity.clone()))?,
        };
        self.managed_authorization(&identity, self.jobs[&root].background)?;
        self.media_predecessor(&root)?;
        let snapshot = self.jobs[&root].snapshot.clone();
        let lock = self.jobs[&root]
            .dependency_lock
            .clone()
            .ok_or(ErrorCode::DependencyArtifactMismatch)?;
        let stored = self.jobs[&root]
            .tool_lease
            .as_ref()
            .ok_or(ErrorCode::DependencyArtifactMismatch)?
            .stored()
            .clone();
        let restored = self
            .tools
            .as_mut()
            .ok_or(ErrorCode::DependencyArtifactMismatch)?
            .restore(&mut self.data, &snapshot, &lock, &stored);
        // Conservative trust IO is allowed to advance its clock; it must not
        // hide an intervening change to this accepted task's durable record.
        self.media_predecessor(&root)?;
        self.jobs.get_mut(&root).unwrap().tool_lease = Some(restored?);
        after_artifacts(self)?;
        self.check_run(run_id)?;
        self.media_predecessor(&root)?;
        self.managed_authorization(&identity, self.jobs[&root].background)?;
        let consumer = PackagePin {
            publisher: identity.publisher.clone(),
            id: identity.plugin_id.clone(),
            version: identity.version.clone(),
            digest: identity.package_digest.clone(),
        };
        let admitted = self.jobs[&root]
            .tool_lease
            .as_ref()
            .unwrap()
            .media_tool(&lock, &consumer, action)?;
        adapter(&mut self.broker, &session, &identity, &admitted)
    }
}

#[path = "runtime_media/audio.rs"]
pub(super) mod audio;
#[path = "runtime_media/inspection.rs"]
pub(super) mod inspection;
pub use audio::{CompletedAudio, MediaAudioHandle, MediaAudioState};
pub use inspection::{MediaInspectionHandle, MediaInspectionState};
pub(super) use inspection::{NativeCall, StoredInspection};
