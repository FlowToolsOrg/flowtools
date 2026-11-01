//! Exercise the actual Core-owned runner, separately from the old test launcher.
use super::*;
use crate::runtime::MediaInspectionState;
use std::time::Instant;
fn revoke(core: &mut RuntimeCore, admin: SessionProof) {
    let response = core.handle(
        "admin",
        request(
            Call::Revoke(PermissionKey {
                plugin_id: "consumer".into(),
                command_id: "convert".into(),
                target: GrantTarget::Cli,
            }),
            Some(admin),
        ),
    );
    assert!(
        matches!(response.outcome, Outcome::Permissions(_)),
        "{response:?}"
    );
}
fn suspended(observed: &(u32, u32, bool, String, bool)) {
    assert_ne!(observed.0, 0);
    assert_eq!(observed.1, 1);
    assert!(!observed.2);
    assert!(observed.3.starts_with("S-1-15-2-"));
    assert!(!observed.4);
}
fn drained(core: &RuntimeCore, run: &str) {
    let observations = core.media_inspection_fixture_observations(run);
    assert_eq!(observations.len(), 1);
    assert_ne!(observations[0].0, 0);
    assert_eq!(observations[0].1, 0);
    assert!(observations[0].2);
}
#[test]
fn actual_core_runner_inspects_fresh_private_input_and_commits_only_projection() {
    let mut world = World::new();
    world.grant_limit("consumer", 1);
    let run = world.start_granted("consumer");
    let bytes = wave();
    let handle = world.core.start_media_inspection(&run, &bytes).unwrap();
    assert_eq!(world.core.active_media_inspections(), 1);
    let row = world.row(&run);
    assert_eq!(row["nativeInspections"][0]["phase"], "running");
    let id = row["nativeInspections"][0]["id"].as_str().unwrap();
    let input = world
        .path
        .join("native-media-inspection-v1")
        .join(id)
        .join("input");
    assert_eq!(fs::read(&input).unwrap(), bytes);
    let started = Instant::now();
    let result = loop {
        match world.core.poll_media_inspection(&handle).unwrap() {
            MediaInspectionState::Running => {
                assert!(started.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(1));
            }
            MediaInspectionState::Complete(result) => break result,
        }
    };
    assert_eq!(result.formats, vec!["wav"]);
    assert_eq!(world.core.active_media_inspections(), 0);
    assert_eq!(result.duration_ms, Some(1000));
    assert_eq!(result.streams.len(), 1);
    assert_eq!(result.streams[0].sample_rate, Some(16_000));
    assert_eq!(
        world.row(&run)["nativeInspections"][0]["phase"],
        "completed"
    );
    assert_eq!(
        world.core.start_media_inspection(&run, &bytes).err(),
        Some(ErrorCode::BudgetExceeded)
    );
    assert_eq!(
        world.row(&run)["nativeInspections"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(matches!(
        world.core.poll_media_inspection(&handle),
        Err(ErrorCode::SessionInvalid)
    ));
    world.core.finish(&run, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], false);
    assert_eq!(fs::read(input).unwrap(), bytes);
}

#[test]
fn two_consumers_get_distinct_suspended_lpac_jobs_and_independent_release() {
    let mut world = World::new();
    let first = world.start("consumer");
    let second = world.start("other-consumer");
    let mut identities = Vec::new();
    let a = world
        .core
        .start_media_inspection_fixture(&first, &wave(), |_, observed| {
            suspended(&observed);
            identities.push((observed.0, observed.3));
            Ok(())
        })
        .unwrap();
    let b = world
        .core
        .start_media_inspection_fixture(&second, &wave(), |_, observed| {
            suspended(&observed);
            identities.push((observed.0, observed.3));
            Ok(())
        })
        .unwrap();
    assert_ne!(identities[0].0, identities[1].0);
    assert_ne!(identities[0].1, identities[1].1);
    assert_eq!(
        world.row(&first)["toolArtifacts"]["images"],
        world.row(&second)["toolArtifacts"]["images"]
    );
    world.core.finish(&first, Ok(json!("fixture")));
    assert_eq!(world.core.state(&first), Some(JobState::Failed));
    drained(&world.core, &first);
    assert_eq!(world.row(&first)["toolArtifacts"]["retained"], false);
    assert_eq!(world.row(&second)["toolArtifacts"]["retained"], true);
    assert!(matches!(
        world.core.poll_media_inspection(&a),
        Err(ErrorCode::Aborted)
    ));
    let started = Instant::now();
    loop {
        match world.core.poll_media_inspection(&b).unwrap() {
            MediaInspectionState::Complete(_) => break,
            MediaInspectionState::Running => {
                assert!(started.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
    world.core.finish(&second, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&second)["toolArtifacts"]["retained"], false);
}
#[test]
fn revocation_before_resume_drains_the_actual_suspended_owned_process() {
    let mut world = World::new();
    let run = world.start("consumer");
    let admin = world.open("admin");
    let result = world
        .core
        .start_media_inspection_fixture(&run, &wave(), |core, observed| {
            suspended(&observed);
            revoke(core, admin);
            Ok(())
        });
    assert_eq!(result.err(), Some(ErrorCode::GrantRevoked));
    drained(&world.core, &run);
    assert_eq!(world.row(&run)["nativeInspections"][0]["phase"], "failed");
    world.core.finish(&run, Err(ErrorCode::GrantRevoked));
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], false);
}
#[test]
fn unknown_sql_after_prepare_preserves_exact_foreign_bytes_and_stops_admission() {
    let mut world = World::new();
    let run = world.start("consumer");
    let mut substituted = String::new();
    let result = world
        .core
        .start_media_inspection_fixture(&run, &wave(), |core, observed| {
            suspended(&observed);
            core.with_tool_fixture_store(|store| {
                store
                    .connection
                    .execute(
                        "UPDATE jobs SET metadata=metadata||' ' WHERE run_id=?1",
                        [&run],
                    )
                    .unwrap();
                substituted = store
                    .connection
                    .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [&run], |r| {
                        r.get(0)
                    })
                    .unwrap();
            });
            Ok(())
        });
    assert_eq!(result.err(), Some(ErrorCode::StoreCorrupt));
    drained(&world.core, &run);
    assert!(matches!(
        world.core.check_run(&run),
        Err(ErrorCode::Aborted)
    ));
    world.core.finish(&run, Err(ErrorCode::Aborted));
    let current: String = world.core.with_tool_fixture_store(|store| {
        store
            .connection
            .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [&run], |r| {
                r.get(0)
            })
            .unwrap()
    });
    assert_eq!(current, substituted);
    let row: Value = serde_json::from_str(&current).unwrap();
    assert_eq!(row["toolArtifacts"]["retained"], true);
    assert_eq!(row["nativeInspections"][0]["phase"], "preparing");
}
#[test]
fn current_revocation_at_projection_commit_never_returns_the_probe_result() {
    let mut world = World::new();
    let run = world.start("consumer");
    let admin = world.open("admin");
    let handle = world.core.start_media_inspection(&run, &wave()).unwrap();
    let mut entered = false;
    let started = Instant::now();
    while !entered {
        world
            .core
            .tick_media_inspections_fixture(|core| {
                entered = true;
                revoke(core, admin.clone());
            })
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    drained(&world.core, &run);
    assert!(matches!(
        world.core.poll_media_inspection(&handle),
        Err(ErrorCode::GrantRevoked)
    ));
    assert_eq!(world.row(&run)["nativeInspections"][0]["phase"], "failed");
}
#[test]
fn restart_keeps_interrupted_attempt_input_and_tool_roots_without_replay() {
    let mut world = World::new();
    let run = world.start("consumer");
    let handle = world.core.start_media_inspection(&run, &wave()).unwrap();
    let row = world.row(&run);
    let id = row["nativeInspections"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    world.core = RuntimeCore::validation("unused1".into(), "unused2".into());
    let store = DataStore::open(&world.path.join("runtime.sqlite")).unwrap();
    world.core = RuntimeCore::managed_fixture_tool_catalog(
        HashMap::from([
            ("cli".into(), "local-cli".into()),
            ("admin".into(), "local-manager".into()),
        ]),
        store,
        false,
        world.catalog.clone(),
        world.fixture.trust(),
        Arc::new(AtomicU64::new(world.fixture.clock())),
    )
    .unwrap();
    let restored = world.row(&run);
    assert_eq!(restored["nativeInspections"][0]["id"], id);
    assert_eq!(restored["nativeInspections"][0]["phase"], "interrupted");
    assert_eq!(restored["toolArtifacts"]["retained"], true);
    assert_eq!(world.core.state(&run), Some(JobState::Interrupted));
    assert!(matches!(
        world.core.poll_media_inspection(&handle),
        Err(ErrorCode::InstanceMismatch)
    ));
    assert!(world
        .core
        .media_inspection_fixture_observations(&run)
        .is_empty());
    assert_eq!(
        fs::read(
            world
                .path
                .join("native-media-inspection-v1")
                .join(id)
                .join("input")
        )
        .unwrap(),
        wave()
    );
}
#[test]
fn invalid_inputs_and_unknown_workspaces_fail_before_native_dispatch() {
    let mut world = World::new();
    let run = world.start("consumer");
    assert_eq!(
        world.core.start_media_inspection(&run, &[]).err(),
        Some(ErrorCode::InputInvalid)
    );
    assert_eq!(
        world
            .core
            .start_media_inspection(&run, &vec![0; 1_048_577])
            .err(),
        Some(ErrorCode::InputInvalid)
    );
    assert!(world.row(&run).get("nativeInspections").is_none());
    let handle = world.core.start_media_inspection(&run, &wave()).unwrap();
    let started = Instant::now();
    loop {
        match world.core.poll_media_inspection(&handle).unwrap() {
            MediaInspectionState::Complete(_) => break,
            MediaInspectionState::Running => {
                assert!(started.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
    let namespace = world.path.join("native-media-inspection-v1");
    fs::write(namespace.join("foreign"), b"KEEP_NATIVE_FOREIGN_BYTES").unwrap();
    assert_eq!(
        world.core.start_media_inspection(&run, &wave()).err(),
        Some(ErrorCode::RecoveryPending)
    );
    assert_eq!(
        fs::read(namespace.join("foreign")).unwrap(),
        b"KEEP_NATIVE_FOREIGN_BYTES"
    );
    assert!(world
        .core
        .media_inspection_fixture_observations(&run)
        .is_empty());
    assert_eq!(
        world.row(&run)["nativeInspections"][1]["phase"],
        "interrupted"
    );
}
#[test]
fn signed_tool_revocation_while_inflight_drains_and_refuses_completion() {
    let mut world = World::new();
    let run = world.start("consumer");
    let handle = world.core.start_media_inspection(&run, &wave()).unwrap();
    let mut root = world.fixture.root.clone();
    root.version += 1;
    let trust = world.fixture.trust();
    root.previous_root_sha256 = Some(
        world
            .core
            .with_tool_fixture_store(|store| trust.snapshot(store).unwrap().root_digest),
    );
    root.revoked_package_digests
        .push(world.fixture.package.archive.sha256.clone());
    let clock = world.fixture.clock();
    world.core.with_tool_fixture_store(|store| {
        trust
            .rotate(store, &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]), clock)
            .unwrap()
    });
    world.core.tick_media_inspections().unwrap();
    drained(&world.core, &run);
    assert!(matches!(
        world.core.poll_media_inspection(&handle),
        Err(ErrorCode::DependencyArtifactMismatch)
    ));
    assert_eq!(world.row(&run)["nativeInspections"][0]["phase"], "failed");
    assert!(world.image.is_dir());
}

#[test]
fn corrupt_native_retention_refuses_restore_and_preserves_private_input() {
    let mut world = World::new();
    let run = world.start("consumer");
    let _handle = world.core.start_media_inspection(&run, &wave()).unwrap();
    let row = world.row(&run);
    let id = row["nativeInspections"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    world.core = RuntimeCore::validation("unused1".into(), "unused2".into());
    let store = DataStore::open(&world.path.join("runtime.sqlite")).unwrap();
    let mut corrupt = row;
    corrupt["nativeInspections"][0]["inputDigest"] = json!("corrupt");
    let bytes = serde_json::to_string(&corrupt).unwrap();
    store
        .connection
        .execute(
            "UPDATE jobs SET metadata=?1 WHERE run_id=?2",
            rusqlite::params![bytes, run],
        )
        .unwrap();
    let restored = RuntimeCore::managed_fixture_tool_catalog(
        HashMap::new(),
        store,
        false,
        world.catalog.clone(),
        world.fixture.trust(),
        Arc::new(AtomicU64::new(world.fixture.clock())),
    );
    // ToolAuthority's catalog setup validates retained roots before Runtime
    // restores jobs; its existing stable boundary maps corrupt roots to this
    // artifact refusal. No admission or cleanup follows it.
    assert_eq!(restored.err(), Some(ErrorCode::DependencyArtifactMismatch));
    let store = DataStore::open(&world.path.join("runtime.sqlite")).unwrap();
    let actual: String = store
        .connection
        .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [&run], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(actual, bytes);
    assert_eq!(
        fs::read(
            world
                .path
                .join("native-media-inspection-v1")
                .join(id)
                .join("input")
        )
        .unwrap(),
        wave()
    );
    assert!(world.image.is_dir());
}
#[test]
fn late_poll_keeps_the_original_five_second_deadline_and_owned_job_drain() {
    let mut world = World::new();
    let run = world.start("consumer");
    let handle = world.core.start_media_inspection(&run, &wave()).unwrap();
    std::thread::sleep(Duration::from_millis(5010));
    world.core.tick_media_inspections().unwrap();
    drained(&world.core, &run);
    assert!(matches!(
        world.core.poll_media_inspection(&handle),
        Err(ErrorCode::Timeout)
    ));
    assert_eq!(world.row(&run)["nativeInspections"][0]["phase"], "failed");
}
