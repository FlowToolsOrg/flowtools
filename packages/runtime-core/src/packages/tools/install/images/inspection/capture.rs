//! Finite native stdout/progress capture, with sticky refusal after overflow.
use super::profile::Profile;
use std::io;
pub(super) struct Capture {
    profile: Profile,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    total: usize,
    refused: bool,
    finished: bool,
}
impl Capture {
    pub(super) fn progress(&self) -> io::Result<crate::packages::tools::media::MediaProgress> {
        if self.refused || self.finished || !self.profile.is_encoding() {
            return Err(self.invalid());
        }
        let mut decoder = crate::packages::tools::media::ProgressDecoder::default();
        decoder
            .push(&self.stderr)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Native progress refused"))
    }
    pub(super) fn new(profile: Profile) -> Self {
        Self {
            profile,
            stdout: Vec::new(),
            stderr: Vec::new(),
            total: 0,
            refused: false,
            finished: false,
        }
    }
    fn invalid(&self) -> io::Error {
        io::Error::new(
            if self.profile.is_encoding() {
                io::ErrorKind::FileTooLarge
            } else {
                io::ErrorKind::Other
            },
            "Native output budget refused",
        )
    }
    pub(super) fn push(&mut self, stdout: bool, bytes: &[u8]) -> io::Result<()> {
        let length = if stdout {
            self.stdout.len()
        } else {
            self.total - self.stdout.len()
        };
        let limit = if stdout {
            self.profile.output_limit()
        } else {
            65_536
        };
        if self.refused
            || self.finished
            || bytes.len() > limit.saturating_sub(length)
            || bytes.len() > self.profile.aggregate_limit().saturating_sub(self.total)
        {
            self.refused = true;
            return Err(self.invalid());
        }
        self.total += bytes.len();
        if stdout {
            self.stdout.extend_from_slice(bytes);
        } else if self.profile.keeps_stderr() {
            self.stderr.extend_from_slice(bytes);
        }
        Ok(())
    }
    pub(super) fn finish(&mut self) -> io::Result<(Vec<u8>, Vec<u8>)> {
        if self.refused || self.finished {
            return Err(self.invalid());
        }
        self.finished = true;
        Ok((
            std::mem::take(&mut self.stdout),
            std::mem::take(&mut self.stderr),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_observes_only_complete_records_and_never_finishes_the_capture() {
        let mut capture = Capture::new(Profile::ExtractAudio);
        capture
            .push(false, b"out_time_us=1000\r\nprogress=cont")
            .unwrap();
        assert_eq!(capture.progress().unwrap().output_time_ms, None);
        capture.push(false, b"inue\r\n").unwrap();
        assert_eq!(capture.progress().unwrap().output_time_ms, Some(1));
        capture
            .push(false, b"out_time_us=N/A\ntotal_size=N/A\nprogress=end\n")
            .unwrap();
        let observed = capture.progress().unwrap();
        assert!(observed.ended);
        assert_eq!(observed.output_time_ms, None);
        assert_eq!(observed.reported_bytes, None);
        capture.push(true, b"PRIVATE_BINARY_OUTPUT").unwrap();
        assert_eq!(capture.progress().unwrap(), observed);
        assert_eq!(capture.finish().unwrap().0, b"PRIVATE_BINARY_OUTPUT");
        assert!(capture.progress().is_err());
    }
    #[test]
    fn progress_refuses_poisoned_or_overflowed_capture_and_never_returns_private_stderr() {
        let mut capture = Capture::new(Profile::TranscodeWebm);
        capture
            .push(false, b"out_time_us=2000\nprogress=continue\n")
            .unwrap();
        assert_eq!(capture.progress().unwrap().output_time_ms, Some(2));
        capture
            .push(false, b"out_time_us=1000\nprogress=continue\n")
            .unwrap();
        assert_eq!(
            capture.progress().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        capture.push(false, b"progress=end\n").unwrap();
        assert!(capture.progress().is_err());
        let mut capture = Capture::new(Profile::ExtractAudio);
        capture.push(false, &vec![b'x'; 65536]).unwrap();
        assert!(capture.push(false, b"x").is_err());
        assert!(capture.progress().is_err());
        let mut capture = Capture::new(Profile::Inspect);
        capture.push(false, b"PRIVATE_STDERR").unwrap();
        assert!(capture.progress().is_err());
    }
    #[test]
    fn inspection_keeps_original_aggregate_limit_and_discards_private_stderr() {
        let mut capture = Capture::new(Profile::Inspect);
        capture.push(true, b"{}").unwrap();
        capture.push(false, &vec![b'x'; 65_534]).unwrap();
        let (stdout, stderr) = capture.finish().unwrap();
        assert_eq!(stdout, b"{}");
        assert!(stderr.is_empty());
        assert!(capture.finish().is_err());
        let mut capture = Capture::new(Profile::Inspect);
        capture.push(false, &vec![b'x'; 65_536]).unwrap();
        assert!(capture.push(true, b"x").is_err());
        assert!(capture.push(true, b"").is_err());
        assert!(capture.finish().is_err());
    }
    #[test]
    fn conversion_bytes_and_progress_have_independent_hard_limits_and_sticky_refusal() {
        for (profile, overflow_stdout) in [Profile::ExtractAudio, Profile::TranscodeWebm]
            .into_iter()
            .flat_map(|profile| [true, false].map(|overflow| (profile, overflow)))
        {
            let mut capture = Capture::new(profile);
            capture.push(true, &vec![0xA5; 1_048_576]).unwrap();
            capture.push(false, &vec![b'x'; 65_536]).unwrap();
            assert_eq!(
                capture.push(overflow_stdout, b"x").unwrap_err().kind(),
                io::ErrorKind::FileTooLarge
            );
            assert!(capture.push(!overflow_stdout, b"").is_err());
            assert!(capture.finish().is_err());
        }
        let mut capture = Capture::new(Profile::ExtractAudio);
        capture.push(true, b"RIFF").unwrap();
        capture.push(false, b"progress=end\n").unwrap();
        let (bytes, progress) = capture.finish().unwrap();
        assert_eq!(bytes, b"RIFF");
        assert_eq!(progress, b"progress=end\n");
    }
    #[test]
    fn extraction_profile_uses_fixed_pipes_without_a_writable_output_filename() {
        let args = Profile::ExtractAudio.arguments();
        assert_eq!(
            Profile::ExtractAudio.program(),
            crate::packages::tools::ToolProgram::Ffmpeg
        );
        assert_eq!(args.last(), Some(&"pipe:1"));
        assert!(args.windows(2).any(|pair| pair == ["-progress", "pipe:2"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-protocol_whitelist", "file"]));
        assert_eq!(args.iter().filter(|arg| **arg == "input").count(), 1);
        assert!(!args.iter().any(|arg| arg.starts_with("output.")
            || arg.contains("http")
            || arg.contains("filter")));
        assert_eq!(Profile::Inspect.arguments(), media_args());
    }
    fn media_args() -> Vec<&'static str> {
        crate::packages::tools::media::recipe(crate::packages::tools::media::MediaActionV1::Inspect)
            .arguments()
            .to_vec()
    }
}
