//! Accepted queue entries never acquire a newer tool or grant at dispatch.
//! Real signed installed releases and durable roots prove the refusal policy.
use super::*;
use crate::private_jobs::PrivateJobs;
use native_cohort::{candidate, fresh_health, install};
use native_continuation::newer;

pub(super) fn enqueue(world: &mut World, id: &str, background: bool) -> String {
    let cli = world.open("cli");
    let response = world.core.handle(
        "cli",
        request(
            Call::Submit(SubmitJob {
                plugin_id: id.into(),
                command_id: "convert".into(),
                input: json!({"text":"MANAGED_MEDIA_PRIVATE_INPUT"}),
                idempotency_key: uuid::Uuid::new_v4().to_string(),
                background,
                deadline: now() + 20_000.0,
            }),
            Some(cli),
        ),
    );
    let Outcome::Receipt(receipt) = response.outcome else {
        panic!("{response:?}");
    };
    assert_eq!(receipt.receipt_type, "job");
    assert_eq!(world.core.state(&receipt.run_id), Some(JobState::Queued));
    receipt.run_id
}
fn unchanged_pin(world: &mut World, run: &str, original: &Value) {
    let row = world.row(run);
    for field in ["packageDigest", "dependencyLock", "grantEpoch", "deadline"] {
        assert_eq!(row["snapshot"][field], original["snapshot"][field]);
    }
    assert_eq!(
        row["toolArtifacts"]["images"],
        original["toolArtifacts"]["images"]
    );
    assert_eq!(
        row["toolArtifacts"]["grantEpoch"],
        original["toolArtifacts"]["grantEpoch"]
    );
    assert!(row["nativeInspections"].is_null());
    assert!(row["nativeAudio"].is_null());
    assert!(row["nativeVideo"].is_null());
    assert_eq!(world.core.active_media_inspections(), 0);
    assert!(!world.path.join("native-media-inspection-v1").exists());
}

#[test]
fn old_queued_pin_is_interrupted_after_a_floor_advance_and_keeps_its_private_input() {
    let mut world = World::new();
    world.grant("consumer");
    let run = enqueue(&mut world, "consumer", false);
    let original = world.row(&run);
    let input = fs::read(
        world
            .path
            .join("private-jobs")
            .join(format!("{run}.input.bin")),
    )
    .unwrap();
    let release = newer(&world);
    install(&mut world, &release, true);
    assert!(world.core.take_run(&run).is_none());
    assert_eq!(world.core.state(&run), Some(JobState::Interrupted));
    assert!(world.core.take_run(&run).is_none());
    assert!(!world.core.pending_runs().contains(&run));
    unchanged_pin(&mut world, &run, &original);
    assert_eq!(
        world.row(&run)["snapshot"]["result"]["error"]["code"],
        "DEPENDENCY_ARTIFACT_MISMATCH"
    );
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], true);
    assert_eq!(
        fs::read(
            world
                .path
                .join("private-jobs")
                .join(format!("{run}.input.bin"))
        )
        .unwrap(),
        input
    );
    assert_eq!(
        PrivateJobs::open(&world.path)
            .unwrap()
            .read(&run, "input")
            .unwrap(),
        json!({"count":1,"text":"MANAGED_MEDIA_PRIVATE_INPUT"})
    );
    assert!(world.image.join("bin/ffmpeg.exe").is_file());
    world.core.with_tool_fixture_store(|store| {
        assert_eq!(
            crate::runtime::retained_tool_leases(store).unwrap().len(),
            1
        )
    });
}

#[test]
fn old_interrupted_queue_receipt_survives_catalog_switch_restart_and_a_fresh_new_run() {
    let mut world = World::new();
    world.grant("consumer");
    let run = enqueue(&mut world, "consumer", false);
    let original = world.row(&run);
    let release = newer(&world);
    install(&mut world, &release, true);
    assert!(world.core.take_run(&run).is_none());
    let catalog = candidate(&world, &release);
    let plan = world.core.stage_provider_cohort(catalog.clone()).unwrap();
    world
        .core
        .begin_provider_cohort(&plan, plan.digest())
        .unwrap();
    world
        .core
        .commit_provider_cohort(&plan, |_| Ok(()))
        .unwrap();
    let retained = world.row(&run);
    assert_eq!(retained["toolArtifacts"]["retained"], true);
    world.core = RuntimeCore::validation("unused1".into(), "unused2".into());
    world.core = RuntimeCore::managed_fixture_tool_catalog(
        HashMap::from([
            ("cli".into(), "local-cli".into()),
            ("admin".into(), "local-manager".into()),
        ]),
        DataStore::open(&world.path.join("runtime.sqlite")).unwrap(),
        false,
        catalog.clone(),
        world.fixture.trust(),
        Arc::new(AtomicU64::new(world.fixture.clock())),
    )
    .unwrap();
    world.catalog = catalog;
    world.sessions.clear();
    assert_eq!(world.core.state(&run), Some(JobState::Interrupted));
    assert!(world.core.take_run(&run).is_none());
    unchanged_pin(&mut world, &run, &original);
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], true);
    let next = fresh_health(&mut world, "consumer", &release);
    assert_ne!(next, run);
    world.core.finish(&next, Err(ErrorCode::Aborted));
    assert_eq!(
        world.row(&run)["toolArtifacts"]["images"],
        retained["toolArtifacts"]["images"]
    );
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], true);
    world.core.with_tool_fixture_store(|store| {
        assert_eq!(
            crate::runtime::retained_tool_leases(store).unwrap().len(),
            1
        )
    });
}

#[test]
fn reapproving_a_queued_package_does_not_refresh_its_accepted_epoch() {
    let mut world = World::new();
    world.grant("consumer");
    let run = enqueue(&mut world, "consumer", false);
    let original = world.row(&run);
    world.grant("consumer");
    assert!(world.core.take_run(&run).is_none());
    assert_eq!(world.core.state(&run), Some(JobState::Failed));
    unchanged_pin(&mut world, &run, &original);
    assert_eq!(
        world.row(&run)["snapshot"]["result"]["error"]["code"],
        "GRANT_REVOKED"
    );
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], false);
    assert!(world.image.join("bin/ffprobe.exe").is_file());
    let next = world.start_granted("consumer");
    assert!(
        world.row(&next)["snapshot"]["grantEpoch"].as_u64().unwrap()
            > original["snapshot"]["grantEpoch"].as_u64().unwrap()
    );
    world.core.finish(&next, Err(ErrorCode::Aborted));
}

#[test]
fn revoked_queued_consumer_cannot_dispatch_while_another_shared_consumer_remains_authorized() {
    let mut world = World::new();
    world.grant("consumer");
    world.grant("other-consumer");
    let a = enqueue(&mut world, "consumer", false);
    let b = enqueue(&mut world, "other-consumer", false);
    let original = world.row(&a);
    let admin = world.open("admin");
    native_inspection::revoke(&mut world.core, admin);
    assert!(world.core.take_run(&a).is_none());
    assert_eq!(world.core.state(&a), Some(JobState::Failed));
    unchanged_pin(&mut world, &a, &original);
    assert_eq!(
        world.row(&a)["snapshot"]["result"]["error"]["code"],
        "GRANT_REVOKED"
    );
    assert!(world.core.take_run(&b).is_some());
    assert_eq!(world.core.state(&b), Some(JobState::Running));
    assert_eq!(world.row(&b)["toolArtifacts"]["retained"], true);
    world.core.finish(&b, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&b)["toolArtifacts"]["retained"], false);
}
