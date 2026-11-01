//! Closed T0 stdio profiles. No caller argv, paths, environment or filter graph.
use crate::packages::tools::{
    media::{self, MediaActionV1},
    ToolProgram,
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Profile {
    Inspect,
    ExtractAudio,
}
impl Profile {
    pub(super) fn program(self) -> ToolProgram {
        match self {
            Self::Inspect => ToolProgram::Ffprobe,
            Self::ExtractAudio => ToolProgram::Ffmpeg,
        }
    }
    pub(super) fn arguments(self) -> Vec<&'static str> {
        match self {
            Self::Inspect => media::recipe(MediaActionV1::Inspect).arguments().to_vec(),
            // Output is only stdout. The child receives no writable output
            // directory/file; T0 enforces byte limits while reading its pipe.
            Self::ExtractAudio => vec![
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
                "0:a:0",
                "-vn",
                "-map_metadata",
                "-1",
                "-map_chapters",
                "-1",
                "-c:a",
                "pcm_s16le",
                "-f",
                "wav",
                "pipe:1",
            ],
        }
    }
    pub(super) fn output_limit(self) -> usize {
        match self {
            Self::Inspect => 65_536,
            Self::ExtractAudio => 1_048_576,
        }
    }
    pub(super) fn aggregate_limit(self) -> usize {
        match self {
            Self::Inspect => 65_536,
            Self::ExtractAudio => 1_114_112,
        }
    }
    pub(super) fn keeps_stderr(self) -> bool {
        self == Self::ExtractAudio
    }
    pub(super) fn pipe_bytes(self) -> u32 {
        if self == Self::Inspect {
            4096
        } else {
            65_536
        }
    }
    pub(super) fn read_bytes(self) -> usize {
        if self == Self::Inspect {
            8192
        } else {
            65_536
        }
    }
}
