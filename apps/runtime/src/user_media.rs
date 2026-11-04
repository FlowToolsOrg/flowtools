//! Async fixed media adapter. The ordinary Core owns each native process/Job,
//! accepted tool pin, current authorization, independent probe and drain.
use super::*;
use flowtools_runtime_core::{
    file_workflows::media_io::UserMediaAction,
    packages::tools::media::MediaInspection,
    runtime::{
        MediaAudioHandle, MediaAudioState, MediaConversionStage, MediaInspectionHandle,
        MediaInspectionState, MediaVideoHandle, MediaVideoState,
    },
};

enum Handle {
    Inspect(MediaInspectionHandle),
    Audio(MediaAudioHandle),
    Video(MediaVideoHandle),
}
pub(super) async fn execute(core: &Core, run: &str, plan: &str) -> Result<Value, ErrorCode> {
    let handle = {
        let mut c = core.lock().await;
        let (action, input) = c.read_user_media(run, plan)?;
        match action {
            UserMediaAction::Inspect => {
                c.report_user_media_stage(run, "inspecting")?;
                Handle::Inspect(c.start_media_inspection(run, &input)?)
            }
            UserMediaAction::Audio => Handle::Audio(c.start_media_audio(run, &input)?),
            UserMediaAction::Webm => Handle::Video(c.start_media_video(run, &input)?),
        }
    };
    loop {
        let completed = {
            let mut c = core.lock().await;
            c.check_run(run)?;
            let progress = match &handle {
                Handle::Audio(h) => Some(c.media_audio_progress(h)?),
                Handle::Video(h) => Some(c.media_video_progress(h)?),
                Handle::Inspect(_) => None,
            };
            if let Some(p) = progress {
                c.report_user_media_stage(
                    run,
                    match p.stage {
                        MediaConversionStage::Converting => "converting",
                        MediaConversionStage::Validating | MediaConversionStage::Complete => {
                            "validating"
                        }
                    },
                )?;
            }
            match &handle {
                Handle::Inspect(h) => match c.poll_media_inspection(h)? {
                    MediaInspectionState::Running => None,
                    MediaInspectionState::Complete(m) => Some((m, None)),
                },
                Handle::Audio(h) => match c.poll_media_audio(h)? {
                    MediaAudioState::Running => None,
                    MediaAudioState::Complete(output) => {
                        Some((output.metadata().clone(), Some(output.into_bytes())))
                    }
                },
                Handle::Video(h) => match c.poll_media_video(h)? {
                    MediaVideoState::Running => None,
                    MediaVideoState::Complete(output) => {
                        Some((output.metadata().clone(), Some(output.into_bytes())))
                    }
                },
            }
        };
        if let Some((metadata, bytes)) = completed {
            let mut c = core.lock().await;
            c.check_run(run)?;
            if let Some(bytes) = &bytes {
                c.report_user_media_stage(run, "publishing")?;
                c.publish_user_media(run, plan, bytes)?;
            }
            return Ok(summary(metadata, bytes.as_deref()));
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
fn summary(m: MediaInspection, bytes: Option<&[u8]>) -> Value {
    json!({"formats":m.formats,"durationMs":m.duration_ms,"sourceBytes":m.bytes,
        "streams":m.streams.iter().map(|s|json!({"kind":s.kind,"codec":s.codec,"width":s.dimensions.map(|d|d[0]),"height":s.dimensions.map(|d|d[1]),"sampleRate":s.sample_rate,"channels":s.channels})).collect::<Vec<_>>(),
        "published":bytes.is_some(),"outputBytes":bytes.map(|b|b.len()),"outputSha256":bytes.map(|b|format!("{:x}",Sha256::digest(b)))})
}
