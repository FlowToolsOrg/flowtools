//! Finite metadata-only completion for a nonseekable WebM pipe. Duration may
//! remain unknown; every stream must have independently decoded frames. This
//! creates no process/file/lease/grant proof and does not relax V1 completion.
use super::{
    completion::{matches_recipe_profile, CompletionError},
    parse_inspection, MediaActionV1, MediaError, MediaInspection, ProgressDecoder,
    MAX_INSPECTION_BYTES,
};
use std::collections::BTreeMap;
pub const MAX_STREAM_BYTES: u64 = 1_048_576;
pub const MAX_DECODED_FRAMES: u32 = 1_000_000;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamingCompletion {
    metadata: MediaInspection,
    frames: Vec<(u32, u32)>,
}
impl StreamingCompletion {
    pub fn metadata(&self) -> &MediaInspection {
        &self.metadata
    }
    pub fn decoded_frames(&self) -> &[(u32, u32)] {
        &self.frames
    }
    pub fn into_metadata(self) -> MediaInspection {
        self.metadata
    }
}
fn failed(error: MediaError) -> CompletionError {
    if error == MediaError::BudgetExceeded {
        CompletionError::BudgetExceeded
    } else {
        CompletionError::OutputInvalid
    }
}
pub fn project_completed_webm_stream(
    exit_code: u32,
    progress: &[u8],
    probe: &[u8],
    observed_bytes: u64,
) -> Result<StreamingCompletion, CompletionError> {
    if exit_code != 0 {
        return Err(CompletionError::ProcessFailed);
    }
    if observed_bytes > MAX_STREAM_BYTES || probe.len() > MAX_INSPECTION_BYTES {
        return Err(CompletionError::BudgetExceeded);
    }
    if observed_bytes == 0 {
        return Err(CompletionError::OutputInvalid);
    }
    let mut decoder = ProgressDecoder::default();
    decoder
        .push(progress)
        .and_then(|_| decoder.finish())
        .map_err(|e| {
            if e == MediaError::BudgetExceeded {
                CompletionError::BudgetExceeded
            } else {
                CompletionError::ProgressInvalid
            }
        })?;
    // Check duplicate keys before removing the one additional known field.
    let mut value =
        super::super::super::strict_json(probe).map_err(|_| CompletionError::OutputInvalid)?;
    let streams = value
        .get_mut("streams")
        .and_then(|v| v.as_array_mut())
        .ok_or(CompletionError::OutputInvalid)?;
    if streams.is_empty() || streams.len() > 32 {
        return Err(CompletionError::BudgetExceeded);
    }
    let mut frames = BTreeMap::new();
    for stream in streams {
        let object = stream
            .as_object_mut()
            .ok_or(CompletionError::OutputInvalid)?;
        let index = object
            .get("index")
            .and_then(|v| v.as_u64())
            .filter(|v| *v < 32)
            .ok_or(CompletionError::OutputInvalid)? as u32;
        let raw = object
            .remove("nb_read_frames")
            .ok_or(CompletionError::OutputInvalid)?;
        let raw = raw.as_str().ok_or(CompletionError::OutputInvalid)?;
        if raw.is_empty()
            || raw.starts_with('0')
            || raw.len() > 7
            || !raw.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(CompletionError::OutputInvalid);
        }
        let count = raw
            .parse::<u32>()
            .map_err(|_| CompletionError::OutputInvalid)?;
        if count > MAX_DECODED_FRAMES {
            return Err(CompletionError::BudgetExceeded);
        }
        if frames.insert(index, count).is_some() {
            return Err(CompletionError::OutputInvalid);
        }
    }
    let metadata =
        parse_inspection(&serde_json::to_vec(&value).map_err(|_| CompletionError::OutputInvalid)?)
            .map_err(failed)?;
    if metadata.bytes != Some(observed_bytes)
        || metadata.duration_ms == Some(0)
        || !matches_recipe_profile(MediaActionV1::TranscodeWebm, &metadata)
    {
        return Err(CompletionError::OutputInvalid);
    }
    let frames = metadata
        .streams
        .iter()
        .map(|stream| {
            frames
                .get(&stream.index)
                .copied()
                .map(|count| (stream.index, count))
                .ok_or(CompletionError::OutputInvalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StreamingCompletion { metadata, frames })
}
// Private fixed profiles consumed by the native adapter, never caller argv.
#[cfg(any(windows, test))]
pub(crate) fn webm_arguments() -> &'static [&'static str] {
    &[
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "quiet",
        "-nostats",
        "-n",
        "-progress",
        "pipe:2",
        "-protocol_whitelist",
        "file",
        "-i",
        "input",
        "-map",
        "0:v:0",
        "-map",
        "0:a:0?",
        "-map_metadata",
        "-1",
        "-map_chapters",
        "-1",
        "-c:v",
        "libvpx-vp9",
        "-b:v",
        "1M",
        "-threads",
        "1",
        "-c:a",
        "libopus",
        "-b:a",
        "96k",
        "-f",
        "webm",
        "pipe:1",
    ]
}
#[cfg(any(windows, test))]
pub(crate) fn probe_arguments() -> &'static [&'static str] {
    &[
    "-v","error","-protocol_whitelist","file","-count_frames","-show_entries",
    "format=format_name,duration,size:stream=index,codec_type,codec_name,width,height,sample_rate,channels,nb_read_frames",
    "-of","json","-i","input",
]
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    const END: &[u8] = b"out_time_us=N/A\ntotal_size=N/A\nprogress=end\n";
    fn video() -> Value {
        json!({"format":{"format_name":"matroska,webm","duration":"N/A","size":"1024"},"streams":[{"index":0,"codec_type":"video","codec_name":"vp9","width":16,"height":12,"nb_read_frames":"12"}]})
    }
    fn project(value: Value) -> Result<StreamingCompletion, CompletionError> {
        project_completed_webm_stream(0, END, &serde_json::to_vec(&value).unwrap(), 1024)
    }
    #[test]
    fn stream_completion_requires_decoded_frames_and_never_invents_duration() {
        let result = project(video()).unwrap();
        assert_eq!(result.metadata().duration_ms, None);
        assert_eq!(result.decoded_frames(), &[(0, 12)]);
        let mut known = video();
        known["format"]["duration"] = json!("1.5");
        assert_eq!(project(known).unwrap().metadata().duration_ms, Some(1500));
        assert!(super::super::completion::project_completed_output(
            MediaActionV1::TranscodeWebm,
            0,
            END,
            &serde_json::to_vec(&video()).unwrap(),
            1024
        )
        .is_err());
    }
    #[test]
    fn missing_zero_unknown_unsafe_or_noncanonical_counts_are_refused() {
        for count in [
            Value::Null,
            json!("0"),
            json!("01"),
            json!("N/A"),
            json!("1e3"),
            json!(1),
            json!("1.5"),
            json!("-1"),
            json!("1000001"),
            json!("999999999999999999999"),
        ] {
            let mut value = video();
            value["streams"][0]["nb_read_frames"] = count;
            assert!(project(value).is_err());
        }
        let mut value = video();
        value["streams"][0]
            .as_object_mut()
            .unwrap()
            .remove("nb_read_frames");
        assert!(project(value).is_err());
        let raw = serde_json::to_string(&video()).unwrap().replace(
            "\"nb_read_frames\":\"12\"",
            "\"nb_read_frames\":\"12\",\"nb_read_frames\":\"13\"",
        );
        assert!(project_completed_webm_stream(0, END, raw.as_bytes(), 1024).is_err());
    }
    #[test]
    fn roles_unknown_metadata_and_duplicate_streams_remain_strict() {
        for field in ["filename", "tags", "unexpected"] {
            let mut value = video();
            value["format"][field] = json!("PRIVATE_CANARY");
            assert!(project(value).is_err());
        }
        for codec in ["h264", "vp8", "pcm_s16le"] {
            let mut value = video();
            value["streams"][0]["codec_name"] = json!(codec);
            assert!(project(value).is_err());
        }
        let mut value = video();
        let duplicate = value["streams"][0].clone();
        value["streams"].as_array_mut().unwrap().push(duplicate);
        assert!(project(value).is_err());
        let mut value = video();
        value["frames"] = json!([{"filename":"PRIVATE"}]);
        assert!(project(value).is_err());
    }
    #[test]
    fn every_optional_audio_stream_must_independently_decode() {
        let mut value = video();
        value["streams"].as_array_mut().unwrap().push(json!({"index":1,"codec_type":"audio","codec_name":"opus","sample_rate":"48000","channels":1,"nb_read_frames":"51"}));
        assert_eq!(
            project(value.clone()).unwrap().decoded_frames(),
            &[(0, 12), (1, 51)]
        );
        value["streams"][1]["nb_read_frames"] = json!("0");
        assert!(project(value).is_err());
    }
    #[test]
    fn progress_exit_actual_size_and_aggregate_limits_are_independent() {
        let raw = serde_json::to_vec(&video()).unwrap();
        assert_eq!(
            project_completed_webm_stream(1, END, &raw, 1024),
            Err(CompletionError::ProcessFailed)
        );
        for progress in [
            b"progress=continue\n".as_slice(),
            b"progress=end\npartial".as_slice(),
            b"progress=end\nprivate=filename\n".as_slice(),
        ] {
            assert!(project_completed_webm_stream(0, progress, &raw, 1024).is_err());
        }
        for size in [0, 1023, 1025, MAX_STREAM_BYTES + 1] {
            assert!(project_completed_webm_stream(0, END, &raw, size).is_err());
        }
        assert!(project_completed_webm_stream(0, END, &vec![b' '; 65_537], 1024).is_err());
    }
    #[test]
    fn fixed_profiles_have_no_output_filename_or_caller_protocol() {
        assert_eq!(webm_arguments().last(), Some(&"pipe:1"));
        assert!(webm_arguments()
            .windows(2)
            .any(|pair| pair == ["-progress", "pipe:2"]));
        assert!(webm_arguments()
            .windows(2)
            .any(|pair| pair == ["-protocol_whitelist", "file"]));
        assert!(probe_arguments().contains(&"-count_frames"));
        assert!(
            !webm_arguments().iter().any(|arg| arg.starts_with("output.")
                || arg.contains("http")
                || arg.contains("filter"))
        );
    }
}
