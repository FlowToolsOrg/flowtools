//! Explicit disposition uses real signed installation and the original writer.
use super::*;
use native_queue::enqueue;

fn interrupted() -> (World, String) {
    let mut world = World::new();
    world.grant("consumer");
    let run = enqueue(&mut world, "consumer", false);
    let release = native_continuation::newer(&world);
    native_cohort::install(&mut world, &release, true);
    assert!(world.core.take_run(&run).is_none());
    assert_eq!(world.core.state(&run), Some(JobState::Interrupted));
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], true);
    (world, run)
}
fn input(world: &World, run: &str) -> Vec<u8> {
    fs::read(
        world
            .path
            .join("private-jobs")
            .join(format!("{run}.input.bin")),
    )
    .unwrap()
}
fn raw(world: &mut World, run: &str) -> String {
    world.core.with_tool_fixture_store(|store| {
        store
            .connection
            .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [run], |row| {
                row.get(0)
            })
            .unwrap()
    })
}
fn roots(world: &mut World) -> usize {
    world
        .core
        .with_tool_fixture_store(|store| crate::runtime::retained_tool_leases(store).unwrap().len())
}
fn reopen(world: &mut World) {
    world.core = RuntimeCore::validation("unused1".into(), "unused2".into());
    world.core = RuntimeCore::managed_fixture_tool_catalog(
        HashMap::from([
            ("cli".into(), "local-cli".into()),
            ("admin".into(), "local-manager".into()),
        ]),
        DataStore::open(&world.path.join("runtime.sqlite")).unwrap(),
        false,
        world.catalog.clone(),
        world.fixture.trust(),
        Arc::new(AtomicU64::new(world.fixture.clock())),
    )
    .unwrap();
    world.sessions.clear();
}
fn upgrade_catalog(world: &mut World) {
    // Startup retains ordinary current-floor catalog admission. Explicitly
    // commit the already installed newer release before reopening the Host.
    let release = native_continuation::newer(world);
    let catalog = native_cohort::candidate(world, &release);
    let plan = world.core.stage_provider_cohort(catalog.clone()).unwrap();
    world
        .core
        .begin_provider_cohort(&plan, plan.digest())
        .unwrap();
    world
        .core
        .commit_provider_cohort(&plan, |_| Ok(()))
        .unwrap();
    world.catalog = catalog;
}

#[test]
fn ordinary_queued_running_and_confirmed_terminal_jobs_cannot_use_interrupted_disposition() {
    let mut world = World::new();
    world.grant("consumer");
    let run = enqueue(&mut world, "consumer", false);
    let original = raw(&mut world, &run);
    assert_eq!(
        world.core.plan_queued_tool_discard(&run).err(),
        Some(ErrorCode::InvalidRequest)
    );
    assert_eq!(raw(&mut world, &run), original);
    assert!(world.core.take_run(&run).is_some());
    assert_eq!(
        world.core.plan_queued_tool_discard(&run).err(),
        Some(ErrorCode::InvalidRequest)
    );
    world.core.finish(&run, Err(ErrorCode::Aborted));
    assert_eq!(
        world.core.plan_queued_tool_discard(&run).err(),
        Some(ErrorCode::InvalidRequest)
    );
}

#[test]
fn exact_consent_releases_only_the_never_started_queue_and_preserves_original_private_bytes() {
    let mut world = World::new();
    world.grant("consumer");
    world.grant("other-consumer");
    let a = enqueue(&mut world, "consumer", false);
    let b = enqueue(&mut world, "other-consumer", false);
    let release = native_continuation::newer(&world);
    native_cohort::install(&mut world, &release, true);
    assert!(world.core.take_run(&a).is_none());
    assert!(world.core.take_run(&b).is_none());
    let original = world.row(&a);
    let private = input(&world, &a);
    let plan = world.core.plan_queued_tool_discard(&a).unwrap();
    assert_eq!(world.row(&a), original);
    assert_eq!(roots(&mut world), 2);
    assert_eq!(
        world.core.discard_queued_tool_job(&plan, "foreign").err(),
        Some(ErrorCode::RevisionConflict)
    );
    let mut other = World::new();
    assert_eq!(
        other
            .core
            .discard_queued_tool_job(&plan, plan.challenge())
            .err(),
        Some(ErrorCode::InstanceMismatch)
    );
    world
        .core
        .discard_queued_tool_job(&plan, plan.challenge())
        .unwrap();
    let disposed = world.row(&a);
    assert_eq!(disposed["snapshot"]["state"], "cancelled");
    assert_eq!(disposed["snapshot"]["result"]["error"]["code"], "ABORTED");
    for field in ["packageDigest", "dependencyLock", "grantEpoch", "deadline"] {
        assert_eq!(disposed["snapshot"][field], original["snapshot"][field]);
    }
    assert_eq!(
        disposed["toolArtifacts"]["images"],
        original["toolArtifacts"]["images"]
    );
    assert_eq!(disposed["toolArtifacts"]["retained"], false);
    assert_eq!(world.row(&b)["toolArtifacts"]["retained"], true);
    assert_eq!(roots(&mut world), 1);
    assert_eq!(input(&world, &a), private);
    assert!(world.image.join("bin/ffmpeg.exe").is_file());
    assert!(world.core.take_run(&a).is_none());
    assert_eq!(
        world
            .core
            .discard_queued_tool_job(&plan, plan.challenge())
            .err(),
        Some(ErrorCode::InvalidRequest)
    );
    let next = world.core.plan_queued_tool_discard(&b).unwrap();
    world
        .core
        .discard_queued_tool_job(&next, next.challenge())
        .unwrap();
    assert_eq!(roots(&mut world), 0);
    assert!(!world.path.join("native-media-inspection-v1").exists());
    upgrade_catalog(&mut world);
    reopen(&mut world);
    assert_eq!(world.core.state(&a), Some(JobState::Cancelled));
    assert!(world.core.take_run(&a).is_none());
    assert_eq!(roots(&mut world), 0);
    assert!(world.image.join("bin/ffprobe.exe").is_file());
}

#[test]
fn exact_row_substitution_refuses_disposition_and_keeps_the_actor_roots_and_input() {
    let (mut world, run) = interrupted();
    let plan = world.core.plan_queued_tool_discard(&run).unwrap();
    let private = input(&world, &run);
    let foreign = raw(&mut world, &run) + " ";
    world.core.with_tool_fixture_store(|store| {
        store
            .connection
            .execute(
                "UPDATE jobs SET metadata=?1 WHERE run_id=?2",
                rusqlite::params![foreign, run],
            )
            .unwrap();
    });
    assert_eq!(
        world
            .core
            .discard_queued_tool_job(&plan, plan.challenge())
            .err(),
        Some(ErrorCode::StoreCorrupt)
    );
    assert_eq!(raw(&mut world, &run), foreign);
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], true);
    assert_eq!(input(&world, &run), private);
    assert_eq!(world.core.state(&run), Some(JobState::Interrupted));
    assert_eq!(
        world.core.plan_queued_tool_discard(&run).err(),
        Some(ErrorCode::ExecutionInterrupted)
    );
}

#[test]
fn trigger_substitution_rolls_back_the_release_and_unknown_write_stops_admission() {
    let (mut world, run) = interrupted();
    let plan = world.core.plan_queued_tool_discard(&run).unwrap();
    let original = raw(&mut world, &run);
    let private = input(&world, &run);
    world.core.with_tool_fixture_store(|store| {
        store.connection.execute_batch("CREATE TRIGGER foreign_discard AFTER UPDATE ON jobs BEGIN UPDATE jobs SET metadata=OLD.metadata WHERE run_id=NEW.run_id; END;").unwrap();
    });
    assert!(world
        .core
        .discard_queued_tool_job(&plan, plan.challenge())
        .is_err());
    assert_eq!(raw(&mut world, &run), original);
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], true);
    assert_eq!(roots(&mut world), 1);
    assert_eq!(input(&world, &run), private);
    assert_eq!(
        world.core.plan_queued_tool_discard(&run).err(),
        Some(ErrorCode::ExecutionInterrupted)
    );
}

#[test]
fn trust_changes_and_restarts_require_a_new_bound_disposition_without_renewing_grants() {
    let (mut world, run) = interrupted();
    let old = world.core.plan_queued_tool_discard(&run).unwrap();
    let original = world.row(&run);
    let trust = world.fixture.trust();
    let mut root = world.fixture.root.clone();
    root.version += 1;
    root.previous_root_sha256 = Some(
        world
            .core
            .with_tool_fixture_store(|store| trust.snapshot(store).unwrap().root_digest),
    );
    world.core.with_tool_fixture_store(|store| {
        trust
            .rotate(
                store,
                &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]),
                world.fixture.clock(),
            )
            .unwrap()
    });
    assert_eq!(
        world
            .core
            .discard_queued_tool_job(&old, old.challenge())
            .err(),
        Some(ErrorCode::RevisionConflict)
    );
    assert_eq!(world.row(&run), original);
    let fresh = world.core.plan_queued_tool_discard(&run).unwrap();
    assert_ne!(fresh.challenge(), old.challenge());
    upgrade_catalog(&mut world);
    reopen(&mut world);
    assert_eq!(
        world
            .core
            .discard_queued_tool_job(&fresh, fresh.challenge())
            .err(),
        Some(ErrorCode::InstanceMismatch)
    );
    let current = world.core.plan_queued_tool_discard(&run).unwrap();
    world
        .core
        .discard_queued_tool_job(&current, current.challenge())
        .unwrap();
    assert_eq!(
        world.row(&run)["snapshot"]["grantEpoch"],
        original["snapshot"]["grantEpoch"]
    );
    assert_eq!(roots(&mut world), 0);
    assert!(world.core.take_run(&run).is_none());
}

#[test]
fn restarted_native_attempts_remain_retained_and_cannot_be_relabelled_as_unused_queues() {
    let mut world = World::new();
    let run = world.start("consumer");
    let _handle = world.core.start_media_inspection(&run, &wave()).unwrap();
    let before = world.row(&run);
    let id = before["nativeInspections"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let native = fs::read(
        world
            .path
            .join("native-media-inspection-v1")
            .join(&id)
            .join("input"),
    )
    .unwrap();
    let private = input(&world, &run);
    reopen(&mut world);
    assert_eq!(world.core.state(&run), Some(JobState::Interrupted));
    let original = raw(&mut world, &run);
    assert_eq!(
        world.core.plan_queued_tool_discard(&run).err(),
        Some(ErrorCode::InvalidRequest)
    );
    assert_eq!(raw(&mut world, &run), original);
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], true);
    assert_eq!(roots(&mut world), 1);
    assert_eq!(input(&world, &run), private);
    assert_eq!(
        fs::read(
            world
                .path
                .join("native-media-inspection-v1")
                .join(id)
                .join("input")
        )
        .unwrap(),
        native
    );
    assert!(world.core.take_run(&run).is_none());
}
