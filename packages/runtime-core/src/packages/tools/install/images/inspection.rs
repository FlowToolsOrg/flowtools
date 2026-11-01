//! T0-owned finite ffprobe inspection. No client path, argv or executable API.
mod kernel;
mod token;
mod workspace;
use super::lease_source::HeldImage;
use crate::{
    packages::tools::{
        media::{self, MediaInspection},
        ToolProgram,
    },
    protocol::ErrorCode,
};
use std::{collections::BTreeSet, path::Path, sync::Arc};

struct Retention {
    _image: Arc<HeldImage>,
    workspace: workspace::Workspace,
    container: kernel::Container,
}
pub(crate) struct Inspection {
    process: kernel::Process,
}
pub(crate) enum Observation {
    Running,
    Complete(MediaInspection),
}
pub(crate) fn valid_input(bytes: &[u8]) -> bool {
    !bytes.is_empty() && bytes.len() <= workspace::MAX_INPUT
}
fn failed(error: std::io::Error) -> ErrorCode {
    if error.kind() == std::io::ErrorKind::TimedOut {
        ErrorCode::Timeout
    } else {
        ErrorCode::ExecutionInterrupted
    }
}
impl Inspection {
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Result<(u32, u32, bool, String, bool), ErrorCode> {
        self.process.snapshot().map_err(failed)
    }
    pub(crate) fn prepare(
        source: Arc<HeldImage>,
        profile: &Path,
        id: &str,
        bytes: &[u8],
        known: &BTreeSet<String>,
    ) -> Result<Self, ErrorCode> {
        if !valid_input(bytes) {
            return Err(ErrorCode::InputInvalid);
        }
        let container = kernel::Container::new(&source.profile_digest, id).map_err(failed)?;
        let workspace = workspace::Workspace::create(profile, id, container.text(), bytes, known)
            .map_err(|error| match error {
            crate::packages::install::InstallError::RecoveryPending => ErrorCode::RecoveryPending,
            crate::packages::install::InstallError::BudgetExceeded => ErrorCode::BudgetExceeded,
            _ => ErrorCode::StorageFailed,
        })?;
        let program = source
            .programs
            .get(&ToolProgram::Ffprobe)
            .ok_or(ErrorCode::DependencyArtifactMismatch)?;
        let image = source
            ._image
            ._layout
            .root
            .join("artifacts")
            .join(&source.reference().pin().archive_digest)
            .join(program);
        let artifact = format!(
            "FlowTools.ToolImages.v1.{}.{}",
            source.profile_digest,
            source.reference().pin().archive_digest
        );
        let retention = Arc::new(Retention {
            _image: source,
            workspace,
            container,
        });
        let process = kernel::Process::prepare(
            &image,
            retention.workspace.path(),
            &retention.container,
            &artifact,
            retention.clone(),
        )
        .map_err(failed)?;
        Ok(Self { process })
    }
    pub(crate) fn resume(&mut self) -> Result<(), ErrorCode> {
        self.process.resume().map_err(failed)
    }
    pub(crate) fn cancel(&mut self) -> Result<(), ErrorCode> {
        self.process.cancel().map_err(failed)
    }
    pub(crate) fn poll(&mut self) -> Result<Observation, ErrorCode> {
        match self.process.poll().map_err(failed)? {
            kernel::Poll::Running => Ok(Observation::Running),
            kernel::Poll::Exited { code: 0, stdout } => media::parse_inspection(&stdout)
                .map(Observation::Complete)
                .map_err(|_| ErrorCode::OutputInvalid),
            kernel::Poll::Exited { .. } => Err(ErrorCode::OutputInvalid),
        }
    }
}
