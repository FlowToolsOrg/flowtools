//! T0-owned finite ffprobe inspection. No client path, argv or executable API.
mod capture;
mod kernel;
mod profile;
mod token;
mod workspace;
use super::lease_source::HeldImage;
use crate::{
    packages::tools::media::{self, MediaInspection},
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
    } else if error.kind() == std::io::ErrorKind::FileTooLarge {
        ErrorCode::BudgetExceeded
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
        prepare_process(source, profile, id, bytes, known, profile::Profile::Inspect)
            .map(|process| Self { process })
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
            kernel::Poll::Exited {
                code: 0, stdout, ..
            } => media::parse_inspection(&stdout)
                .map(Observation::Complete)
                .map_err(|_| ErrorCode::OutputInvalid),
            kernel::Poll::Exited { .. } => Err(ErrorCode::OutputInvalid),
        }
    }
}
fn prepare_process(
    source: Arc<HeldImage>,
    profile: &Path,
    id: &str,
    bytes: &[u8],
    known: &BTreeSet<String>,
    mode: profile::Profile,
) -> Result<kernel::Process, ErrorCode> {
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
        .get(&mode.program())
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
    kernel::Process::prepare(
        mode,
        &image,
        retention.workspace.path(),
        &retention.container,
        &artifact,
        retention.clone(),
    )
    .map_err(failed)
}
pub(crate) struct Extraction {
    process: kernel::Process,
}
pub(crate) enum ExtractionObservation {
    Running,
    Complete { bytes: Vec<u8>, progress: Vec<u8> },
}
impl Extraction {
    pub(crate) fn progress(&self) -> Result<media::MediaProgress, ErrorCode> {
        self.process.progress().map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidData {
                ErrorCode::OutputInvalid
            } else {
                failed(error)
            }
        })
    }
    pub(crate) fn prepare(
        source: Arc<HeldImage>,
        profile: &Path,
        id: &str,
        bytes: &[u8],
        known: &BTreeSet<String>,
        action: media::MediaActionV1,
    ) -> Result<Self, ErrorCode> {
        let mode = match action {
            media::MediaActionV1::ExtractAudioWav => profile::Profile::ExtractAudio,
            media::MediaActionV1::TranscodeWebm => profile::Profile::TranscodeWebm,
            _ => return Err(ErrorCode::CapabilityUndeclared),
        };
        prepare_process(source, profile, id, bytes, known, mode).map(|process| Self { process })
    }
    pub(crate) fn resume(&mut self) -> Result<(), ErrorCode> {
        self.process.resume().map_err(failed)
    }
    pub(crate) fn cancel(&mut self) -> Result<(), ErrorCode> {
        self.process.cancel().map_err(failed)
    }
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Result<(u32, u32, bool, String, bool), ErrorCode> {
        self.process.snapshot().map_err(failed)
    }
    pub(crate) fn poll(&mut self) -> Result<ExtractionObservation, ErrorCode> {
        match self.process.poll().map_err(failed)? {
            kernel::Poll::Running => Ok(ExtractionObservation::Running),
            kernel::Poll::Exited {
                code: 0,
                stdout,
                stderr,
            } => {
                let mut decoder = media::ProgressDecoder::default();
                decoder
                    .push(&stderr)
                    .and_then(|_| decoder.finish())
                    .map_err(|_| ErrorCode::OutputInvalid)?;
                if stdout.is_empty() {
                    return Err(ErrorCode::OutputInvalid);
                }
                Ok(ExtractionObservation::Complete {
                    bytes: stdout,
                    progress: stderr,
                })
            }
            kernel::Poll::Exited { .. } => Err(ErrorCode::OutputInvalid),
        }
    }
}
pub(crate) struct ConversionProbe {
    process: kernel::Process,
    reject_stderr: bool,
}
pub(crate) enum ProbeObservation {
    Running,
    Complete(Vec<u8>),
}
impl ConversionProbe {
    pub(crate) fn prepare(
        source: Arc<HeldImage>,
        profile: &Path,
        id: &str,
        bytes: &[u8],
        known: &BTreeSet<String>,
        action: media::MediaActionV1,
    ) -> Result<Self, ErrorCode> {
        let mode = match action {
            media::MediaActionV1::ExtractAudioWav => profile::Profile::Inspect,
            media::MediaActionV1::TranscodeWebm => profile::Profile::InspectDecodedWebm,
            _ => return Err(ErrorCode::CapabilityUndeclared),
        };
        prepare_process(source, profile, id, bytes, known, mode).map(|process| Self {
            process,
            reject_stderr: action == media::MediaActionV1::TranscodeWebm,
        })
    }
    pub(crate) fn resume(&mut self) -> Result<(), ErrorCode> {
        self.process.resume().map_err(failed)
    }
    pub(crate) fn cancel(&mut self) -> Result<(), ErrorCode> {
        self.process.cancel().map_err(failed)
    }
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Result<(u32, u32, bool, String, bool), ErrorCode> {
        self.process.snapshot().map_err(failed)
    }
    pub(crate) fn poll(&mut self) -> Result<ProbeObservation, ErrorCode> {
        match self.process.poll().map_err(failed)? {
            kernel::Poll::Running => Ok(ProbeObservation::Running),
            kernel::Poll::Exited {
                code: 0,
                stdout,
                stderr,
            } if !self.reject_stderr || stderr.is_empty() => Ok(ProbeObservation::Complete(stdout)),
            kernel::Poll::Exited { .. } => Err(ErrorCode::OutputInvalid),
        }
    }
}
