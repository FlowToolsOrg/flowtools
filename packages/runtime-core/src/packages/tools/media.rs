//! Fixed media recipes and bounded projections. No process, path admission,
//! installed-tool proof, broker grant or filesystem effect is created here.
use super::ToolProgram;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_INSPECTION_BYTES: usize = 65_536;
pub const MAX_MEDIA_BYTES: u64 = 268_435_456;
pub const MAX_DURATION_MS: u64 = 604_800_000;
const MAX_STREAMS: usize = 32;
const MAX_PROGRESS_BYTES: usize = 65_536;
const MAX_PROGRESS_LINE: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaError {
    MetadataInvalid,
    BudgetExceeded,
    ProgressInvalid,
}
impl MediaError {
    pub fn code(self) -> &'static str {
        match self {
            Self::MetadataInvalid => "MEDIA_METADATA_INVALID",
            Self::BudgetExceeded => "MEDIA_BUDGET_EXCEEDED",
            Self::ProgressInvalid => "MEDIA_PROGRESS_INVALID",
        }
    }
}

/// Versioned, finite T0 recipe selection. A recipe is never a tool lease.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MediaActionV1 {
    Inspect,
    ExtractAudioWav,
    TranscodeWebm,
}

pub struct MediaRecipe {
    program: ToolProgram,
    args: &'static [&'static str],
    output: Option<&'static str>,
}
impl MediaRecipe {
    pub fn program(&self) -> ToolProgram {
        self.program
    }
    pub fn arguments(&self) -> &[&'static str] {
        self.args
    }
    pub fn output_name(&self) -> Option<&'static str> {
        self.output
    }
}
pub fn recipe(action: MediaActionV1) -> MediaRecipe {
    match action {
        MediaActionV1::Inspect => MediaRecipe {
            program: ToolProgram::Ffprobe,
            args: &["-v", "error", "-protocol_whitelist", "file", "-show_entries",
                "format=format_name,duration,size:stream=index,codec_type,codec_name,width,height,sample_rate,channels",
                "-of", "json", "-i", "input"],
            output: None,
        },
        MediaActionV1::ExtractAudioWav => MediaRecipe {
            program: ToolProgram::Ffmpeg,
            args: &["-nostdin", "-hide_banner", "-loglevel", "error", "-nostats", "-n",
                "-progress", "pipe:1", "-protocol_whitelist", "file", "-i", "input",
                "-map", "0:a:0", "-vn", "-map_metadata", "-1", "-map_chapters", "-1",
                "-c:a", "pcm_s16le", "-f", "wav", "output.wav"],
            output: Some("output.wav"),
        },
        MediaActionV1::TranscodeWebm => MediaRecipe {
            program: ToolProgram::Ffmpeg,
            args: &["-nostdin", "-hide_banner", "-loglevel", "error", "-nostats", "-n",
                "-progress", "pipe:1", "-protocol_whitelist", "file", "-i", "input",
                "-map", "0:v:0", "-map", "0:a:0?", "-map_metadata", "-1", "-map_chapters", "-1",
                "-c:v", "libvpx-vp9", "-b:v", "1M", "-threads", "1", "-c:a", "libopus",
                "-b:a", "96k", "-f", "webm", "output.webm"],
            output: Some("output.webm"),
        },
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaInspection {
    pub formats: Vec<String>,
    pub duration_ms: Option<u64>,
    pub bytes: Option<u64>,
    pub streams: Vec<MediaStream>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaStream {
    pub index: u32,
    pub kind: StreamKind,
    pub codec: Option<String>,
    pub dimensions: Option<[u32; 2]>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StreamKind {
    Video,
    Audio,
    Subtitle,
    Data,
    Attachment,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProbe {
    format: RawFormat,
    streams: Vec<RawStream>,
    // Current ffprobe emits these containers when selecting stream entries.
    // Only empty arrays are supported; nested program/group data is not selected,
    // retained or silently discarded by the privacy projection.
    #[serde(default, rename = "programs")]
    _programs: [(); 0],
    #[serde(default, rename = "stream_groups")]
    _stream_groups: [(); 0],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFormat {
    format_name: String,
    duration: Option<String>,
    size: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStream {
    index: u32,
    codec_type: StreamKind,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    sample_rate: Option<String>,
    channels: Option<u32>,
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn unsigned(value: &str, maximum: u64) -> Result<u64, MediaError> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err(MediaError::MetadataInvalid);
    }
    let value = value
        .parse::<u64>()
        .map_err(|_| MediaError::MetadataInvalid)?;
    if value > maximum {
        return Err(MediaError::BudgetExceeded);
    }
    Ok(value)
}
fn milliseconds(value: &str) -> Result<u64, MediaError> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if value.len() > 30
        || (value.contains('.') && fraction.is_empty())
        || fraction.len() > 9
        || !fraction.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(MediaError::MetadataInvalid);
    }
    let seconds = unsigned(whole, MAX_DURATION_MS / 1000)?;
    if seconds == MAX_DURATION_MS / 1000 && fraction.bytes().any(|b| b != b'0') {
        return Err(MediaError::BudgetExceeded);
    }
    let mut ms = 0;
    for byte in fraction.bytes().chain(std::iter::repeat(b'0')).take(3) {
        ms = ms * 10 + u64::from(byte - b'0');
    }
    let value = seconds * 1000 + ms;
    if value > MAX_DURATION_MS {
        Err(MediaError::BudgetExceeded)
    } else {
        Ok(value)
    }
}
pub fn parse_inspection(bytes: &[u8]) -> Result<MediaInspection, MediaError> {
    if bytes.len() > MAX_INSPECTION_BYTES {
        return Err(MediaError::BudgetExceeded);
    }
    // Reuse duplicate-key refusal, not serde_json's last-key-wins Value parser.
    let value = super::super::strict_json(bytes).map_err(|_| MediaError::MetadataInvalid)?;
    let raw: RawProbe = serde_json::from_value(value).map_err(|_| MediaError::MetadataInvalid)?;
    if raw.streams.is_empty() || raw.streams.len() > MAX_STREAMS {
        return Err(MediaError::BudgetExceeded);
    }
    let formats: Vec<String> = raw
        .format
        .format_name
        .split(',')
        .map(str::to_owned)
        .collect();
    if formats.is_empty()
        || formats.len() > 16
        || formats.iter().any(|v| !identifier(v))
        || formats.iter().collect::<BTreeSet<_>>().len() != formats.len()
    {
        return Err(MediaError::MetadataInvalid);
    }
    let duration_ms = raw
        .format
        .duration
        .filter(|s| s != "N/A")
        .map(|s| milliseconds(&s))
        .transpose()?;
    let bytes = raw
        .format
        .size
        .filter(|s| s != "N/A")
        .map(|s| unsigned(&s, MAX_MEDIA_BYTES))
        .transpose()?;
    let mut indexes = BTreeSet::new();
    let mut streams = Vec::new();
    for raw in raw.streams {
        if raw.index as usize >= MAX_STREAMS
            || !indexes.insert(raw.index)
            || raw.codec_name.as_deref().is_some_and(|s| !identifier(s))
        {
            return Err(MediaError::MetadataInvalid);
        }
        let dimensions = match (raw.width, raw.height) {
            (None, None) => None,
            (Some(w), Some(h))
                if w > 0
                    && h > 0
                    && w <= 16384
                    && h <= 16384
                    && u64::from(w) * u64::from(h) <= 67_108_864 =>
            {
                Some([w, h])
            }
            _ => return Err(MediaError::BudgetExceeded),
        };
        let sample_rate = raw
            .sample_rate
            .map(|s| {
                unsigned(&s, 384000).and_then(|n| {
                    if n == 0 {
                        Err(MediaError::MetadataInvalid)
                    } else {
                        Ok(n as u32)
                    }
                })
            })
            .transpose()?;
        if raw.channels.is_some_and(|n| n == 0 || n > 32) {
            return Err(MediaError::BudgetExceeded);
        }
        streams.push(MediaStream {
            index: raw.index,
            kind: raw.codec_type,
            codec: raw.codec_name,
            dimensions,
            sample_rate,
            channels: raw.channels,
        });
    }
    Ok(MediaInspection {
        formats,
        duration_ms,
        bytes,
        streams,
    })
}

/// Advisory metadata only. Actual bytes, deadlines and Job exit remain T0 facts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MediaProgress {
    pub output_time_ms: Option<u64>,
    pub reported_bytes: Option<u64>,
    pub ended: bool,
}
#[derive(Default)]
pub struct ProgressDecoder {
    line: Vec<u8>,
    total: usize,
    pending: MediaProgress,
    observed: MediaProgress,
    seen_time: bool,
    seen_bytes: bool,
    failed: bool,
    max_time_ms: u64,
    max_bytes: u64,
}
impl ProgressDecoder {
    pub fn snapshot(&self) -> MediaProgress {
        self.observed.clone()
    }
    pub fn push(&mut self, bytes: &[u8]) -> Result<MediaProgress, MediaError> {
        if self.failed {
            return Err(MediaError::ProgressInvalid);
        }
        let result = self.push_inner(bytes);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn push_inner(&mut self, bytes: &[u8]) -> Result<MediaProgress, MediaError> {
        self.total = self
            .total
            .checked_add(bytes.len())
            .ok_or(MediaError::BudgetExceeded)?;
        if self.total > MAX_PROGRESS_BYTES || (self.observed.ended && !bytes.is_empty()) {
            return Err(MediaError::BudgetExceeded);
        }
        for &byte in bytes {
            if byte == b'\n' {
                let line = std::mem::take(&mut self.line);
                let line = line.strip_suffix(b"\r").unwrap_or(&line);
                let text = std::str::from_utf8(line).map_err(|_| MediaError::ProgressInvalid)?;
                self.record(text)?;
            } else {
                if self.line.len() >= MAX_PROGRESS_LINE
                    || (byte != b'\r' && !(0x20..=0x7e).contains(&byte))
                {
                    return Err(MediaError::ProgressInvalid);
                }
                self.line.push(byte);
            }
        }
        Ok(self.snapshot())
    }
    pub fn finish(self) -> Result<MediaProgress, MediaError> {
        if self.failed
            || !self.line.is_empty()
            || !self.observed.ended
            || self.seen_time
            || self.seen_bytes
        {
            return Err(MediaError::ProgressInvalid);
        }
        Ok(self.observed)
    }
    fn record(&mut self, line: &str) -> Result<(), MediaError> {
        if self.observed.ended {
            return Err(MediaError::ProgressInvalid);
        }
        if line.bytes().any(|b| !(0x20..=0x7e).contains(&b)) {
            return Err(MediaError::ProgressInvalid);
        }
        let (key, value) = line.split_once('=').ok_or(MediaError::ProgressInvalid)?;
        if !identifier(key) {
            return Err(MediaError::ProgressInvalid);
        }
        match key {
            "out_time_us" => {
                if self.seen_time {
                    return Err(MediaError::ProgressInvalid);
                }
                self.seen_time = true;
                if value == "N/A" {
                    self.pending.output_time_ms = None;
                    return Ok(());
                }
                let (negative, digits) = value
                    .strip_prefix('-')
                    .map_or((false, value), |s| (true, s));
                let micros = unsigned(digits, MAX_DURATION_MS * 1000)?;
                self.pending.output_time_ms = Some(if negative { 0 } else { micros / 1000 });
            }
            "total_size" => {
                if self.seen_bytes {
                    return Err(MediaError::ProgressInvalid);
                }
                self.seen_bytes = true;
                self.pending.reported_bytes = if value == "N/A" {
                    None
                } else {
                    Some(unsigned(value, MAX_MEDIA_BYTES)?)
                };
            }
            "progress" => {
                self.pending.ended = match value {
                    "continue" => false,
                    "end" => true,
                    _ => return Err(MediaError::ProgressInvalid),
                };
                if self
                    .pending
                    .output_time_ms
                    .is_some_and(|v| v < self.max_time_ms)
                    || self
                        .pending
                        .reported_bytes
                        .is_some_and(|v| v < self.max_bytes)
                {
                    return Err(MediaError::ProgressInvalid);
                }
                self.observed = self.pending.clone();
                self.max_time_ms = self
                    .max_time_ms
                    .max(self.observed.output_time_ms.unwrap_or(0));
                self.max_bytes = self
                    .max_bytes
                    .max(self.observed.reported_bytes.unwrap_or(0));
                self.seen_time = false;
                self.seen_bytes = false;
            }
            // FFmpeg's remaining bounded fields are intentionally not projected.
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn sample() -> serde_json::Value {
        json!({"format":{"format_name":"matroska,webm","duration":"1.234567","size":"1024"},
            "streams":[{"index":0,"codec_type":"video","codec_name":"vp9","width":1920,"height":1080},
                {"index":1,"codec_type":"audio","codec_name":"opus","sample_rate":"48000","channels":2}]})
    }
    fn parse(value: serde_json::Value) -> Result<MediaInspection, MediaError> {
        parse_inspection(&serde_json::to_vec(&value).unwrap())
    }
    #[test]
    fn only_fixed_versioned_recipes_select_program_roles_and_workspace_names() {
        for action in [
            MediaActionV1::Inspect,
            MediaActionV1::ExtractAudioWav,
            MediaActionV1::TranscodeWebm,
        ] {
            let plan = recipe(action);
            assert_eq!(
                plan.program(),
                if action == MediaActionV1::Inspect {
                    ToolProgram::Ffprobe
                } else {
                    ToolProgram::Ffmpeg
                }
            );
            assert!(plan.arguments().windows(2).any(|p| p == ["-i", "input"]));
            assert!(plan
                .arguments()
                .windows(2)
                .any(|p| p == ["-protocol_whitelist", "file"]));
            assert!(!plan
                .arguments()
                .iter()
                .any(|s| s.contains("http") || s.contains("..") || *s == "-y"));
            if action != MediaActionV1::Inspect {
                assert!(plan.arguments().contains(&"-n"));
                assert!(plan.arguments().contains(&"-nostdin"));
                assert_eq!(plan.arguments().last().copied(), plan.output_name());
            }
        }
        for hostile in [
            r#""transcode;whoami""#,
            r#"{"operation":"inspect","argv":["-y"]}"#,
            r#""C:\\ffmpeg.exe""#,
        ] {
            assert!(serde_json::from_str::<MediaActionV1>(hostile).is_err());
        }
    }
    #[test]
    fn probe_projection_preserves_only_bounded_media_facts_and_exact_milliseconds() {
        let parsed = parse(sample()).unwrap();
        assert_eq!(parsed.duration_ms, Some(1234));
        assert_eq!(parsed.bytes, Some(1024));
        assert_eq!(parsed.formats, ["matroska", "webm"]);
        assert_eq!(parsed.streams[0].dimensions, Some([1920, 1080]));
        assert_eq!(parsed.streams[1].sample_rate, Some(48000));
        assert_eq!(parsed.streams[1].channels, Some(2));
    }
    #[test]
    fn actual_ffprobe_empty_containers_are_supported_but_cannot_carry_nested_metadata() {
        let mut actual = sample();
        actual["programs"] = json!([]);
        actual["stream_groups"] = json!([]);
        assert_eq!(parse(actual.clone()).unwrap(), parse(sample()).unwrap());
        for field in ["programs", "stream_groups"] {
            for hostile in [
                json!(null),
                json!({}),
                json!([null]),
                json!([{}]),
                json!([{"tags":{"private":"PRIVATE_CANARY"}}]),
            ] {
                let mut changed = actual.clone();
                changed[field] = hostile;
                assert_eq!(parse(changed).unwrap_err(), MediaError::MetadataInvalid);
            }
        }
    }
    #[test]
    fn duplicate_keys_private_paths_tags_and_control_characters_are_refused() {
        assert_eq!(
            parse_inspection(br#"{"format":{},"format":{},"streams":[]}"#).unwrap_err(),
            MediaError::MetadataInvalid
        );
        for extra in [
            "filename",
            "tags",
            "stderr",
            "credential",
            "programs",
            "argv",
        ] {
            let mut value = sample();
            value["format"][extra] = json!("PRIVATE_PATH_CANARY");
            assert_eq!(parse(value).unwrap_err(), MediaError::MetadataInvalid);
        }
        for codec in ["../../private", "C:\\private", "vp9\nSECRET", "vp9\0", ""] {
            let mut value = sample();
            value["streams"][0]["codec_name"] = json!(codec);
            assert_eq!(parse(value).unwrap_err(), MediaError::MetadataInvalid);
        }
    }
    #[test]
    fn inspection_byte_stream_dimension_and_integer_budgets_are_enforced() {
        assert_eq!(
            parse_inspection(&vec![b' '; MAX_INSPECTION_BYTES + 1]).unwrap_err(),
            MediaError::BudgetExceeded
        );
        let mut value = sample();
        value["streams"] = json!(vec![value["streams"][0].clone(); 33]);
        assert_eq!(parse(value).unwrap_err(), MediaError::BudgetExceeded);
        for dimension in [0, 16385, u32::MAX as u64, 9_007_199_254_740_991] {
            let mut value = sample();
            value["streams"][0]["width"] = json!(dimension);
            assert!(parse(value).is_err());
        }
        let mut value = sample();
        value["streams"][1]["index"] = json!(0);
        assert_eq!(parse(value).unwrap_err(), MediaError::MetadataInvalid);
        for size in ["-1", "1e3", "nan", "18446744073709551616", "268435457"] {
            let mut value = sample();
            value["format"]["size"] = json!(size);
            assert!(parse(value).is_err());
        }
    }
    #[test]
    fn unknown_duration_is_explicit_and_invalid_decimals_cannot_be_rounded_into_range() {
        let mut value = sample();
        value["format"]["duration"] = json!("N/A");
        assert_eq!(parse(value).unwrap().duration_ms, None);
        for duration in [
            "-0.1",
            "NaN",
            "1e3",
            "1.",
            ".1",
            "1.0000000000",
            "604800.001",
            "604800.000000001",
        ] {
            let mut value = sample();
            value["format"]["duration"] = json!(duration);
            assert!(parse(value).is_err(), "{duration}");
        }
        assert_eq!(milliseconds("0.001"), Ok(1));
        assert_eq!(milliseconds("604800.000"), Ok(MAX_DURATION_MS));
    }
    #[test]
    fn chunked_crlf_progress_converts_microseconds_and_commits_only_complete_records() {
        let mut parser = ProgressDecoder::default();
        assert_eq!(
            parser.push(b"out_time_us=123").unwrap(),
            MediaProgress::default()
        );
        assert_eq!(
            parser.push(b"4567\r\ntotal_size=1024\r\n").unwrap(),
            MediaProgress::default()
        );
        assert_eq!(
            parser
                .push(b"speed=1.23x\r\nprogress=continue\r\n")
                .unwrap(),
            MediaProgress {
                output_time_ms: Some(1234),
                reported_bytes: Some(1024),
                ended: false
            }
        );
        parser
            .push(b"out_time_ms=9999999\nout_time_us=2345678\ntotal_size=2048\nprogress=end\n")
            .unwrap();
        assert_eq!(
            parser.finish().unwrap(),
            MediaProgress {
                output_time_ms: Some(2345),
                reported_bytes: Some(2048),
                ended: true
            }
        );
    }
    #[test]
    fn negative_initial_timestamps_and_unknown_muxer_size_are_not_execution_success() {
        let mut parser = ProgressDecoder::default();
        parser
            .push(b"out_time_us=-7111\ntotal_size=N/A\nprogress=end\n")
            .unwrap();
        assert_eq!(
            parser.finish().unwrap(),
            MediaProgress {
                output_time_ms: Some(0),
                reported_bytes: None,
                ended: true
            }
        );
        assert!(ProgressDecoder::default().finish().is_err());
    }
    #[test]
    fn unavailable_native_timestamps_stay_unknown_and_cannot_reset_high_watermarks() {
        let mut parser = ProgressDecoder::default();
        parser
            .push(b"out_time_us=N/A\ntotal_size=N/A\nprogress=continue\n")
            .unwrap();
        assert_eq!(parser.snapshot().output_time_ms, None);
        parser
            .push(b"out_time_us=2000\ntotal_size=20\nprogress=continue\n")
            .unwrap();
        parser
            .push(b"out_time_us=N/A\ntotal_size=N/A\nprogress=continue\n")
            .unwrap();
        assert_eq!(parser.snapshot().reported_bytes, None);
        assert!(parser
            .push(b"out_time_us=1000\ntotal_size=10\nprogress=end\n")
            .is_err());
        assert!(parser.finish().is_err());
    }
    #[test]
    fn malformed_or_regressing_progress_permanently_poisons_the_decoder() {
        for bytes in [
            b"out_time_us=1\nout_time_us=2\n".as_slice(),
            b"progress=yes\n",
            b"secret\n",
            b"progress=end\nprogress=end\n",
            b"a=\0\n",
            b"secret=one\rtwo\n",
        ] {
            let mut parser = ProgressDecoder::default();
            assert!(parser.push(bytes).is_err());
            assert!(parser.push(b"progress=end\n").is_err());
            assert!(parser.finish().is_err());
        }
        let mut parser = ProgressDecoder::default();
        parser
            .push(b"out_time_us=2000\ntotal_size=20\nprogress=continue\n")
            .unwrap();
        assert!(parser
            .push(b"out_time_us=1000\ntotal_size=10\nprogress=end\n")
            .is_err());
    }
    #[test]
    fn progress_line_total_and_reported_byte_budgets_refuse_without_unbounded_buffers() {
        let mut parser = ProgressDecoder::default();
        assert!(parser.push(&vec![b'x'; 513]).is_err());
        assert!(parser.line.len() <= MAX_PROGRESS_LINE);
        let mut parser = ProgressDecoder::default();
        assert_eq!(
            parser
                .push(&vec![b' '; MAX_PROGRESS_BYTES + 1])
                .unwrap_err(),
            MediaError::BudgetExceeded
        );
        assert!(parser.line.is_empty());
        let mut parser = ProgressDecoder::default();
        assert!(parser
            .push(b"total_size=268435457\nprogress=end\n")
            .is_err());
        let mut parser = ProgressDecoder::default();
        parser.push(b"progress=end\n").unwrap();
        assert!(parser.push(b"x").is_err());
        assert!(parser.finish().is_err());
    }
}
