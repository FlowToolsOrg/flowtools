//! Pure completion projection for the fixed conversion recipes. The native
//! adapter must separately prove owned-process drain, fresh output identity and
//! that ffprobe read that held output. These caller values prove none of those.
use super::{
    parse_inspection, MediaActionV1, MediaError, MediaInspection, ProgressDecoder, StreamKind,
    MAX_MEDIA_BYTES,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionError {
    ProcessFailed,
    ProgressInvalid,
    OutputInvalid,
    BudgetExceeded,
}
impl CompletionError {
    pub fn code(self) -> &'static str {
        match self {
            Self::ProcessFailed => "MEDIA_PROCESS_FAILED",
            Self::ProgressInvalid => "MEDIA_PROGRESS_INVALID",
            Self::OutputInvalid => "MEDIA_OUTPUT_INVALID",
            Self::BudgetExceeded => "MEDIA_BUDGET_EXCEEDED",
        }
    }
}
fn budget_or(error: MediaError, fallback: CompletionError) -> CompletionError {
    if error == MediaError::BudgetExceeded {
        CompletionError::BudgetExceeded
    } else {
        fallback
    }
}

/// Joins process exit, a complete bounded progress stream, and the bounded probe
/// projection of an output whose actual length T0 observed. This is metadata,
/// not a publication receipt, file handle, grant or executable identity.
pub fn project_completed_output(
    action: MediaActionV1,
    exit_code: u32,
    progress: &[u8],
    output_probe: &[u8],
    observed_output_bytes: u64,
) -> Result<MediaInspection, CompletionError> {
    if action == MediaActionV1::Inspect {
        return Err(CompletionError::OutputInvalid);
    }
    if exit_code != 0 {
        return Err(CompletionError::ProcessFailed);
    }
    if observed_output_bytes > MAX_MEDIA_BYTES {
        return Err(CompletionError::BudgetExceeded);
    }
    if observed_output_bytes == 0 {
        return Err(CompletionError::OutputInvalid);
    }
    let mut decoder = ProgressDecoder::default();
    decoder
        .push(progress)
        .map_err(|error| budget_or(error, CompletionError::ProgressInvalid))?;
    // A zero exit on this pinned FFmpeg includes -n refusal. A prior snapshot's
    // ended bit cannot hide an unfinished line or a poisoned later record.
    decoder
        .finish()
        .map_err(|error| budget_or(error, CompletionError::ProgressInvalid))?;
    let output = parse_inspection(output_probe)
        .map_err(|error| budget_or(error, CompletionError::OutputInvalid))?;
    if output.bytes != Some(observed_output_bytes)
        || output.duration_ms.is_none_or(|duration| duration == 0)
    {
        return Err(CompletionError::OutputInvalid);
    }
    if !matches_recipe_profile(action, &output) {
        return Err(CompletionError::OutputInvalid);
    }
    // Progress time/size remain advisory, including N/A. Actual output size and
    // media properties come from T0 observations and the separate probe result.
    Ok(output)
}
pub(super) fn matches_recipe_profile(action: MediaActionV1, output: &MediaInspection) -> bool {
    match action {
        MediaActionV1::ExtractAudioWav => {
            output.formats == ["wav"]
                && output.streams.len() == 1
                && output.streams[0].kind == StreamKind::Audio
                && output.streams[0].codec.as_deref() == Some("pcm_s16le")
                && output.streams[0].sample_rate.is_some()
                && output.streams[0].channels.is_some()
        }
        MediaActionV1::TranscodeWebm => {
            output.formats == ["matroska", "webm"]
                && (1..=2).contains(&output.streams.len())
                && output.streams[0].kind == StreamKind::Video
                && output.streams[0].codec.as_deref() == Some("vp9")
                && output.streams[0].dimensions.is_some()
                && output.streams.get(1).is_none_or(|stream| {
                    stream.kind == StreamKind::Audio
                        && stream.codec.as_deref() == Some("opus")
                        && stream.sample_rate.is_some()
                        && stream.channels.is_some()
                })
        }
        MediaActionV1::Inspect => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    const END: &[u8] = b"out_time_us=N/A\ntotal_size=N/A\nprogress=end\n";
    fn wave() -> Value {
        json!({"format":{"format_name":"wav","duration":"1.0","size":"1024"},
            "streams":[{"index":0,"codec_type":"audio","codec_name":"pcm_s16le","sample_rate":"16000","channels":1}]})
    }
    fn webm() -> Value {
        json!({"format":{"format_name":"matroska,webm","duration":"1.0","size":"1024"},
            "streams":[{"index":0,"codec_type":"video","codec_name":"vp9","width":16,"height":16},
                {"index":1,"codec_type":"audio","codec_name":"opus","sample_rate":"48000","channels":2}]})
    }
    fn check(action: MediaActionV1, value: &Value) -> Result<MediaInspection, CompletionError> {
        project_completed_output(action, 0, END, &serde_json::to_vec(value).unwrap(), 1024)
    }
    #[test]
    fn fixed_output_roles_accept_audio_and_video_with_or_without_the_optional_audio_stream() {
        assert_eq!(
            check(MediaActionV1::ExtractAudioWav, &wave())
                .unwrap()
                .bytes,
            Some(1024)
        );
        assert_eq!(
            check(MediaActionV1::TranscodeWebm, &webm())
                .unwrap()
                .streams
                .len(),
            2
        );
        let mut silent = webm();
        silent["streams"].as_array_mut().unwrap().pop();
        assert_eq!(
            check(MediaActionV1::TranscodeWebm, &silent)
                .unwrap()
                .streams
                .len(),
            1
        );
    }
    #[test]
    fn zero_exit_and_a_prior_end_snapshot_cannot_hide_missing_or_poisoned_complete_progress() {
        let probe = serde_json::to_vec(&wave()).unwrap();
        for progress in [
            b"".as_slice(),
            b"progress=continue\n",
            b"progress=end",
            b"progress=end\nx=1\n",
            b"progress=end\nunknown",
            b"out_time_us=1\nout_time_us=2\nprogress=end\n",
        ] {
            assert!(project_completed_output(
                MediaActionV1::ExtractAudioWav,
                0,
                progress,
                &probe,
                1024
            )
            .is_err());
        }
    }
    #[test]
    fn failed_exit_inspection_action_and_zero_or_over_budget_output_never_project_success() {
        let probe = serde_json::to_vec(&wave()).unwrap();
        assert_eq!(
            project_completed_output(MediaActionV1::ExtractAudioWav, 71, END, &probe, 1024)
                .unwrap_err(),
            CompletionError::ProcessFailed
        );
        assert_eq!(
            project_completed_output(MediaActionV1::Inspect, 0, END, &probe, 1024).unwrap_err(),
            CompletionError::OutputInvalid
        );
        for size in [0, MAX_MEDIA_BYTES + 1] {
            assert!(
                project_completed_output(MediaActionV1::ExtractAudioWav, 0, END, &probe, size)
                    .is_err()
            );
        }
    }
    #[test]
    fn stale_or_unknown_size_duration_codec_and_extra_streams_refuse_the_fixed_wave_output() {
        for (field, value) in [
            ("size", json!("1023")),
            ("size", Value::Null),
            ("duration", json!("0")),
            ("duration", Value::Null),
            ("format_name", json!("mp3")),
        ] {
            let mut probe = wave();
            probe["format"][field] = value;
            assert!(check(MediaActionV1::ExtractAudioWav, &probe).is_err());
        }
        for (field, value) in [
            ("codec_name", json!("mp3")),
            ("sample_rate", Value::Null),
            ("channels", Value::Null),
            ("codec_type", json!("video")),
        ] {
            let mut probe = wave();
            probe["streams"][0][field] = value;
            assert!(check(MediaActionV1::ExtractAudioWav, &probe).is_err());
        }
        let mut extra = wave();
        let stream = extra["streams"][0].clone();
        extra["streams"].as_array_mut().unwrap().push(stream);
        assert!(check(MediaActionV1::ExtractAudioWav, &extra).is_err());
    }
    #[test]
    fn wrong_webm_roles_order_dimensions_formats_and_unexpected_streams_are_refused() {
        for (field, value) in [
            ("codec_name", json!("h264")),
            ("width", Value::Null),
            ("height", Value::Null),
        ] {
            let mut probe = webm();
            probe["streams"][0][field] = value;
            assert!(check(MediaActionV1::TranscodeWebm, &probe).is_err());
        }
        let mut wrong = webm();
        wrong["streams"][1]["codec_name"] = json!("aac");
        assert!(check(MediaActionV1::TranscodeWebm, &wrong).is_err());
        let mut order = webm();
        order["streams"].as_array_mut().unwrap().swap(0, 1);
        assert!(check(MediaActionV1::TranscodeWebm, &order).is_err());
        let mut extra = webm();
        extra["streams"]
            .as_array_mut()
            .unwrap()
            .push(json!({"index":2,"codec_type":"attachment"}));
        assert!(check(MediaActionV1::TranscodeWebm, &extra).is_err());
        let mut format = webm();
        format["format"]["format_name"] = json!("matroska");
        assert!(check(MediaActionV1::TranscodeWebm, &format).is_err());
    }
    #[test]
    fn raw_names_tags_duplicate_keys_and_probe_or_progress_budgets_still_fail_closed() {
        let mut probe = wave();
        probe["format"]["filename"] = json!("PRIVATE_FILENAME_CANARY");
        assert!(check(MediaActionV1::ExtractAudioWav, &probe).is_err());
        probe = wave();
        probe["format"]["tags"] = json!({"secret":"PRIVATE_METADATA_CANARY"});
        assert!(check(MediaActionV1::ExtractAudioWav, &probe).is_err());
        let duplicate = b"{\"format\":{},\"format\":{},\"streams\":[]}";
        assert!(
            project_completed_output(MediaActionV1::ExtractAudioWav, 0, END, duplicate, 1024)
                .is_err()
        );
        let valid = serde_json::to_vec(&wave()).unwrap();
        assert_eq!(
            project_completed_output(
                MediaActionV1::ExtractAudioWav,
                0,
                &vec![b'x'; 65537],
                &valid,
                1024
            )
            .unwrap_err(),
            CompletionError::BudgetExceeded
        );
        assert_eq!(
            project_completed_output(
                MediaActionV1::ExtractAudioWav,
                0,
                END,
                &vec![b' '; 65537],
                1024
            )
            .unwrap_err(),
            CompletionError::BudgetExceeded
        );
    }
}
