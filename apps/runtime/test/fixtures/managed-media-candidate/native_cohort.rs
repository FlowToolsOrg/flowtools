//! Two exact signed releases and two consumer catalog pins, using actual held
//! native images. Cohort consent never supplies the new execution grants.
use super::*;
use crate::runtime::{MediaInspectionState, ToolHealthState};
use native_continuation::{newer, Newer};
use std::time::Instant;

pub(super) fn install(world: &mut World, release: &Newer, materialize: bool) {
    let trust = world.fixture.trust();
    let time = world.fixture.clock();
    world.core.with_tool_fixture_store(|store| {
        let installed = install::install_fixture_with_hook(
            &trust,
            store,
            ToolArchiveInput {
                envelope: &release.envelope,
                manifest: &release.manifest,
                archive: &release.archive,
            },
            || time,
            |_, _| Ok(()),
        )
        .unwrap();
        if materialize {
            images::materialize_fixture_with_hook(
                &trust,
                store,
                &installed,
                || time,
                |_, _| Ok(()),
            )
            .unwrap();
        }
    });
}
pub(super) fn candidate(world: &World, release: &Newer) -> BuiltinCatalog {
    BuiltinCatalog::from_host_manifests(
        world
            .catalog
            .list()
            .map(|(_, manifest)| {
                let mut manifest = manifest.clone();
                manifest["version"] = json!("2.0.0");
                manifest["dependencies"]["tools"][0]["digest"] =
                    json!(release.package.archive.sha256);
                manifest
            })
            .collect(),
    )
    .unwrap()
}
fn submit(world: &mut World, id: &str) -> Outcome {
    let cli = world.open("cli");
    world
        .core
        .handle(
            "cli",
            request(
                Call::Submit(SubmitJob {
                    plugin_id: id.into(),
                    command_id: "convert".into(),
                    input: json!({"text":"MANAGED_MEDIA_PRIVATE_INPUT"}),
                    idempotency_key: uuid::Uuid::new_v4().to_string(),
                    background: false,
                    deadline: now() + 20_000.0,
                }),
                Some(cli),
            ),
        )
        .outcome
}
pub(super) fn fresh_health(world: &mut World, id: &str, release: &Newer) -> String {
    assert!(matches!(
        submit(world, id),
        Outcome::Error(RuntimeError {
            code: ErrorCode::ApprovalRequired
        })
    ));
    let run = world.start(id);
    let handle = world.core.start_tool_health(&run).unwrap();
    let started = Instant::now();
    while world.row(&run)["nativeAudio"][0]["phase"] != "completed" {
        assert!(started.elapsed() < Duration::from_secs(10));
        world.core.tick_media_inspections().unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
    let observed = world.core.media_audio_fixture_observations(&run);
    assert_eq!(observed.len(), 2);
    for value in observed {
        assert_ne!(value.0, 0);
        assert_eq!(value.1, 0);
        assert!(value.2);
    }
    let ToolHealthState::Healthy(health) = world.core.poll_tool_health(&handle).unwrap() else {
        panic!("Current release did not pass the actual roundtrip")
    };
    assert_eq!(health.pin().version, "2.0.0");
    assert_eq!(health.pin().archive_digest, release.package.archive.sha256);
    run
}
fn raw_journal(world: &mut World) -> Option<String> {
    use rusqlite::OptionalExtension;
    world.core.with_tool_fixture_store(|store| {
        store
            .connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='service-catalog-state-v1'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap()
    })
}

#[test]
fn both_old_consumers_must_drain_before_the_new_shared_image_can_receive_fresh_grants() {
    let mut world = World::new();
    let first = world.start("consumer");
    let second = world.start("other-consumer");
    let handle = world.core.start_media_inspection(&first, &wave()).unwrap();
    let started = Instant::now();
    while world.row(&first)["nativeInspections"][0]["phase"] != "completed" {
        assert!(started.elapsed() < Duration::from_secs(10));
        world.core.tick_media_inspections().unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
    let observations = world.core.media_inspection_fixture_observations(&first);
    assert_eq!(observations.len(), 1);
    assert_ne!(observations[0].0, 0);
    assert_eq!(observations[0].1, 0);
    assert!(observations[0].2);
    assert!(matches!(
        world.core.poll_media_inspection(&handle).unwrap(),
        MediaInspectionState::Complete(_)
    ));
    let old_images = world.row(&first)["toolArtifacts"]["images"].clone();
    let release = newer(&world);
    install(&mut world, &release, true);
    let catalog = candidate(&world, &release);
    let plan = world.core.stage_provider_cohort(catalog.clone()).unwrap();
    assert_eq!(plan.changes().len(), 2);
    world
        .core
        .begin_provider_cohort(&plan, plan.digest())
        .unwrap();
    assert_eq!(
        world.core.commit_provider_cohort(&plan, |_| panic!(
            "accepted old consumers still own both pins"
        )),
        Err(ErrorCode::RuntimeBusy)
    );
    world.core.finish(&first, Err(ErrorCode::Aborted));
    assert_eq!(
        world
            .core
            .commit_provider_cohort(&plan, |_| panic!("second old root still owns its pin")),
        Err(ErrorCode::RuntimeBusy)
    );
    assert_eq!(world.row(&second)["toolArtifacts"]["images"], old_images);
    assert_eq!(world.row(&second)["toolArtifacts"]["retained"], true);
    world.core.finish(&second, Err(ErrorCode::Aborted));
    world
        .core
        .commit_provider_cohort(&plan, |_| Ok(()))
        .unwrap();
    world.catalog = catalog;
    assert_eq!(
        fs::read_dir(world.path.join("tool-image-store-v1/artifacts"))
            .unwrap()
            .count(),
        2
    );
    let a = fresh_health(&mut world, "consumer", &release);
    let b = fresh_health(&mut world, "other-consumer", &release);
    let a_images = world.row(&a)["toolArtifacts"]["images"].clone();
    assert_eq!(a_images, world.row(&b)["toolArtifacts"]["images"]);
    assert_ne!(a_images, old_images);
    world.core.finish(&a, Err(ErrorCode::Aborted));
    world.core.finish(&b, Err(ErrorCode::Aborted));
    assert!(world.image.join("bin/ffmpeg.exe").is_file());
}

#[test]
fn missing_current_materialization_refuses_before_migration_and_can_be_retried_explicitly() {
    let mut world = World::new();
    let release = newer(&world);
    install(&mut world, &release, false);
    let catalog = candidate(&world, &release);
    let plan = world.core.stage_provider_cohort(catalog.clone()).unwrap();
    world
        .core
        .begin_provider_cohort(&plan, plan.digest())
        .unwrap();
    assert_eq!(
        world
            .core
            .commit_provider_cohort(&plan, |_| panic!("no current held image")),
        Err(ErrorCode::DependencyArtifactMismatch)
    );
    assert!(raw_journal(&mut world).is_none());
    assert_eq!(
        fs::read_dir(world.path.join("tool-image-store-v1/artifacts"))
            .unwrap()
            .count(),
        1
    );
    install(&mut world, &release, true);
    world
        .core
        .commit_provider_cohort(&plan, |_| Ok(()))
        .unwrap();
    world.catalog = catalog;
    let run = fresh_health(&mut world, "consumer", &release);
    world.core.finish(&run, Err(ErrorCode::Aborted));
}

#[test]
fn failed_cohort_does_not_lower_the_tool_floor_or_erase_retained_images() {
    let mut world = World::new();
    let release = newer(&world);
    install(&mut world, &release, true);
    let catalog = candidate(&world, &release);
    let before = world.core.with_tool_fixture_store(|store| {
        store
            .connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='tool-image-inventory-v1'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
    });
    let plan = world.core.stage_provider_cohort(catalog.clone()).unwrap();
    world
        .core
        .begin_provider_cohort(&plan, plan.digest())
        .unwrap();
    assert_eq!(
        world.core.commit_provider_cohort(&plan, |tx| {
            tx.execute(
                "UPDATE core_metadata SET value='foreign' WHERE key='tool-image-inventory-v1'",
                [],
            )
            .unwrap();
            Ok(())
        }),
        Err(ErrorCode::StoreCorrupt)
    );
    let after = world.core.with_tool_fixture_store(|store| {
        store
            .connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='tool-image-inventory-v1'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
    });
    assert_eq!(before, after);
    assert!(raw_journal(&mut world).is_none());
    world.grant("consumer");
    assert!(matches!(
        submit(&mut world, "consumer"),
        Outcome::Error(RuntimeError {
            code: ErrorCode::DependencyArtifactMismatch
        })
    ));
    assert!(world.image.join("bin/ffprobe.exe").is_file());
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
    let run = fresh_health(&mut world, "consumer", &release);
    world.core.finish(&run, Err(ErrorCode::Aborted));
}

#[test]
fn committed_cohort_restarts_only_with_new_pins_and_never_reuses_old_execution_grants() {
    let mut world = World::new();
    world.grant("consumer");
    let release = newer(&world);
    install(&mut world, &release, true);
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
    world.core = RuntimeCore::validation("unused1".into(), "unused2".into());
    let restore = |catalog| {
        RuntimeCore::managed_fixture_tool_catalog(
            HashMap::from([
                ("cli".into(), "local-cli".into()),
                ("admin".into(), "local-manager".into()),
            ]),
            DataStore::open(&world.path.join("runtime.sqlite")).unwrap(),
            false,
            catalog,
            world.fixture.trust(),
            Arc::new(AtomicU64::new(world.fixture.clock())),
        )
    };
    assert_eq!(
        restore(world.catalog.clone()).err(),
        Some(ErrorCode::RecoveryPending)
    );
    world.core = restore(catalog.clone()).unwrap();
    world.catalog = catalog;
    world.sessions.clear();
    let run = fresh_health(&mut world, "consumer", &release);
    world.core.finish(&run, Err(ErrorCode::Aborted));
    assert_eq!(
        fs::read_dir(world.path.join("tool-image-store-v1/artifacts"))
            .unwrap()
            .count(),
        2
    );
}
