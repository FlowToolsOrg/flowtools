#[cfg(not(all(windows, target_arch = "x86_64")))]
compile_error!("Real-media candidate acceptance requires Windows/x64; do not report an empty suite as success.");
// This separate non-published workspace requires explicit candidate preparation.
// Missing files fail compilation. There is no test skip or Runtime Host selector.
#[allow(dead_code)]
#[path = "../../../tests/isolation/harness.rs"]
mod harness;
use flowtools_runtime_core::packages::tools::{
    media::{self, MediaActionV1, ProgressDecoder, StreamKind},
    ToolProgram,
};
use harness::*;
mod video_input;
use sha2::{Digest, Sha256};
use std::{
    fs,
    net::{TcpListener, TcpStream},
    time::{Duration, Instant},
};
use video_input::video;

const FFMPEG: &[u8] = include_bytes!(".generated/bin/ffmpeg.exe");
const FFPROBE: &[u8] = include_bytes!(".generated/bin/ffprobe.exe");
const LOCK: &str = include_str!("candidate-lock.json");

fn checked_image(bytes: &'static [u8], expected: &str) -> Result<&'static [u8], &'static str> {
    if bytes.len() < 1024 || bytes.len() > 33_554_432 || bytes.get(..2) != Some(b"MZ") {
        return Err("candidate image budget/header");
    }
    if format!("{:x}", Sha256::digest(bytes)) != expected {
        return Err("candidate image hash");
    }
    Ok(bytes)
}
fn fixture(program: ToolProgram) -> Fixture {
    let lock: serde_json::Value = serde_json::from_str(LOCK).unwrap();
    assert_eq!(lock["formatVersion"], 1);
    assert_eq!(lock["maturity"], "prototype");
    // Both fixed files pass before either can be placed in the fixture tree.
    let ffmpeg = checked_image(FFMPEG, lock["programs"]["ffmpeg"].as_str().unwrap()).unwrap();
    let ffprobe = checked_image(FFPROBE, lock["programs"]["ffprobe"].as_str().unwrap()).unwrap();
    Fixture::from_program_bytes(match program {
        ToolProgram::Ffmpeg => ffmpeg,
        ToolProgram::Ffprobe => ffprobe,
    })
    .unwrap()
}
fn run(fixture: &Fixture, action: MediaActionV1) -> (u32, Vec<u8>, Vec<u8>) {
    let recipe = media::recipe(action);
    let child = fixture
        .launch(true, "media", 0, 512 * 1024 * 1024, recipe.arguments())
        .unwrap();
    assert_eq!(child.child_policy, 1);
    assert_eq!(child.active_limit, 1);
    assert!(!child.private_inherited);
    let output = child.output(Duration::from_secs(5), 65_536).unwrap();
    child.wait_empty(Duration::from_millis(500)).unwrap();
    assert_eq!(child.active().unwrap(), 0);
    assert_eq!(
        fs::read(fixture.host.join("private.txt")).unwrap(),
        b"HOST_PRIVATE_CANARY"
    );
    output
}
fn wave() -> Vec<u8> {
    let mut bytes = Vec::new();
    let pcm: Vec<u8> = (0..16_000u32)
        .flat_map(|sample| ((sample.wrapping_mul(127) % 30_000) as i16 - 15_000).to_le_bytes())
        .collect();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    bytes.extend_from_slice(&16_000u32.to_le_bytes());
    bytes.extend_from_slice(&32_000u32.to_le_bytes());
    bytes.extend_from_slice(b"\x02\0\x10\0data");
    bytes.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&pcm);
    bytes
}
fn wave_pcm(bytes: &[u8]) -> &[u8] {
    assert_eq!(bytes.get(..4), Some(b"RIFF".as_slice()));
    assert_eq!(bytes.get(8..12), Some(b"WAVE".as_slice()));
    let mut offset = 12;
    while offset + 8 <= bytes.len() {
        let length = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let end = offset.checked_add(8).unwrap().checked_add(length).unwrap();
        assert!(end <= bytes.len());
        if &bytes[offset..offset + 4] == b"data" {
            return &bytes[offset + 8..end];
        }
        offset = end + length % 2;
    }
    panic!("actual output has no complete PCM chunk")
}
fn inspect(bytes: &[u8]) -> media::MediaInspection {
    let fixture = fixture(ToolProgram::Ffprobe);
    fs::write(fixture.work.join("input"), bytes).unwrap();
    let (code, output, _) = run(&fixture, MediaActionV1::Inspect);
    assert_eq!(code, 0);
    media::parse_inspection(&output).unwrap_or_else(|error| {
        panic!(
            "controlled probe metadata {error:?}: {}",
            String::from_utf8_lossy(&output)
        )
    })
}
#[test]
fn fixed_candidate_hash_refuses_before_any_child_is_created() {
    assert_eq!(
        checked_image(FFMPEG, &"0".repeat(64)).unwrap_err(),
        "candidate image hash"
    );
    assert_eq!(
        checked_image(b"MZ", &"0".repeat(64)).unwrap_err(),
        "candidate image budget/header"
    );
}
#[test]
fn actual_ffprobe_projects_only_bounded_audio_metadata() {
    let input = wave();
    let info = inspect(&input);
    assert_eq!(info.formats, ["wav"]);
    assert_eq!(info.duration_ms, Some(1000));
    assert_eq!(info.bytes, Some(input.len() as u64));
    assert_eq!(info.streams.len(), 1);
    assert_eq!(info.streams[0].kind, StreamKind::Audio);
    assert_eq!(info.streams[0].codec.as_deref(), Some("pcm_s16le"));
    assert_eq!(info.streams[0].sample_rate, Some(16_000));
    assert_eq!(info.streams[0].channels, Some(1));
}
#[test]
fn actual_wav_extraction_preserves_pcm_and_refuses_existing_output() {
    let fixture = fixture(ToolProgram::Ffmpeg);
    let input = wave();
    fs::write(fixture.work.join("input"), &input).unwrap();
    let (code, progress, _) = run(&fixture, MediaActionV1::ExtractAudioWav);
    assert_eq!(code, 0);
    let mut decoder = ProgressDecoder::default();
    decoder.push(&progress).unwrap();
    let final_progress = decoder.finish().unwrap();
    assert!(final_progress.ended);
    assert_eq!(final_progress.output_time_ms, Some(1000));
    let output = fs::read(fixture.work.join("output.wav")).unwrap();
    assert_eq!(wave_pcm(&output), wave_pcm(&input));
    assert_eq!(inspect(&output).duration_ms, Some(1000));
    let existing = b"EXISTING_OUTPUT_MUST_NOT_CHANGE";
    fs::write(fixture.work.join("output.wav"), existing).unwrap();
    let (code, progress, stderr) = run(&fixture, MediaActionV1::ExtractAudioWav);
    assert_eq!(fs::read(fixture.work.join("output.wav")).unwrap(), existing);
    // This pinned upstream main maps AVERROR_EXIT to zero, including -n refusal.
    // Assert the real exit AND missing complete progress, not assumed exit rules.
    assert_eq!(code, 0);
    assert!(String::from_utf8_lossy(&stderr).contains("already exists. Exiting."));
    let mut refused_progress = ProgressDecoder::default();
    refused_progress.push(&progress).unwrap();
    assert!(refused_progress.finish().is_err());
}
#[test]
fn actual_webm_output_is_inspected_and_redecoded_in_new_private_jobs() {
    let fixture = fixture(ToolProgram::Ffmpeg);
    fs::write(fixture.work.join("input"), video(3, 16)).unwrap();
    let (code, _, stderr) = run(&fixture, MediaActionV1::TranscodeWebm);
    assert_eq!(
        code,
        0,
        "controlled encoder: {}",
        String::from_utf8_lossy(&stderr)
    );
    let output = fs::read(fixture.work.join("output.webm")).unwrap();
    assert_eq!(&output[..4], b"\x1a\x45\xdf\xa3");
    let info = inspect(&output);
    assert_eq!(info.formats, ["matroska", "webm"]);
    assert_eq!(info.streams[0].codec.as_deref(), Some("vp9"));
    assert_eq!(info.streams[0].dimensions, Some([16, 16]));
    assert_eq!(info.duration_ms, Some(3000));
    let second = self::fixture(ToolProgram::Ffmpeg);
    fs::write(second.work.join("input"), output).unwrap();
    let (code, _, _) = run(&second, MediaActionV1::TranscodeWebm);
    assert_eq!(code, 0);
    let decoded = fs::read(second.work.join("output.webm")).unwrap();
    assert_eq!(inspect(&decoded).streams[0].dimensions, Some([16, 16]));
}
#[test]
fn actual_candidate_refuses_playlist_protocols_without_contacting_the_listener() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let control = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let accepted = listener.accept().unwrap();
    drop((control, accepted));
    let fixture = fixture(ToolProgram::Ffprobe);
    let playlist = format!("#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXTINF:1,\nhttp://127.0.0.1:{}/private\n#EXT-X-ENDLIST\n",listener.local_addr().unwrap().port());
    fs::write(fixture.work.join("input"), playlist).unwrap();
    let (code, _, _) = run(&fixture, MediaActionV1::Inspect);
    assert_ne!(code, 0);
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
#[test]
fn actual_ffmpeg_cancellation_reaps_the_owned_job_and_preserves_host_bytes() {
    let fixture = fixture(ToolProgram::Ffmpeg);
    fs::write(fixture.work.join("input"), video(5000, 64)).unwrap();
    let recipe = media::recipe(MediaActionV1::TranscodeWebm);
    let child = fixture
        .launch(true, "media", 0, 512 * 1024 * 1024, recipe.arguments())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while fs::metadata(fixture.work.join("output.webm")).map_or(true, |m| m.len() == 0) {
        assert!(
            !child.exited().unwrap(),
            "real encoder must reach actual output before cancellation"
        );
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!child.exited().unwrap());
    assert_eq!(child.active().unwrap(), 1);
    let cancelled = Instant::now();
    child.terminate().unwrap();
    assert_eq!(child.wait(Duration::from_millis(500)).unwrap(), 71);
    child.wait_empty(Duration::from_millis(500)).unwrap();
    assert_eq!(child.active().unwrap(), 0);
    assert!(cancelled.elapsed() < Duration::from_secs(1));
    assert_eq!(
        fs::read(fixture.host.join("private.txt")).unwrap(),
        b"HOST_PRIVATE_CANARY"
    );
    // Partial workspace bytes are never copied to an output outside this fixture.
    assert!(!fixture.host.join("output.webm").exists());
}
