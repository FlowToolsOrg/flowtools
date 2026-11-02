//! Fixed synthetic round trip through the ordinary current authorized actor.
//! This observation never activates a version or grants a later tool effect.
use super::*;
use crate::packages::tools::{install::ToolArchivePin, media::StreamKind};

pub struct ToolHealthHandle {
    instance: String,
    id: String,
}
pub enum ToolHealthState {
    Running,
    Healthy(ToolHealthObservation),
}
/// Diagnostic health at collection, never install, rollback or execution consent.
/// ```compile_fail
/// use flowtools_runtime_core::runtime::ToolHealthObservation;
/// let forged = ToolHealthObservation { pin: todo!(), checked_at_ms: 0.0 };
/// ```
pub struct ToolHealthObservation {
    pin: ToolArchivePin,
    checked_at_ms: f64,
}
impl ToolHealthObservation {
    pub fn pin(&self) -> &ToolArchivePin {
        &self.pin
    }
    pub fn checked_at_ms(&self) -> f64 {
        self.checked_at_ms
    }
}

const SAMPLES: usize = 16_000;
fn pcm() -> Vec<u8> {
    (0..SAMPLES)
        .flat_map(|n| (((n % 257) as i32 - 128) as i16 * 128).to_le_bytes())
        .collect()
}
fn wave() -> Vec<u8> {
    let data = pcm();
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    bytes.extend_from_slice(&16_000u32.to_le_bytes());
    bytes.extend_from_slice(&32_000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    bytes
}
fn healthy(output: &CapturedMedia) -> bool {
    let metadata = &output.metadata;
    if metadata.formats != ["wav"]
        || metadata.duration_ms != Some(1000)
        || metadata.bytes != Some(output.bytes.len() as u64)
        || metadata.streams.len() != 1
        || !output.frames.is_empty()
    {
        return false;
    }
    let stream = &metadata.streams[0];
    if stream.index != 0
        || stream.kind != StreamKind::Audio
        || stream.codec.as_deref() != Some("pcm_s16le")
        || stream.dimensions.is_some()
        || stream.sample_rate != Some(16_000)
        || stream.channels != Some(1)
    {
        return false;
    }
    let bytes = &output.bytes;
    if bytes.len() < 44
        || bytes.len() > 1_048_576
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
    {
        return false;
    }
    let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if size != u32::MAX && size as usize != bytes.len() - 8 {
        return false;
    }
    let mut position = 12usize;
    let input = wave();
    let mut format = false;
    let mut data = false;
    let mut chunks = 0;
    while position < bytes.len() {
        if bytes.len() - position < 8 || chunks >= 8 {
            return false;
        }
        chunks += 1;
        let kind = &bytes[position..position + 4];
        let size = u32::from_le_bytes(bytes[position + 4..position + 8].try_into().unwrap());
        position += 8;
        let length = if kind == b"data" && size == u32::MAX {
            bytes.len() - position
        } else {
            size as usize
        };
        let Some(end) = position.checked_add(length).filter(|n| *n <= bytes.len()) else {
            return false;
        };
        match kind {
            b"fmt " if !format && !data => {
                if ![16, 18].contains(&length)
                    || bytes[position..position + 16] != input[20..36]
                    || (length == 18 && bytes[position + 16..end] != [0, 0])
                {
                    return false;
                }
                format = true;
            }
            b"data" if format && !data => {
                if bytes[position..end] != input[44..] || end != bytes.len() {
                    return false;
                }
                data = true;
            }
            b"LIST" | b"JUNK" if !data && length <= 1024 => {}
            _ => return false,
        }
        position = end;
        if !length.is_multiple_of(2) {
            if bytes.get(position) != Some(&0) {
                return false;
            }
            position += 1;
        }
    }
    format && data
}

impl RuntimeCore {
    /// T0 fixed input only, sharing the normal WAV call, claims, quotas and drain.
    /// The accepted command must already authorize the ordinary WAV operation.
    pub fn start_tool_health(&mut self, run: &str) -> Result<ToolHealthHandle, ErrorCode> {
        let handle = self.start_media_audio(run, &wave())?;
        Ok(ToolHealthHandle {
            instance: handle.instance,
            id: handle.id,
        })
    }
    pub fn poll_tool_health(
        &mut self,
        handle: &ToolHealthHandle,
    ) -> Result<ToolHealthState, ErrorCode> {
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
            return Ok(ToolHealthState::Running);
        }
        let call = self.audio_calls.remove(&handle.id).unwrap();
        let result = (|| {
            call.outcome
                .as_ref()
                .unwrap()
                .as_ref()
                .map_err(Clone::clone)?;
            self.check_audio(&call)?;
            if !healthy(call.outcome.as_ref().unwrap().as_ref().unwrap()) {
                return Err(ErrorCode::OutputInvalid);
            }
            // An old in-flight conversion may drain across a floor advance. It
            // must never certify health of the currently selected release.
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
            self.broker
                .invoke_claimed_tool(&call.claim, now(), |_| Ok(()))?;
            self.managed_authorization(call.claim.identity(), self.jobs[&call.root].background)?;
            let record = self.jobs[&call.root]
                .native_audio
                .iter()
                .find(|record| record.id == call.id)
                .ok_or(ErrorCode::StoreCorrupt)?;
            let input = wave();
            if record.input_bytes != input.len() as u32 || record.input_digest != hash(&input) {
                return Err(ErrorCode::StoreCorrupt);
            }
            let pin = self.jobs[&call.root]
                .tool_lease
                .as_ref()
                .unwrap()
                .stored()
                .references()
                .iter()
                .find(|r| r.pin().archive_digest == record.archive)
                .ok_or(ErrorCode::DependencyArtifactMismatch)?
                .pin()
                .clone();
            Ok(ToolHealthState::Healthy(ToolHealthObservation {
                pin,
                checked_at_ms: now(),
            }))
        })();
        self.media_result(&call.run, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn output() -> CapturedMedia {
        let bytes = wave();
        CapturedMedia {
            metadata: MediaInspection {
                formats: vec!["wav".into()],
                duration_ms: Some(1000),
                bytes: Some(bytes.len() as u64),
                streams: vec![media::MediaStream {
                    index: 0,
                    kind: StreamKind::Audio,
                    codec: Some("pcm_s16le".into()),
                    dimensions: None,
                    sample_rate: Some(16_000),
                    channels: Some(1),
                }],
            },
            bytes,
            frames: vec![],
        }
    }
    #[test]
    fn health_requires_exact_non_silent_pcm_and_independent_metadata() {
        assert!(healthy(&output()));
        let mut changed = output();
        changed.bytes[44] ^= 1;
        assert!(!healthy(&changed));
        let mut changed = output();
        changed.metadata.duration_ms = None;
        assert!(!healthy(&changed));
        let mut changed = output();
        changed.metadata.streams[0].channels = Some(2);
        assert!(!healthy(&changed));
        let mut changed = output();
        changed.metadata.bytes = None;
        assert!(!healthy(&changed));
    }
    #[test]
    fn health_accepts_the_fixed_pipe_unknown_sizes_but_refuses_ambiguous_or_extra_data() {
        let mut pipe = output();
        pipe.bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        pipe.bytes[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(healthy(&pipe));
        let mut changed = output();
        changed.bytes.extend_from_slice(b"JUNK\0\0\0\0");
        changed.metadata.bytes = Some(changed.bytes.len() as u64);
        assert!(!healthy(&changed));
        let mut changed = output();
        changed.bytes[12..16].copy_from_slice(b"JUNK");
        assert!(!healthy(&changed));
        let mut changed = output();
        changed.bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(!healthy(&changed));
    }
}
