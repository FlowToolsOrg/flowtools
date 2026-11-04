//! Actual Core-owned ffmpeg pipe capture and independent held-input ffprobe.
use super::native_inspection::revoke;
use super::*;
use crate::runtime::{CompletedAudio, MediaAudioState};
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
fn complete(core: &mut RuntimeCore, handle: &crate::runtime::MediaAudioHandle) -> CompletedAudio {
    let started = Instant::now();
    loop {
        match core.poll_media_audio(handle).unwrap() {
            MediaAudioState::Complete(value) => return value,
            MediaAudioState::Running => {
                assert!(started.elapsed() < Duration::from_secs(10));
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}
#[test]
fn actual_wav_capture_probes_the_same_fresh_bytes_with_one_charged_call() {
    let mut world = World::new();
    world.grant_limit("consumer", 1);
    let run = world.start_granted("consumer");
    let bytes = wave();
    let mut identities = Vec::new();
    let handle = world
        .core
        .start_media_audio_fixture(&run, &bytes, |_, value| {
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
    assert_eq!(output.metadata().formats, vec!["wav"]);
    assert_eq!(output.metadata().duration_ms, Some(1000));
    assert_eq!(output.metadata().bytes, Some(output.bytes().len() as u64));
    assert!(output.bytes().len() <= 1_048_576);
    assert_eq!(&output.bytes()[..4], b"RIFF");
    // Synthetic test output only, for independent pinned-browser decode checks.
    // Create-only and exact existing bytes: never overwrite an unknown artifact.
    let artifact = Path::new(env!("CARGO_MANIFEST_DIR")).join(".generated/owned-audio-v1.wav");
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
        Err(error) => panic!("Synthetic WAV artifact refused: {error}"),
    }
    assert_eq!(super::output(&artifact).0, output.bytes());
    let row = world.row(&run);
    assert_eq!(row["nativeAudio"][0]["phase"], "completed");
    let a = row["nativeAudio"][0]["id"].as_str().unwrap();
    let b = row["nativeAudio"][0]["probeId"].as_str().unwrap();
    assert_ne!(a, b);
    let root = world.path.join("native-media-inspection-v1");
    assert_eq!(fs::read(root.join(a).join("input")).unwrap(), bytes);
    assert_eq!(
        fs::read(root.join(b).join("input")).unwrap(),
        output.bytes()
    );
    for id in [a, b] {
        assert_eq!(fs::read_dir(root.join(id)).unwrap().count(), 1);
        assert!(!root.join(id).join("output.wav").exists());
    }
    assert_eq!(
        world.core.start_media_audio(&run, &bytes).err(),
        Some(ErrorCode::BudgetExceeded)
    );
    assert_eq!(world.row(&run)["nativeAudio"].as_array().unwrap().len(), 1);
    world.core.finish(&run, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], false);
}

#[test]
fn cancelling_before_extraction_resume_drains_the_actual_suspended_job() {
    let mut world = World::new();
    let run = world.start("consumer");
    let result = world
        .core
        .start_media_audio_fixture(&run, &wave(), |core, value| {
            suspended(&value);
            core.cancel(&run);
            Ok(())
        });
    assert_eq!(result.err(), Some(ErrorCode::Aborted));
    drained(&world.core, &run, 1);
    let row = world.row(&run);
    assert_eq!(row["nativeAudio"][0]["phase"], "failed");
    let probe = row["nativeAudio"][0]["probeId"].as_str().unwrap();
    assert!(!world
        .path
        .join("native-media-inspection-v1")
        .join(probe)
        .exists());
    world.core.finish(&run, Err(ErrorCode::Aborted));
    assert_eq!(world.core.state(&run), Some(JobState::Cancelled));
}
#[test]
fn revocation_before_probe_resume_never_returns_the_captured_audio() {
    let mut world = World::new();
    let run = world.start("consumer");
    let admin = world.open("admin");
    let handle = world.core.start_media_audio(&run, &wave()).unwrap();
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
                |_, _| panic!("Revoked audio commit"),
            )
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    drained(&world.core, &run, 2);
    assert!(matches!(
        world.core.poll_media_audio(&handle),
        Err(ErrorCode::GrantRevoked)
    ));
    assert_eq!(world.row(&run)["nativeAudio"][0]["phase"], "failed");
}
#[test]
fn revocation_at_final_audio_commit_preserves_the_original_call_budget_and_refuses_output() {
    let mut world = World::new();
    let run = world.start("consumer");
    let admin = world.open("admin");
    let handle = world.core.start_media_audio(&run, &wave()).unwrap();
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
        world.core.poll_media_audio(&handle),
        Err(ErrorCode::GrantRevoked)
    ));
}
#[test]
fn foreign_sql_before_probe_resume_is_never_overwritten_and_owned_processes_drain() {
    let mut world = World::new();
    let run = world.start("consumer");
    let _handle = world.core.start_media_audio(&run, &wave()).unwrap();
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
            |_, _| panic!("Foreign audio commit"),
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
    assert_eq!(row["nativeAudio"][0]["phase"], "preparingProbe");
    assert_eq!(row["toolArtifacts"]["retained"], true);
}
#[test]
fn wav_header_overhead_over_the_hard_pipe_limit_never_creates_a_probe_input() {
    let mut world = World::new();
    let run = world.start("consumer");
    // The pinned build supports pcm_s16le, not pcm_u8. Its fixed WAV
    // header/encoder fields take a maximal supported input over the pipe limit.
    let mut bytes = wave();
    bytes.resize(1_048_576, 0);
    bytes[4..8].copy_from_slice(&1_048_568u32.to_le_bytes());
    bytes[40..44].copy_from_slice(&1_048_532u32.to_le_bytes());
    assert_eq!(bytes.len(), 1_048_576);
    let handle = world.core.start_media_audio(&run, &bytes).unwrap();
    let started = Instant::now();
    loop {
        world.core.tick_media_inspections().unwrap();
        if world.row(&run)["nativeAudio"][0]["phase"] == "failed" {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    drained(&world.core, &run, 1);
    assert_eq!(
        world.core.poll_media_audio(&handle).err(),
        Some(ErrorCode::BudgetExceeded)
    );
    let row = world.row(&run);
    assert_eq!(row["nativeAudio"][0]["phase"], "failed");
    let probe = row["nativeAudio"][0]["probeId"].as_str().unwrap();
    assert!(!world
        .path
        .join("native-media-inspection-v1")
        .join(probe)
        .exists());
    assert_eq!(world.core.active_media_inspections(), 0);
}
#[test]
fn cancelling_one_audio_consumer_keeps_the_other_shared_tool_call_authorized() {
    let mut world = World::new();
    let a = world.start("consumer");
    let b = world.start("other-consumer");
    let first = world.core.start_media_audio(&a, &wave()).unwrap();
    let second = world.core.start_media_audio(&b, &wave()).unwrap();
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
    assert_eq!(output.metadata().duration_ms, Some(1000));
    world.core.finish(&b, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&b)["toolArtifacts"]["retained"], false);
}
#[test]
fn restart_during_probe_keeps_both_inputs_and_never_replays_an_audio_attempt() {
    // Exercise the old deterministic-Base64 background retry exception with a
    // build-owned tool-bearing catalog. Native attempts must always interrupt.
    let mut world = World::new_with_ids(&["plugin-base64-encoder", "other-consumer"]);
    world.grant_options("plugin-base64-encoder", 8, true);
    let run = world.start_granted_options("plugin-base64-encoder", true);
    let handle = world.core.start_media_audio(&run, &wave()).unwrap();
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
                |_, _| panic!("Unexpected audio completion"),
            )
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    let row = world.row(&run);
    assert_eq!(row["nativeAudio"][0]["phase"], "probing");
    let id = row["nativeAudio"][0]["id"].as_str().unwrap().to_owned();
    let probe = row["nativeAudio"][0]["probeId"]
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
        world.core.poll_media_audio(&handle),
        Err(ErrorCode::InstanceMismatch)
    ));
    assert_eq!(world.core.active_media_inspections(), 0);
    let restored = world.row(&run);
    assert_eq!(world.core.state(&run), Some(JobState::Interrupted));
    assert!(world.core.take_run(&run).is_none());
    assert_eq!(world.core.state(&control), Some(JobState::Queued));
    assert!(world.core.take_run(&control).is_some());
    world.core.finish(&control, Err(ErrorCode::Aborted));
    assert_eq!(restored["nativeAudio"][0]["phase"], "interrupted");
    assert_eq!(restored["toolArtifacts"]["retained"], true);
    assert_eq!(fs::read(root.join(id).join("input")).unwrap(), wave());
    assert_eq!(fs::read(root.join(probe).join("input")).unwrap(), output);
}
