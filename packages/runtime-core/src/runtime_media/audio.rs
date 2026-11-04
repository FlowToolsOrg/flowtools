//! Finite WAV/WebM conversion into bounded T0 memory, followed by an independent
//! fresh private-file ffprobe. No user output path or publication authority.
use super::*;
use crate::{
    broker::ToolCallClaim,
    packages::tools::{
        install::images::inspection::{
            self, ConversionProbe, Extraction, ExtractionObservation, ProbeObservation,
        },
        media::{self, MediaInspection},
    },
};
use sha2::{Digest, Sha256};
#[path = "audio/health.rs"]
pub(super) mod health;
#[path = "audio/progress.rs"]
pub(super) mod progress;
pub struct MediaAudioHandle {
    instance: String,
    id: String,
}
pub enum MediaAudioState {
    Running,
    Complete(CompletedAudio),
}
pub struct CompletedAudio {
    metadata: MediaInspection,
    bytes: Vec<u8>,
}
pub struct MediaVideoHandle {
    instance: String,
    id: String,
}
pub enum MediaVideoState {
    Running,
    Complete(CompletedVideo),
}
pub struct CompletedVideo {
    metadata: MediaInspection,
    bytes: Vec<u8>,
    frames: Vec<(u32, u32)>,
}
impl CompletedVideo {
    pub fn metadata(&self) -> &MediaInspection {
        &self.metadata
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn decoded_frames(&self) -> &[(u32, u32)] {
        &self.frames
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
struct CapturedMedia {
    metadata: MediaInspection,
    bytes: Vec<u8>,
    frames: Vec<(u32, u32)>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Audio,
    Video,
}
impl Kind {
    fn action(self) -> MediaActionV1 {
        match self {
            Self::Audio => MediaActionV1::ExtractAudioWav,
            Self::Video => MediaActionV1::TranscodeWebm,
        }
    }
    fn profile(self) -> &'static str {
        match self {
            Self::Audio => "native-audio-pipe-v1",
            Self::Video => "native-video-pipe-v1",
        }
    }
}
impl CompletedAudio {
    pub fn metadata(&self) -> &MediaInspection {
        &self.metadata
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Phase {
    Preparing,
    Extracting,
    PreparingProbe,
    Probing,
    Completed,
    Failed,
    Interrupted,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Output {
    bytes: u32,
    digest: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::runtime) struct StoredAudio {
    format_version: u16,
    profile: String,
    id: String,
    probe_id: String,
    run: String,
    consumer: PackagePin,
    archive: String,
    input_bytes: u32,
    input_digest: String,
    deadline: f64,
    phase: Phase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output: Option<Output>,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}
impl StoredAudio {
    pub(in crate::runtime) fn retains_artifacts(&self) -> bool {
        !matches!(self.phase, Phase::Completed | Phase::Failed)
    }
    pub(in crate::runtime) fn interrupt(&mut self) {
        if !matches!(
            self.phase,
            Phase::Completed | Phase::Failed | Phase::Interrupted
        ) {
            self.phase = Phase::Interrupted;
        }
    }
}
pub(super) fn input_ids(stored: &StoredJob) -> impl Iterator<Item = &str> {
    stored
        .native_audio
        .iter()
        .chain(stored.native_video.iter())
        .flat_map(|call| [call.id.as_str(), call.probe_id.as_str()])
}
pub(in crate::runtime) fn validate_stored(stored: &StoredJob) -> Result<(), ErrorCode> {
    if stored.native_audio.is_empty() && stored.native_video.is_empty() {
        return Ok(());
    }
    if stored.native_audio.len() + stored.native_video.len() + stored.native_inspections.len() > 8 {
        return Err(ErrorCode::StoreCorrupt);
    }
    let lock = stored
        .dependency_lock
        .as_ref()
        .ok_or(ErrorCode::StoreCorrupt)?;
    let lease = stored
        .tool_artifacts
        .as_ref()
        .ok_or(ErrorCode::StoreCorrupt)?;
    let mut ids: std::collections::BTreeSet<_> = stored
        .native_inspections
        .iter()
        .map(|call| call.id.as_str())
        .collect();
    for (kind, call) in stored
        .native_audio
        .iter()
        .map(|call| (Kind::Audio, call))
        .chain(stored.native_video.iter().map(|call| (Kind::Video, call)))
    {
        let consumer = if call.run == stored.snapshot.run_id {
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
        if call.format_version != 1
            || call.profile != kind.profile()
            || !uuid(&call.id)
            || !uuid(&call.probe_id)
            || !ids.insert(call.id.as_str())
            || !ids.insert(call.probe_id.as_str())
            || consumer != &call.consumer
            || !digest(&call.archive)
            || !digest(&call.input_digest)
            || call.input_bytes == 0
            || call.input_bytes > 1_048_576
            || !call.deadline.is_finite()
            || call.deadline <= 0.0
            || call.deadline > stored.snapshot.deadline
            || stored
                .service_calls
                .iter()
                .any(|service| service.run_id == call.run && call.deadline > service.deadline)
            || !lock.tools.iter().any(|tool| {
                tool.consumer == call.consumer.identity()
                    && tool.id == "ffmpeg"
                    && tool.digest == call.archive
            })
            || call.output.as_ref().is_some_and(|output| {
                output.bytes == 0 || output.bytes > 1_048_576 || !digest(&output.digest)
            })
            || (matches!(
                call.phase,
                Phase::PreparingProbe | Phase::Probing | Phase::Completed
            ) && call.output.is_none())
            || (call.retains_artifacts() && !lease.retained())
        {
            return Err(ErrorCode::StoreCorrupt);
        }
    }
    Ok(())
}
enum Process {
    Extract(Extraction),
    Probe {
        process: ConversionProbe,
        bytes: Vec<u8>,
        progress: Vec<u8>,
    },
}
impl Process {
    fn cancel(&mut self) -> Result<(), ErrorCode> {
        match self {
            Self::Extract(process) => process.cancel(),
            Self::Probe { process, .. } => process.cancel(),
        }
    }
    fn resume(&mut self) -> Result<(), ErrorCode> {
        match self {
            Self::Extract(process) => process.resume(),
            Self::Probe { process, .. } => process.resume(),
        }
    }
    #[cfg(test)]
    fn snapshot(&self) -> Result<(u32, u32, bool, String, bool), ErrorCode> {
        match self {
            Self::Extract(process) => process.snapshot(),
            Self::Probe { process, .. } => process.snapshot(),
        }
    }
}
pub(in crate::runtime) struct AudioCall {
    id: String,
    continuation: Option<leases::LiveToolCall>,
    kind: Kind,
    root: String,
    run: String,
    claim: ToolCallClaim,
    process: Option<Process>,
    outcome: Option<Result<CapturedMedia, ErrorCode>>,
    progress: media::MediaProgress,
    #[cfg(test)]
    observations: Vec<(u32, u32, bool, String, bool)>,
}
impl AudioCall {
    pub(in crate::runtime) fn holds_root(&self, run: &str) -> bool {
        self.root == run && self.process.is_some()
    }
    pub(in crate::runtime) fn holds_run(&self, run: &str) -> bool {
        self.run == run && self.process.is_some()
    }
}
impl RuntimeCore {
    #[cfg(test)]
    pub fn start_media_audio_fixture(
        &mut self,
        run: &str,
        input: &[u8],
        after_prepare: impl FnOnce(&mut Self, (u32, u32, bool, String, bool)) -> Result<(), ErrorCode>,
    ) -> Result<MediaAudioHandle, ErrorCode> {
        let result = self
            .start_audio_inner(run, input, Kind::Audio, |core, process| {
                after_prepare(core, process.snapshot()?)
            })
            .map(|id| MediaAudioHandle {
                instance: self.instance_id.clone(),
                id,
            });
        self.media_result(run, result)
    }
    #[cfg(test)]
    pub fn media_audio_fixture_observations(
        &self,
        run: &str,
    ) -> Vec<(u32, u32, bool, String, bool)> {
        self.audio_calls
            .values()
            .filter(|call| call.run == run)
            .flat_map(|call| {
                let mut values = call.observations.clone();
                if let Some(process) = &call.process {
                    values.push(process.snapshot().unwrap());
                }
                values
            })
            .collect()
    }
    #[cfg(test)]
    pub fn tick_media_audio_fixture(
        &mut self,
        mut after_probe_prepare: impl FnMut(
            &mut Self,
            (u32, u32, bool, String, bool),
        ) -> Result<(), ErrorCode>,
        mut before_commit: impl FnMut(&mut Self, (u32, u32, bool, String, bool)),
    ) -> Result<(), ErrorCode> {
        self.tick_audio_inner(
            |core, process| after_probe_prepare(core, process.snapshot()?),
            |core, process| before_commit(core, process.snapshot().unwrap()),
        )
    }
    pub fn start_media_audio(
        &mut self,
        run: &str,
        input: &[u8],
    ) -> Result<MediaAudioHandle, ErrorCode> {
        let result = self
            .start_audio_inner(run, input, Kind::Audio, |_, _| Ok(()))
            .map(|id| MediaAudioHandle {
                instance: self.instance_id.clone(),
                id,
            });
        self.media_result(run, result)
    }
    pub fn start_media_video(
        &mut self,
        run: &str,
        input: &[u8],
    ) -> Result<MediaVideoHandle, ErrorCode> {
        let result = self
            .start_audio_inner(run, input, Kind::Video, |_, _| Ok(()))
            .map(|id| MediaVideoHandle {
                instance: self.instance_id.clone(),
                id,
            });
        self.media_result(run, result)
    }
    #[cfg(test)]
    pub fn start_media_video_fixture(
        &mut self,
        run: &str,
        input: &[u8],
        after_prepare: impl FnOnce(&mut Self, (u32, u32, bool, String, bool)) -> Result<(), ErrorCode>,
    ) -> Result<MediaVideoHandle, ErrorCode> {
        let result = self
            .start_audio_inner(run, input, Kind::Video, |core, process| {
                after_prepare(core, process.snapshot()?)
            })
            .map(|id| MediaVideoHandle {
                instance: self.instance_id.clone(),
                id,
            });
        self.media_result(run, result)
    }
    fn start_audio_inner(
        &mut self,
        run: &str,
        input: &[u8],
        kind: Kind,
        after_prepare: impl FnOnce(&mut Self, &Process) -> Result<(), ErrorCode>,
    ) -> Result<String, ErrorCode> {
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
        let job = &self.jobs[&root];
        if known.len() > 62
            || self.active_media_inspections() >= 4
            || job.native_inspections.len() + job.native_audio.len() + job.native_video.len() >= 8
            || self.native_calls.values().any(|call| call.holds_run(run))
            || self.audio_calls.values().any(|call| call.holds_run(run))
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let claim = self.admit_media(
            run,
            kind.action(),
            None,
            None,
            |_| Ok(()),
            |broker, session, _, _, _store| {
                broker.claim_tool_call(session, &leases::media_operation(kind.action()), now())
            },
        )?;
        let identity = claim.identity();
        let consumer = PackagePin {
            publisher: identity.publisher.clone(),
            id: identity.plugin_id.clone(),
            version: identity.version.clone(),
            digest: identity.package_digest.clone(),
        };
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
        let id = Uuid::new_v4().to_string();
        let probe_id = Uuid::new_v4().to_string();
        let deadline = self
            .service_invocations
            .get(run)
            .map_or(self.jobs[&root].snapshot.deadline, |call| {
                call.spec.deadline
            });
        let stored = StoredAudio {
            format_version: 1,
            profile: kind.profile().into(),
            id: id.clone(),
            probe_id,
            run: run.into(),
            consumer,
            archive,
            input_bytes: input.len() as u32,
            input_digest: hash(input),
            deadline,
            phase: Phase::Preparing,
            output: None,
        };
        self.media_predecessor(&root)?;
        match kind {
            Kind::Audio => self.jobs.get_mut(&root).unwrap().native_audio.push(stored),
            Kind::Video => self.jobs.get_mut(&root).unwrap().native_video.push(stored),
        }
        self.persist_audio(&root)?;
        let known = self.inspection_ids()?;
        let profile = self
            .data
            .profile()
            .ok_or(ErrorCode::StorageFailed)?
            .to_owned();
        let mut call = AudioCall {
            id: id.clone(),
            continuation: None,
            kind,
            root: root.clone(),
            run: run.into(),
            claim,
            process: None,
            outcome: None,
            progress: media::MediaProgress::default(),
            #[cfg(test)]
            observations: Vec::new(),
        };
        let mut entered = false;
        let prepared = self.admit_media(
            run,
            kind.action(),
            Some(&call.claim),
            None,
            |_| Ok(()),
            |broker, _, _, tool, _store| {
                broker.invoke_claimed_tool(&call.claim, now(), |_| {
                    entered = true;
                    tool.prepare_conversion(&profile, &id, input, &known)
                })
            },
        );
        match prepared {
            Ok(process) => call.process = Some(Process::Extract(process)),
            Err(code) => {
                self.audio_phase(
                    &root,
                    &id,
                    if entered {
                        Phase::Interrupted
                    } else {
                        Phase::Failed
                    },
                )?;
                if entered {
                    self.stopping = true;
                }
                return Err(code);
            }
        }
        let resumed = (|| {
            call.continuation =
                Some(self.begin_live_media(&root, run, &id, &call.claim, kind.action())?);
            after_prepare(self, call.process.as_ref().unwrap())?;
            self.audio_phase(&root, &id, Phase::Extracting)?;
            self.admit_media(
                run,
                kind.action(),
                Some(&call.claim),
                Some((
                    call.continuation
                        .as_ref()
                        .ok_or(ErrorCode::SessionInvalid)?,
                    &call.id,
                )),
                |_| Ok(()),
                |broker, _, _, _, _store| {
                    broker.invoke_claimed_tool(&call.claim, now(), |_| {
                        call.process.as_mut().unwrap().resume()
                    })
                },
            )
        })();
        if let Err(code) = resumed {
            let stopped = self.end_audio(&id, &mut call, Err(code.clone()));
            self.audio_calls.insert(id, call);
            stopped?;
            return Err(code);
        }
        self.audio_calls.insert(id.clone(), call);
        Ok(id)
    }
    fn persist_audio(&mut self, root: &str) -> Result<(), ErrorCode> {
        if let Err(code) = self.persist_job(&self.jobs[root]) {
            self.stopping = true;
            return Err(code);
        }
        Ok(())
    }
    fn audio_phase(&mut self, root: &str, id: &str, phase: Phase) -> Result<(), ErrorCode> {
        self.media_predecessor(root)?;
        let job = self.jobs.get_mut(root).unwrap();
        job.native_audio
            .iter_mut()
            .chain(job.native_video.iter_mut())
            .find(|call| call.id == id)
            .ok_or(ErrorCode::StoreCorrupt)?
            .phase = phase;
        self.persist_audio(root)
    }
    fn check_audio(&mut self, call: &AudioCall) -> Result<(), ErrorCode> {
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
            .check_live_call(
                &mut self.data,
                &job.snapshot,
                job.dependency_lock.as_ref().unwrap(),
                job.tool_lease.as_ref().unwrap(),
                call.continuation
                    .as_ref()
                    .ok_or(ErrorCode::SessionInvalid)?,
                &leases::LiveCallBinding {
                    id: &call.id,
                    run: &call.run,
                    consumer: &Self::media_consumer(call.claim.identity()),
                    action: call.kind.action(),
                },
            )?;
        self.media_predecessor(&call.root)?;
        self.check_run(&call.run)?;
        self.broker
            .invoke_claimed_tool(&call.claim, now(), |_| Ok(()))?;
        self.managed_authorization(call.claim.identity(), self.jobs[&call.root].background)
    }
    fn drop_audio_process(call: &mut AudioCall) -> Result<(), ErrorCode> {
        if let Some(process) = call.process.as_mut() {
            process.cancel()?;
            #[cfg(test)]
            call.observations.push(process.snapshot()?);
        }
        call.process.take();
        Ok(())
    }
    fn end_audio(
        &mut self,
        id: &str,
        call: &mut AudioCall,
        outcome: Result<CapturedMedia, ErrorCode>,
    ) -> Result<(), ErrorCode> {
        if let Err(code) = Self::drop_audio_process(call) {
            self.stopping = true;
            return Err(code);
        }
        self.audio_phase(
            &call.root,
            id,
            if outcome.is_ok() {
                Phase::Completed
            } else {
                Phase::Failed
            },
        )?;
        if let Err(code) = &outcome {
            let _: Result<(), _> = self.media_result(&call.run, Err(code.clone()));
        }
        call.outcome = Some(outcome);
        Ok(())
    }
    fn begin_audio_probe(
        &mut self,
        id: &str,
        call: &mut AudioCall,
        bytes: Vec<u8>,
        progress: Vec<u8>,
        after_prepare: &mut impl FnMut(&mut Self, &Process) -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        Self::drop_audio_process(call)?;
        let mut decoder = media::ProgressDecoder::default();
        call.progress = decoder
            .push(&progress)
            .and_then(|_| decoder.finish())
            .map_err(|_| ErrorCode::OutputInvalid)?;
        self.media_predecessor(&call.root)?;
        let job = self.jobs.get_mut(&call.root).unwrap();
        let stored = job
            .native_audio
            .iter_mut()
            .chain(job.native_video.iter_mut())
            .find(|call| call.id == id)
            .ok_or(ErrorCode::StoreCorrupt)?;
        stored.phase = Phase::PreparingProbe;
        stored.output = Some(Output {
            bytes: bytes.len() as u32,
            digest: hash(&bytes),
        });
        let probe_id = stored.probe_id.clone();
        self.persist_audio(&call.root)?;
        let known = self.inspection_ids()?;
        let profile = self
            .data
            .profile()
            .ok_or(ErrorCode::StorageFailed)?
            .to_owned();
        let mut entered = false;
        let prepared = self.admit_media(
            &call.run,
            call.kind.action(),
            Some(&call.claim),
            Some((
                call.continuation
                    .as_ref()
                    .ok_or(ErrorCode::SessionInvalid)?,
                &call.id,
            )),
            |_| Ok(()),
            |broker, _, _, tool, _store| {
                broker.invoke_claimed_tool(&call.claim, now(), |_| {
                    entered = true;
                    tool.prepare_conversion_probe(&profile, &probe_id, &bytes, &known)
                })
            },
        );
        let process = match prepared {
            Ok(process) => process,
            Err(code) => {
                if entered {
                    self.audio_phase(&call.root, id, Phase::Interrupted)?;
                    self.stopping = true;
                    return Err(code);
                }
                return self.end_audio(id, call, Err(code));
            }
        };
        call.process = Some(Process::Probe {
            process,
            bytes,
            progress,
        });
        if let Err(code) = after_prepare(self, call.process.as_ref().unwrap()) {
            return self.end_audio(id, call, Err(code));
        }
        self.audio_phase(&call.root, id, Phase::Probing)?;
        let resumed = self.admit_media(
            &call.run,
            call.kind.action(),
            Some(&call.claim),
            Some((
                call.continuation
                    .as_ref()
                    .ok_or(ErrorCode::SessionInvalid)?,
                &call.id,
            )),
            |_| Ok(()),
            |broker, _, _, _, _store| {
                broker.invoke_claimed_tool(&call.claim, now(), |_| {
                    call.process.as_mut().unwrap().resume()
                })
            },
        );
        if let Err(code) = resumed {
            return self.end_audio(id, call, Err(code));
        }
        Ok(())
    }
    pub(super) fn active_audio_calls(&self) -> usize {
        self.audio_calls
            .values()
            .filter(|call| call.process.is_some())
            .count()
    }
    pub(super) fn tick_audio_calls(&mut self) -> Result<(), ErrorCode> {
        self.tick_audio_inner(|_, _| Ok(()), |_, _| {})
    }
    fn tick_audio_inner(
        &mut self,
        mut after_prepare: impl FnMut(&mut Self, &Process) -> Result<(), ErrorCode>,
        mut before_commit: impl FnMut(&mut Self, &Process),
    ) -> Result<(), ErrorCode> {
        let ids: Vec<_> = self
            .audio_calls
            .iter()
            .filter(|(_, call)| call.process.is_some())
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            let mut call = self.audio_calls.remove(&id).unwrap();
            let checked = self.check_audio(&call);
            let result = match checked {
                Err(code) => self.end_audio(&id, &mut call, Err(code)),
                Ok(()) => match call.process.as_mut().unwrap() {
                    Process::Extract(process) => match process.poll() {
                        Ok(ExtractionObservation::Running) => Ok(()),
                        Ok(ExtractionObservation::Complete { bytes, progress }) => self
                            .begin_audio_probe(&id, &mut call, bytes, progress, &mut after_prepare),
                        Err(code) => self.end_audio(&id, &mut call, Err(code)),
                    },
                    Process::Probe {
                        process,
                        bytes,
                        progress,
                    } => match process.poll() {
                        Ok(ProbeObservation::Running) => Ok(()),
                        Ok(ProbeObservation::Complete(raw)) => {
                            let output = match call.kind {
                                Kind::Audio => media::completion::project_completed_output(
                                    call.kind.action(),
                                    0,
                                    progress,
                                    &raw,
                                    bytes.len() as u64,
                                )
                                .map(|metadata| CapturedMedia {
                                    metadata,
                                    bytes: std::mem::take(bytes),
                                    frames: Vec::new(),
                                }),
                                Kind::Video => media::streaming::project_completed_webm_stream(
                                    0,
                                    progress,
                                    &raw,
                                    bytes.len() as u64,
                                )
                                .map(|result| {
                                    let frames = result.decoded_frames().to_vec();
                                    CapturedMedia {
                                        metadata: result.into_metadata(),
                                        frames,
                                        bytes: std::mem::take(bytes),
                                    }
                                }),
                            }
                            .map_err(|_| ErrorCode::OutputInvalid);
                            before_commit(self, call.process.as_ref().unwrap());
                            let committed = self.admit_media(
                                &call.run,
                                call.kind.action(),
                                Some(&call.claim),
                                Some((
                                    call.continuation
                                        .as_ref()
                                        .ok_or(ErrorCode::SessionInvalid)?,
                                    &call.id,
                                )),
                                |_| Ok(()),
                                |broker, _, _, _, _store| {
                                    broker.invoke_claimed_tool(&call.claim, now(), |_| output)
                                },
                            );
                            self.end_audio(&id, &mut call, committed)
                        }
                        Err(code) => self.end_audio(&id, &mut call, Err(code)),
                    },
                },
            };
            if result.is_err() {
                self.stopping = true;
                let _ = Self::drop_audio_process(&mut call);
            }
            self.audio_calls.insert(id, call);
            result?;
        }
        Ok(())
    }
    pub fn poll_media_audio(
        &mut self,
        handle: &MediaAudioHandle,
    ) -> Result<MediaAudioState, ErrorCode> {
        if handle.instance != self.instance_id {
            return Err(ErrorCode::InstanceMismatch);
        }
        self.tick_media_inspections()?;
        let call = self
            .audio_calls
            .get(&handle.id)
            .ok_or(ErrorCode::SessionInvalid)?;
        if call.kind != Kind::Audio {
            return Err(ErrorCode::SessionInvalid);
        }
        if call.outcome.is_none() {
            return Ok(MediaAudioState::Running);
        }
        let call = self.audio_calls.remove(&handle.id).unwrap();
        if call.outcome.as_ref().unwrap().is_ok() {
            self.check_audio(&call)?;
        }
        call.outcome.unwrap().map(|result| {
            MediaAudioState::Complete(CompletedAudio {
                metadata: result.metadata,
                bytes: result.bytes,
            })
        })
    }
    pub fn poll_media_video(
        &mut self,
        handle: &MediaVideoHandle,
    ) -> Result<MediaVideoState, ErrorCode> {
        if handle.instance != self.instance_id {
            return Err(ErrorCode::InstanceMismatch);
        }
        self.tick_media_inspections()?;
        let call = self
            .audio_calls
            .get(&handle.id)
            .ok_or(ErrorCode::SessionInvalid)?;
        if call.kind != Kind::Video {
            return Err(ErrorCode::SessionInvalid);
        }
        if call.outcome.is_none() {
            return Ok(MediaVideoState::Running);
        }
        let call = self.audio_calls.remove(&handle.id).unwrap();
        if call.outcome.as_ref().unwrap().is_ok() {
            self.check_audio(&call)?;
        }
        call.outcome.unwrap().map(|result| {
            MediaVideoState::Complete(CompletedVideo {
                metadata: result.metadata,
                bytes: result.bytes,
                frames: result.frames,
            })
        })
    }
    pub(in crate::runtime) fn stop_audio_calls(&mut self, run: &str) -> Result<(), ErrorCode> {
        let ids: Vec<_> = self
            .audio_calls
            .iter()
            .filter(|(_, call)| call.root == run || call.run == run)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            let mut call = self.audio_calls.remove(&id).unwrap();
            let stopped = if call.process.is_some() {
                self.end_audio(&id, &mut call, Err(ErrorCode::Aborted))
            } else {
                Ok(())
            };
            self.audio_calls.insert(id, call);
            if stopped.is_err() {
                self.stopping = true;
            }
            stopped?;
        }
        Ok(())
    }
}
