//! Fixed official signed-source 8.1.3 images. No caller paths or product option.
use super::*;
use crate::runtime::{MediaAudioState, MediaVideoState, ToolHealthState};
use std::time::Instant;

const OFFICIAL_FFMPEG: &[u8] = include_bytes!(".generated/official/ffmpeg.exe");
const OFFICIAL_FFPROBE: &[u8] = include_bytes!(".generated/official/ffprobe.exe");
const OFFICIAL_NOTICES: &[u8] = include_bytes!(".generated/official/notices.txt");
fn checked(role: &str, bytes: &[u8]) -> bool {
    let lock: Value = serde_json::from_str(include_str!(
        "../../../../../scripts/media-candidate/official-8.1.3-lock.json"
    ))
    .unwrap();
    matches!(role, "ffmpeg" | "ffprobe")
        && bytes.len() >= 1024
        && bytes.len() <= 33_554_432
        && bytes.get(..2) == Some(b"MZ")
        && hash(bytes) == lock["programs"][role]
}
fn world() -> World {
    assert!(checked("ffmpeg", OFFICIAL_FFMPEG));
    assert!(checked("ffprobe", OFFICIAL_FFPROBE));
    let lock: Value = serde_json::from_str(include_str!(
        "../../../../../scripts/media-candidate/official-8.1.3-lock.json"
    ))
    .unwrap();
    assert_eq!(hash(OFFICIAL_NOTICES), lock["combinedNoticesSha256"]);
    World::with_fixture(
        &["consumer", "other-consumer"],
        SignedFixture::from_images(OFFICIAL_FFMPEG, OFFICIAL_FFPROBE, OFFICIAL_NOTICES),
    )
}
fn settled(world: &mut World, run: &str, field: &str) {
    let started = Instant::now();
    while world.row(run)[field][0]["phase"] != "completed" {
        assert!(started.elapsed() < Duration::from_secs(10));
        world.core.tick_media_inspections().unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn drained(world: &World, run: &str, count: usize) {
    let values = world.core.media_audio_fixture_observations(run);
    assert_eq!(values.len(), count);
    for value in values {
        assert_ne!(value.0, 0);
        assert_eq!(value.1, 0);
        assert!(value.2);
    }
}

#[test]
fn official_source_images_have_distinct_exact_pins_and_no_implicit_production_admission() {
    assert!(checked("ffmpeg", OFFICIAL_FFMPEG));
    assert!(checked("ffprobe", OFFICIAL_FFPROBE));
    assert!(!checked("ffmpeg", FFMPEG));
    assert!(!checked("ffprobe", FFPROBE));
    assert!(candidate("ffmpeg", OFFICIAL_FFMPEG).is_err());
    let mut changed = OFFICIAL_FFMPEG.to_vec();
    changed[1024] ^= 1;
    assert!(!checked("ffmpeg", &changed));
    assert!(!checked("other", OFFICIAL_FFMPEG));
    let mut world = world();
    let envelope = signed(PACKAGE_PAYLOAD_TYPE, &world.fixture.package, &[3]);
    let trust = world.fixture.trust();
    let time = world.fixture.clock();
    world.core.with_tool_fixture_store(|store| {
        assert!(matches!(
            install::install_inert_tool(&trust, store, world.fixture.input(&envelope), || time),
            Err(install::ToolInstallError::Admission(
                tools::ToolAdmissionError::PolicyDenied
            ))
        ));
    });
    assert_eq!(
        fs::read_dir(world.path.join("tool-image-store-v1/artifacts"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn both_consumers_share_the_official_image_and_pass_the_actual_fixed_native_health() {
    let mut world = world();
    let a = world.start("consumer");
    let b = world.start("other-consumer");
    assert_eq!(
        world.row(&a)["toolArtifacts"]["images"],
        world.row(&b)["toolArtifacts"]["images"]
    );
    for run in [&a, &b] {
        let handle = world.core.start_tool_health(run).unwrap();
        settled(&mut world, run, "nativeAudio");
        drained(&world, run, 2);
        let ToolHealthState::Healthy(observed) = world.core.poll_tool_health(&handle).unwrap()
        else {
            panic!("Official image failed actual native health")
        };
        assert_eq!(
            observed.pin().archive_digest,
            world.fixture.package.archive.sha256
        );
        assert_eq!(
            hash(&fs::read(world.image.join("bin/ffmpeg.exe")).unwrap()),
            hash(OFFICIAL_FFMPEG)
        );
        assert_eq!(
            hash(&fs::read(world.image.join("bin/ffprobe.exe")).unwrap()),
            hash(OFFICIAL_FFPROBE)
        );
    }
    world.core.finish(&a, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&b)["toolArtifacts"]["retained"], true);
    world.core.finish(&b, Err(ErrorCode::Aborted));
}

#[test]
fn official_source_wav_and_webm_keep_the_fixed_profile_and_independent_full_probe() {
    let mut world = world();
    let a = world.start("consumer");
    let b = world.start("other-consumer");
    let audio = world.core.start_media_audio(&a, &wave()).unwrap();
    settled(&mut world, &a, "nativeAudio");
    drained(&world, &a, 2);
    let MediaAudioState::Complete(output) = world.core.poll_media_audio(&audio).unwrap() else {
        panic!("Official WAV incomplete")
    };
    assert_eq!(output.metadata().formats, ["wav"]);
    assert_eq!(output.metadata().duration_ms, Some(1000));
    assert_eq!(&output.bytes()[..4], b"RIFF");
    assert!(output.bytes().len() <= 1_048_576);
    let video = world
        .core
        .start_media_video(&b, &video_input::video(3, 16))
        .unwrap();
    settled(&mut world, &b, "nativeVideo");
    drained(&world, &b, 2);
    let MediaVideoState::Complete(output) = world.core.poll_media_video(&video).unwrap() else {
        panic!("Official WebM incomplete")
    };
    assert_eq!(output.metadata().formats, ["matroska", "webm"]);
    assert_eq!(output.metadata().duration_ms, None);
    assert_eq!(output.decoded_frames(), &[(0, 3)]);
    assert_eq!(output.metadata().streams[0].codec.as_deref(), Some("vp9"));
    assert!(output.bytes().len() <= 1_048_576);
    world.core.finish(&a, Err(ErrorCode::Aborted));
    world.core.finish(&b, Err(ErrorCode::Aborted));
}

#[test]
fn official_source_actual_probe_refuses_revoked_grants_and_drains_without_stale_output() {
    let mut world = world();
    let run = world.start("consumer");
    let admin = world.open("admin");
    let handle = world.core.start_media_audio(&run, &wave()).unwrap();
    let mut reached = false;
    let started = Instant::now();
    while !reached {
        world
            .core
            .tick_media_audio_fixture(
                |core, value| {
                    assert_ne!(value.0, 0);
                    assert_eq!(value.1, 1);
                    assert!(!value.2);
                    reached = true;
                    native_inspection::revoke(core, admin.clone());
                    Ok(())
                },
                |_, _| panic!("Revoked official probe cannot commit"),
            )
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    drained(&world, &run, 2);
    assert_eq!(
        world.core.poll_media_audio(&handle).err(),
        Some(ErrorCode::GrantRevoked)
    );
    assert_eq!(world.row(&run)["nativeAudio"][0]["phase"], "failed");
    world.core.finish(&run, Err(ErrorCode::GrantRevoked));
}
