//! Actual fixed images across a signed release-floor advance. No new run grant.
use super::*;
use crate::packages::trust::DescriptorVerification;
use crate::runtime::{MediaAudioState, MediaInspectionState, MediaVideoState};
use std::time::Instant;
pub(super) struct Newer {
    package: PackagePayload,
    manifest: Vec<u8>,
    archive: Vec<u8>,
    envelope: Vec<u8>,
}
pub(super) fn newer(world: &World) -> Newer {
    let mut package = world.fixture.package.clone();
    package.version = "2.0.0".into();
    package.release_sequence += 1;
    let mut manifest: Value = serde_json::from_slice(&world.fixture.manifest).unwrap();
    manifest["version"] = json!("2.0.0");
    let manifest = serde_json::to_vec(&manifest).unwrap();
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (path, bytes) in [
        ("manifest.json", manifest.as_slice()),
        ("bin/ffmpeg.exe", FFMPEG),
        ("bin/ffprobe.exe", FFPROBE),
        ("licenses/ffmpeg.txt", NOTICES),
    ] {
        zip.start_file(
            path,
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(bytes).unwrap();
    }
    let archive = zip.finish().unwrap().into_inner();
    package.manifest = BlobDescriptor {
        size: manifest.len() as u64,
        sha256: hash(&manifest),
    };
    package.archive.size = archive.len() as u64;
    package.archive.sha256 = hash(&archive);
    let envelope = signed(PACKAGE_PAYLOAD_TYPE, &package, &[3]);
    Newer {
        package,
        manifest,
        archive,
        envelope,
    }
}
pub(super) fn advance(
    core: &mut RuntimeCore,
    trust: &PackageTrustStore,
    release: &Newer,
    time: u64,
) {
    core.with_tool_fixture_store(|store| {
        trust
            .verify_descriptor(
                store,
                DescriptorVerification {
                    envelope: &release.envelope,
                    manifest: &release.manifest,
                    archive: &release.archive,
                    target: release.package.target.clone(),
                    now: time,
                },
            )
            .unwrap();
    });
}
fn drained(core: &RuntimeCore, run: &str, audio: bool, count: usize) {
    let observations = if audio {
        core.media_audio_fixture_observations(run)
    } else {
        core.media_inspection_fixture_observations(run)
    };
    assert_eq!(observations.len(), count);
    for observed in observations {
        assert_ne!(observed.0, 0);
        assert_eq!(observed.1, 0);
        assert!(observed.2);
    }
}
fn deadline(started: Instant) {
    assert!(started.elapsed() < Duration::from_secs(10));
    std::thread::sleep(Duration::from_millis(1));
}
#[test]
fn live_inspection_finishes_after_floor_advance_but_a_new_call_still_refuses_the_old_release() {
    let mut world = World::new();
    let release = newer(&world);
    let trust = world.fixture.trust();
    let time = world.fixture.clock();
    let run = world.start("consumer");
    let handle = world
        .core
        .start_media_inspection_fixture(&run, &wave(), |core, observed| {
            assert_eq!(observed.1, 1);
            assert!(!observed.2);
            assert!(!observed.4);
            advance(core, &trust, &release, time);
            Ok(())
        })
        .unwrap();
    let started = Instant::now();
    while world.row(&run)["nativeInspections"][0]["phase"] != "completed" {
        world.core.tick_media_inspections().unwrap();
        deadline(started);
    }
    drained(&world.core, &run, false, 1);
    let MediaInspectionState::Complete(output) = world.core.poll_media_inspection(&handle).unwrap()
    else {
        panic!("Incomplete old inspection")
    };
    assert_eq!(output.duration_ms, Some(1000));
    assert_eq!(
        world.core.start_media_inspection(&run, &wave()).err(),
        Some(ErrorCode::DependencyArtifactMismatch)
    );
    assert_eq!(
        world.row(&run)["nativeInspections"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    world.core.finish(&run, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], false);
}
#[test]
fn live_wav_probe_continues_but_another_old_accepted_root_cannot_start_a_fresh_call() {
    let mut world = World::new();
    let release = newer(&world);
    let trust = world.fixture.trust();
    let time = world.fixture.clock();
    let run = world.start("consumer");
    let other = world.start("other-consumer");
    let handle = world.core.start_media_audio(&run, &wave()).unwrap();
    let mut advanced = false;
    let started = Instant::now();
    while world.row(&run)["nativeAudio"][0]["phase"] != "completed" {
        world
            .core
            .tick_media_audio_fixture(
                |core, observed| {
                    assert_eq!(observed.1, 1);
                    assert!(!observed.2);
                    assert!(!advanced);
                    advance(core, &trust, &release, time);
                    advanced = true;
                    Ok(())
                },
                |_, observed| {
                    assert_eq!(observed.1, 0);
                    assert!(observed.2);
                },
            )
            .unwrap();
        deadline(started);
    }
    assert!(advanced);
    drained(&world.core, &run, true, 2);
    let MediaAudioState::Complete(output) = world.core.poll_media_audio(&handle).unwrap() else {
        panic!("Incomplete old audio")
    };
    assert_eq!(output.metadata().duration_ms, Some(1000));
    assert_eq!(
        world.core.start_media_audio(&other, &wave()).err(),
        Some(ErrorCode::DependencyArtifactMismatch)
    );
    assert!(world.row(&other)["nativeAudio"].is_null());
}
#[test]
fn live_webm_commit_and_collection_keep_exact_bytes_across_a_release_floor_change() {
    let mut world = World::new();
    let release = newer(&world);
    let trust = world.fixture.trust();
    let time = world.fixture.clock();
    let run = world.start("consumer");
    let handle = world
        .core
        .start_media_video(&run, &video_input::video(3, 16))
        .unwrap();
    let mut advanced = false;
    let started = Instant::now();
    while world.row(&run)["nativeVideo"][0]["phase"] != "completed" {
        world
            .core
            .tick_media_audio_fixture(
                |_, _| Ok(()),
                |core, observed| {
                    assert_eq!(observed.1, 0);
                    assert!(observed.2);
                    assert!(!advanced);
                    advance(core, &trust, &release, time);
                    advanced = true;
                },
            )
            .unwrap();
        deadline(started);
    }
    assert!(advanced);
    drained(&world.core, &run, true, 2);
    let MediaVideoState::Complete(output) = world.core.poll_media_video(&handle).unwrap() else {
        panic!("Incomplete old video")
    };
    assert_eq!(output.decoded_frames(), &[(0, 3)]);
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
}
#[test]
fn live_continuity_never_overrides_a_revoked_current_grant() {
    let mut world = World::new();
    let release = newer(&world);
    let trust = world.fixture.trust();
    let time = world.fixture.clock();
    let run = world.start("consumer");
    let admin = world.open("admin");
    let result = world
        .core
        .start_media_inspection_fixture(&run, &wave(), |core, _| {
            advance(core, &trust, &release, time);
            super::native_inspection::revoke(core, admin);
            Ok(())
        });
    assert_eq!(result.err(), Some(ErrorCode::GrantRevoked));
    drained(&world.core, &run, false, 1);
}
#[test]
fn signed_package_revocation_after_floor_advance_still_drains_the_old_live_call() {
    let mut world = World::new();
    let release = newer(&world);
    let trust = world.fixture.trust();
    let time = world.fixture.clock();
    let run = world.start("consumer");
    let mut root = world.fixture.root.clone();
    let old = world.fixture.package.archive.sha256.clone();
    let result = world
        .core
        .start_media_inspection_fixture(&run, &wave(), |core, _| {
            advance(core, &trust, &release, time);
            core.with_tool_fixture_store(|store| {
                root.version += 1;
                root.previous_root_sha256 = Some(trust.snapshot(store).unwrap().root_digest);
                root.revoked_package_digests.push(old);
                trust
                    .rotate(store, &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]), time)
                    .unwrap();
            });
            Ok(())
        });
    assert_eq!(result.err(), Some(ErrorCode::DependencyArtifactMismatch));
    drained(&world.core, &run, false, 1);
}
#[test]
fn reopening_after_a_floor_change_has_no_live_proof_and_preserves_both_private_inputs() {
    let mut world = World::new();
    let release = newer(&world);
    let trust = world.fixture.trust();
    let time = world.fixture.clock();
    let run = world.start("consumer");
    let _handle = world.core.start_media_audio(&run, &wave()).unwrap();
    let mut advanced = false;
    let started = Instant::now();
    while !advanced {
        world
            .core
            .tick_media_audio_fixture(
                |core, _| {
                    advance(core, &trust, &release, time);
                    advanced = true;
                    Ok(())
                },
                |_, _| panic!("Unexpected completion"),
            )
            .unwrap();
        deadline(started);
    }
    let row = world.row(&run);
    assert_eq!(row["nativeAudio"][0]["phase"], "probing");
    let a = row["nativeAudio"][0]["id"].as_str().unwrap().to_owned();
    let b = row["nativeAudio"][0]["probeId"]
        .as_str()
        .unwrap()
        .to_owned();
    let root = world.path.join("native-media-inspection-v1");
    let bytes = fs::read(root.join(&b).join("input")).unwrap();
    world.core = RuntimeCore::validation("unused1".into(), "unused2".into());
    let store = DataStore::open(&world.path.join("runtime.sqlite")).unwrap();
    let restored = RuntimeCore::managed_fixture_tool_catalog(
        HashMap::new(),
        store,
        false,
        world.catalog.clone(),
        world.fixture.trust(),
        Arc::new(AtomicU64::new(time)),
    );
    assert_eq!(restored.err(), Some(ErrorCode::DependencyArtifactMismatch));
    assert_eq!(fs::read(root.join(a).join("input")).unwrap(), wave());
    assert_eq!(fs::read(root.join(b).join("input")).unwrap(), bytes);
}
