//! Borrowed T0 artifact admission within a current broker effect callback.
use super::*;
use crate::{
    broker::CapabilityOperation,
    dependencies::PackagePin,
    packages::tools::{
        media::{self, MediaActionV1},
        ToolProgram,
    },
};

/// This borrowed artifact context is usable only within RuntimeCore's current
/// broker callback. It has no executable/path/capability getter or Serde/Clone.
/// The diagnostic pins below are not grants or deferred permission tickets.
pub struct AdmittedMediaTool<'a> {
    action: MediaActionV1,
    consumer: &'a PackagePin,
    tool: &'a ToolPin,
    _held: &'a HeldImage,
}
impl AdmittedMediaTool<'_> {
    pub fn action(&self) -> MediaActionV1 {
        self.action
    }
    pub fn program(&self) -> ToolProgram {
        media::recipe(self.action).program()
    }
    pub fn consumer(&self) -> &PackagePin {
        self.consumer
    }
    pub fn tool(&self) -> &ToolPin {
        self.tool
    }
}
// A stable declaration/grant slot, not the semantic lock digest (which includes
// the manifest). T0 independently resolves it against this run's exact accepted
// lock and held image below. Neither IPC nor a caller selects this mapping.
pub(crate) fn media_operation(action: MediaActionV1) -> CapabilityOperation {
    CapabilityOperation::ToolExecute {
        lock: "ffmpeg".into(),
        action: match action {
            MediaActionV1::Inspect => "inspect-v1",
            MediaActionV1::ExtractAudioWav => "extract-audio-wav-v1",
            MediaActionV1::TranscodeWebm => "transcode-webm-v1",
        }
        .into(),
    }
}
impl ToolLease {
    pub(crate) fn media_tool<'a>(
        &'a self,
        lock: &'a DependencyLock,
        consumer: &'a PackagePin,
        action: MediaActionV1,
    ) -> std::result::Result<AdmittedMediaTool<'a>, ErrorCode> {
        if !self.stored.retained
            || self.stored.dependency_lock != lock.digest
            || !lock.packages.contains(consumer)
        {
            return Err(ErrorCode::DependencyArtifactMismatch);
        }
        let mut matching = lock
            .tools
            .iter()
            .filter(|tool| tool.consumer == consumer.identity() && tool.id == "ffmpeg");
        let tool = matching
            .next()
            .ok_or(ErrorCode::DependencyArtifactMismatch)?;
        if matching.next().is_some() {
            return Err(ErrorCode::DependencyArtifactMismatch);
        }
        let held = self
            ._held
            .iter()
            .find(|held| image_matches(tool, held.reference()))
            .ok_or(ErrorCode::DependencyArtifactMismatch)?;
        Ok(AdmittedMediaTool {
            action,
            consumer,
            tool,
            _held: held,
        })
    }
}
