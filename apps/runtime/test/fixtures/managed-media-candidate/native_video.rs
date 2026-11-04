//! Actual Core-owned ffmpeg pipe capture and independent held-input ffprobe.
use super::native_inspection::revoke;
use super::*;
use crate::runtime::{CompletedVideo, MediaVideoState};
use std::time::Instant;
fn suspended(value: &(u32, u32, bool, String, bool)) {
    assert_ne!(value.0, 0);
    assert_eq!(value.1, 1);
    assert!(!value.2);
    assert!(!value.4);
    assert!(value.3.starts_with("S-1-15-2-"));
}
fn drained(core: &RuntimeCore, run: &str, count: usize) {
    let values = core.media_audio_fixture_observations(run);
    assert_eq!(values.len(), count);
    for value in values {
        assert_ne!(value.0, 0);
        assert_eq!(value.1, 0);
        assert!(value.2);
    }
}
fn complete(core: &mut RuntimeCore, handle: &crate::runtime::MediaVideoHandle) -> CompletedVideo {
    let started = Instant::now();
    loop {
        match core.poll_media_video(handle).unwrap() {
            MediaVideoState::Complete(value) => return value,
            MediaVideoState::Running => {
                assert!(started.elapsed() < Duration::from_secs(10));
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}
#[test]
fn actual_webm_capture_independently_decodes_the_same_fresh_bytes_with_one_charged_call() {
    let mut world = World::new();
    world.grant_limit("consumer", 1);
    let run = world.start_granted("consumer");
    let bytes = video_input::video(3, 16);
    let mut identities = Vec::new();
    let handle = world
        .core
        .start_media_video_fixture(&run, &bytes, |_, value| {
            suspended(&value);
            identities.push((value.0, value.3));
            Ok(())
        })
        .unwrap();
    assert_eq!(world.core.active_media_inspections(), 1);
    let mut committed = false;
    let started = Instant::now();
    while !committed {
        world
            .core
            .tick_media_audio_fixture(
                |_, value| {
                    suspended(&value);
                    identities.push((value.0, value.3));
                    Ok(())
                },
                |_, value| {
                    assert_eq!(value.1, 0);
                    assert!(value.2);
                    committed = true;
                },
            )
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(identities.len(), 2);
    assert_ne!(identities[0].0, identities[1].0);
    assert_ne!(identities[0].1, identities[1].1);
    drained(&world.core, &run, 2);
    let output = complete(&mut world.core, &handle);
    assert_eq!(world.core.active_media_inspections(), 0);
    assert_eq!(output.metadata().formats, vec!["matroska", "webm"]);
    assert_eq!(output.decoded_frames(), &[(0, 3)]);
    assert_eq!(output.metadata().bytes, Some(output.bytes().len() as u64));
    assert!(output.bytes().len() <= 1_048_576);
    assert_eq!(&output.bytes()[..4], &[0x1a, 0x45, 0xdf, 0xa3]);
    assert_eq!(output.metadata().streams[0].codec.as_deref(), Some("vp9"));
    // Synthetic fixture bytes only. Random container IDs require a digest name.
    // Create-only and byte-checked; no user file publication follows.
    let artifact = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        ".generated/owned-video-v1-{}.webm",
        hash(output.bytes())
    ));
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&artifact)
    {
        Ok(mut file) => {
            file.write_all(output.bytes()).unwrap();
            file.sync_all().unwrap();
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => panic!("Synthetic WebM artifact refused: {error}"),
    }
    assert_eq!(super::output(&artifact).0, output.bytes());
    let row = world.row(&run);
    assert_eq!(row["nativeVideo"][0]["phase"], "completed");
    let a = row["nativeVideo"][0]["id"].as_str().unwrap();
    let b = row["nativeVideo"][0]["probeId"].as_str().unwrap();
    assert_ne!(a, b);
    let root = world.path.join("native-media-inspection-v1");
    assert_eq!(fs::read(root.join(a).join("input")).unwrap(), bytes);
    assert_eq!(
        fs::read(root.join(b).join("input")).unwrap(),
        output.bytes()
    );
    for id in [a, b] {
        assert_eq!(fs::read_dir(root.join(id)).unwrap().count(), 1);
        assert!(!root.join(id).join("output.webm").exists());
    }
    assert_eq!(
        world.core.start_media_video(&run, &bytes).err(),
        Some(ErrorCode::BudgetExceeded)
    );
    assert_eq!(world.row(&run)["nativeVideo"].as_array().unwrap().len(), 1);
    world.core.finish(&run, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], false);
}

#[test]
fn cancelling_before_extraction_resume_drains_the_actual_suspended_job() {
    let mut world = World::new();
    let run = world.start("consumer");
    let result =
        world
            .core
            .start_media_video_fixture(&run, &video_input::video(3, 16), |core, value| {
                suspended(&value);
                core.cancel(&run);
                Ok(())
            });
    assert_eq!(result.err(), Some(ErrorCode::Aborted));
    drained(&world.core, &run, 1);
    let row = world.row(&run);
    assert_eq!(row["nativeVideo"][0]["phase"], "failed");
    let probe = row["nativeVideo"][0]["probeId"].as_str().unwrap();
    assert!(!world
        .path
        .join("native-media-inspection-v1")
        .join(probe)
        .exists());
    world.core.finish(&run, Err(ErrorCode::Aborted));
    assert_eq!(world.core.state(&run), Some(JobState::Cancelled));
}
#[test]
fn revocation_before_probe_resume_never_returns_the_captured_video() {
    let mut world = World::new();
    let run = world.start("consumer");
    let admin = world.open("admin");
    let handle = world
        .core
        .start_media_video(&run, &video_input::video(3, 16))
        .unwrap();
    let mut entered = false;
    let started = Instant::now();
    while !entered {
        world
            .core
            .tick_media_audio_fixture(
                |core, value| {
                    suspended(&value);
                    entered = true;
                    revoke(core, admin.clone());
                    Ok(())
                },
                |_, _| panic!("Revoked video commit"),
            )
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    drained(&world.core, &run, 2);
    assert!(matches!(
        world.core.poll_media_video(&handle),
        Err(ErrorCode::GrantRevoked)
    ));
    assert_eq!(world.row(&run)["nativeVideo"][0]["phase"], "failed");
}
#[test]
fn revocation_at_final_video_commit_preserves_the_original_call_budget_and_refuses_output() {
    let mut world = World::new();
    let run = world.start("consumer");
    let admin = world.open("admin");
    let handle = world
        .core
        .start_media_video(&run, &video_input::video(3, 16))
        .unwrap();
    let mut entered = false;
    let started = Instant::now();
    while !entered {
        world
            .core
            .tick_media_audio_fixture(
                |_, value| {
                    suspended(&value);
                    Ok(())
                },
                |core, value| {
                    assert_eq!(value.1, 0);
                    assert!(value.2);
                    entered = true;
                    revoke(core, admin.clone());
                },
            )
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    drained(&world.core, &run, 2);
    assert!(matches!(
        world.core.poll_media_video(&handle),
        Err(ErrorCode::GrantRevoked)
    ));
}
#[test]
fn foreign_sql_before_probe_resume_is_never_overwritten_and_owned_processes_drain() {
    let mut world = World::new();
    let run = world.start("consumer");
    let _handle = world
        .core
        .start_media_video(&run, &video_input::video(3, 16))
        .unwrap();
    let mut entered = false;
    let mut frozen = String::new();
    let started = Instant::now();
    loop {
        let tick = world.core.tick_media_audio_fixture(
            |core, value| {
                suspended(&value);
                entered = true;
                core.with_tool_fixture_store(|store| {
                    store
                        .connection
                        .execute(
                            "UPDATE jobs SET metadata=metadata||' ' WHERE run_id=?1",
                            [&run],
                        )
                        .unwrap();
                    frozen = store
                        .connection
                        .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [&run], |r| {
                            r.get(0)
                        })
                        .unwrap();
                });
                Ok(())
            },
            |_, _| panic!("Foreign video commit"),
        );
        if entered {
            assert_eq!(tick, Err(ErrorCode::StoreCorrupt));
            break;
        }
        tick.unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    drained(&world.core, &run, 2);
    assert_eq!(world.core.active_media_inspections(), 0);
    world.core.finish(&run, Err(ErrorCode::Aborted));
    let actual: String = world.core.with_tool_fixture_store(|store| {
        store
            .connection
            .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [&run], |r| {
                r.get(0)
            })
            .unwrap()
    });
    assert_eq!(actual, frozen);
    let row: Value = serde_json::from_str(&actual).unwrap();
    assert_eq!(row["nativeVideo"][0]["phase"], "preparingProbe");
    assert_eq!(row["toolArtifacts"]["retained"], true);
}
#[test]
fn audio_only_input_cannot_succeed_as_webm_or_create_a_probe_input() {
    let mut world = World::new();
    let run = world.start("consumer");
    let handle = world.core.start_media_video(&run, &wave()).unwrap();
    let started = Instant::now();
    loop {
        world.core.tick_media_inspections().unwrap();
        if world.row(&run)["nativeVideo"][0]["phase"] == "failed" {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    drained(&world.core, &run, 1);
    assert_eq!(
        world.core.poll_media_video(&handle).err(),
        Some(ErrorCode::OutputInvalid)
    );
    let row = world.row(&run);
    let probe = row["nativeVideo"][0]["probeId"].as_str().unwrap();
    assert!(!world
        .path
        .join("native-media-inspection-v1")
        .join(probe)
        .exists());
    assert_eq!(world.core.active_media_inspections(), 0);
}
#[test]
fn cancelling_audio_keeps_the_video_consumer_of_the_same_image_authorized() {
    let mut world = World::new();
    let a = world.start("consumer");
    let b = world.start("other-consumer");
    let first = world.core.start_media_audio(&a, &wave()).unwrap();
    let second = world
        .core
        .start_media_video(&b, &video_input::video(3, 16))
        .unwrap();
    assert_eq!(
        world.row(&a)["toolArtifacts"]["images"],
        world.row(&b)["toolArtifacts"]["images"]
    );
    world.core.cancel(&a);
    drained(&world.core, &a, 1);
    assert!(matches!(
        world.core.poll_media_audio(&first),
        Err(ErrorCode::Aborted)
    ));
    world.core.finish(&a, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&a)["toolArtifacts"]["retained"], false);
    assert_eq!(world.row(&b)["toolArtifacts"]["retained"], true);
    let output = complete(&mut world.core, &second);
    assert_eq!(output.decoded_frames(), &[(0, 3)]);
    world.core.finish(&b, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&b)["toolArtifacts"]["retained"], false);
}
#[test]
fn restart_during_probe_keeps_both_inputs_and_never_replays_a_video_attempt() {
    // Exercise the old deterministic-Base64 background retry exception with a
    // build-owned tool-bearing catalog. Native attempts must always interrupt.
    let mut world = World::new_with_ids(&["plugin-base64-encoder", "other-consumer"]);
    world.grant_options("plugin-base64-encoder", 8, true);
    let run = world.start_granted_options("plugin-base64-encoder", true);
    let handle = world
        .core
        .start_media_video(&run, &video_input::video(3, 16))
        .unwrap();
    let mut prepared = false;
    let started = Instant::now();
    while !prepared {
        world
            .core
            .tick_media_audio_fixture(
                |_, value| {
                    suspended(&value);
                    prepared = true;
                    Ok(())
                },
                |_, _| panic!("Unexpected video completion"),
            )
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    let row = world.row(&run);
    assert_eq!(row["nativeVideo"][0]["phase"], "probing");
    let id = row["nativeVideo"][0]["id"].as_str().unwrap().to_owned();
    let probe = row["nativeVideo"][0]["probeId"]
        .as_str()
        .unwrap()
        .to_owned();
    let root = world.path.join("native-media-inspection-v1");
    let output = fs::read(root.join(&probe).join("input")).unwrap();
    // Positive control uses the same live catalog/epoch/background policy and
    // deadline. Its absent native attempt remains eligible for the old retry.
    let control = world.start_granted_options("plugin-base64-encoder", true);
    world.core = RuntimeCore::validation("unused1".into(), "unused2".into());
    let store = DataStore::open(&world.path.join("runtime.sqlite")).unwrap();
    world.core = RuntimeCore::managed_fixture_tool_catalog(
        HashMap::new(),
        store,
        false,
        world.catalog.clone(),
        world.fixture.trust(),
        Arc::new(AtomicU64::new(world.fixture.clock())),
    )
    .unwrap();
    assert!(matches!(
        world.core.poll_media_video(&handle),
        Err(ErrorCode::InstanceMismatch)
    ));
    assert_eq!(world.core.active_media_inspections(), 0);
    let restored = world.row(&run);
    assert_eq!(world.core.state(&run), Some(JobState::Interrupted));
    assert!(world.core.take_run(&run).is_none());
    assert_eq!(world.core.state(&control), Some(JobState::Queued));
    assert!(world.core.take_run(&control).is_some());
    world.core.finish(&control, Err(ErrorCode::Aborted));
    assert_eq!(restored["nativeVideo"][0]["phase"], "interrupted");
    assert_eq!(restored["toolArtifacts"]["retained"], true);
    assert_eq!(
        fs::read(root.join(&id).join("input")).unwrap(),
        video_input::video(3, 16)
    );
    assert_eq!(fs::read(root.join(&probe).join("input")).unwrap(), output);
    // A forged audio profile in the fixed video domain must fail restore before
    // orphan cleanup, preserving both actual files. Stored profile never chooses
    // the dispatch operation.
    let mut forged = world.row(&run);
    forged["nativeVideo"][0]["profile"] = json!("native-audio-pipe-v1");
    world.core = RuntimeCore::validation("unused3".into(), "unused4".into());
    let store = DataStore::open(&world.path.join("runtime.sqlite")).unwrap();
    store
        .connection
        .execute(
            "UPDATE jobs SET metadata=?1 WHERE run_id=?2",
            rusqlite::params![serde_json::to_string(&forged).unwrap(), &run],
        )
        .unwrap();
    assert_eq!(
        crate::runtime::retained_tool_leases(&store).err(),
        Some(ErrorCode::StoreCorrupt)
    );
    let restored = RuntimeCore::managed_fixture_tool_catalog(
        HashMap::new(),
        store,
        false,
        world.catalog.clone(),
        world.fixture.trust(),
        Arc::new(AtomicU64::new(world.fixture.clock())),
    );
    // Host setup maps the fixed retention reader's failure at its existing
    // artifact boundary. Both exact layers must refuse before cleanup.
    assert_eq!(restored.err(), Some(ErrorCode::DependencyArtifactMismatch));
    assert_eq!(
        fs::read(root.join(id).join("input")).unwrap(),
        video_input::video(3, 16)
    );
    assert_eq!(fs::read(root.join(probe).join("input")).unwrap(), output);
}

// Deterministic AVI with the existing H.264 frames plus synthetic PCM16.
// No new encoder, media download, program image or product input option.
fn audio_video() -> Vec<u8> {
    fn chunk(tag: &[u8; 4], bytes: &[u8]) -> Vec<u8> {
        let mut result = tag.to_vec();
        result.extend((bytes.len() as u32).to_le_bytes());
        result.extend(bytes);
        if !bytes.len().is_multiple_of(2) {
            result.push(0);
        }
        result
    }
    let video = video_input::video(3, 16);
    let audio = wave();
    let header_len = u32::from_le_bytes(video[16..20].try_into().unwrap()) as usize;
    let movi = 20 + header_len + header_len % 2;
    assert_eq!(&video[movi..movi + 4], b"LIST");
    let mut headers = video[20..20 + header_len].to_vec();
    assert_eq!(&headers[..8], b"hdrlavih");
    headers[36..40].copy_from_slice(&2u32.to_le_bytes());
    let mut stream = b"strl".to_vec();
    let mut strh = b"auds\0\0\0\0".to_vec();
    for value in [0u32, 0, 0, 2, 32000, 0, 16000, 32000, u32::MAX, 2] {
        strh.extend(value.to_le_bytes());
    }
    strh.extend([0; 8]);
    assert_eq!(strh.len(), 56);
    stream.extend(chunk(b"strh", &strh));
    stream.extend(chunk(b"strf", &audio[20..36]));
    headers.extend(chunk(b"LIST", &stream));
    let movi_len = u32::from_le_bytes(video[movi + 4..movi + 8].try_into().unwrap()) as usize;
    let mut media = video[movi + 8..movi + 8 + movi_len].to_vec();
    assert_eq!(&media[..4], b"movi");
    media.extend(chunk(b"01wb", &audio[44..]));
    let mut data = b"AVI ".to_vec();
    data.extend(chunk(b"LIST", &headers));
    data.extend(chunk(b"LIST", &media));
    chunk(b"RIFF", &data)
}
#[test]
fn actual_optional_audio_is_encoded_as_opus_and_every_output_stream_decodes() {
    let mut world = World::new();
    world.grant_limit("consumer", 1);
    let run = world.start_granted("consumer");
    let bytes = audio_video();
    let handle = world.core.start_media_video(&run, &bytes).unwrap();
    let started = Instant::now();
    while world.row(&run)["nativeVideo"][0]["phase"] != "completed" {
        world.core.tick_media_inspections().unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    drained(&world.core, &run, 2);
    let output = complete(&mut world.core, &handle);
    assert_eq!(output.metadata().streams.len(), 2);
    assert_eq!(output.metadata().streams[0].codec.as_deref(), Some("vp9"));
    assert_eq!(output.metadata().streams[1].codec.as_deref(), Some("opus"));
    assert_eq!(output.metadata().streams[1].sample_rate, Some(48000));
    assert_eq!(output.metadata().streams[1].channels, Some(1));
    assert_eq!(output.decoded_frames()[0], (0, 3));
    assert!(output.decoded_frames()[1].1 > 0);
    assert_eq!(output.decoded_frames()[1].0, 1);
    assert_eq!(output.metadata().bytes, Some(output.bytes().len() as u64));
    let row = world.row(&run);
    let probe = row["nativeVideo"][0]["probeId"].as_str().unwrap();
    assert_eq!(
        fs::read(
            world
                .path
                .join("native-media-inspection-v1")
                .join(probe)
                .join("input")
        )
        .unwrap(),
        output.bytes()
    );
    assert_eq!(
        world.core.start_media_video(&run, &bytes).err(),
        Some(ErrorCode::BudgetExceeded)
    );
    world.core.finish(&run, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], false);
}
