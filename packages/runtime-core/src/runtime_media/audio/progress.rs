//! Current-authorized metadata observation; no maintenance, raw pipe or output collection.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaConversionStage {
    Converting,
    Validating,
    Complete,
}
/// Advisory FFmpeg time/size is never duration, percent, publication or Job exit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaConversionProgress {
    pub stage: MediaConversionStage,
    pub advisory: media::MediaProgress,
}
impl RuntimeCore {
    /// T0 opaque-handle observation. Ordinary Host maintenance advances the actor.
    pub fn media_audio_progress(
        &mut self,
        handle: &MediaAudioHandle,
    ) -> Result<MediaConversionProgress, ErrorCode> {
        self.conversion_progress(&handle.instance, &handle.id, Kind::Audio)
    }
    pub fn media_video_progress(
        &mut self,
        handle: &MediaVideoHandle,
    ) -> Result<MediaConversionProgress, ErrorCode> {
        self.conversion_progress(&handle.instance, &handle.id, Kind::Video)
    }
    fn conversion_progress(
        &mut self,
        instance: &str,
        id: &str,
        kind: Kind,
    ) -> Result<MediaConversionProgress, ErrorCode> {
        if instance != self.instance_id {
            return Err(ErrorCode::InstanceMismatch);
        }
        let call = self.audio_calls.get(id).ok_or(ErrorCode::SessionInvalid)?;
        if call.kind != kind {
            return Err(ErrorCode::SessionInvalid);
        }
        let mut call = self.audio_calls.remove(id).unwrap();
        let result = (|| {
            if let Some(Err(code)) = &call.outcome {
                return Err(code.clone());
            }
            self.check_audio(&call)?;
            match &call.process {
                Some(Process::Extract(process)) => Ok(MediaConversionProgress {
                    stage: MediaConversionStage::Converting,
                    advisory: process.progress()?,
                }),
                Some(Process::Probe { .. }) => Ok(MediaConversionProgress {
                    stage: MediaConversionStage::Validating,
                    advisory: call.progress.clone(),
                }),
                None if matches!(&call.outcome, Some(Ok(_))) => Ok(MediaConversionProgress {
                    stage: MediaConversionStage::Complete,
                    advisory: call.progress.clone(),
                }),
                None => Err(ErrorCode::ExecutionInterrupted),
            }
        })();
        let result = if let Err(code) = &result {
            if call.process.is_some() {
                // A refused current observation cannot leave an unobserved native
                // process running. Unknown drain/SQL retains the original actor.
                if let Err(drain) = self.end_audio(id, &mut call, Err(code.clone())) {
                    self.stopping = true;
                    Err(drain)
                } else {
                    result
                }
            } else {
                result
            }
        } else {
            result
        };
        let run = call.run.clone();
        self.audio_calls.insert(id.into(), call);
        self.media_result(&run, result)
    }
}
