//! Refusals retain old native bytes and metadata instead of rearming work.
use super::*;
use std::sync::atomic::Ordering;

#[test]
fn expired_or_rolled_back_trust_interrupts_dispatch_without_erasing_input_or_artifact_roots() {
    for expired in [false, true] {
        let mut world = World::new();
        let image = prepare_image(&mut world);
        let catalog = catalog(&world, &["consumer"]);
        let (mut core, clock) = core_with_clock(&mut world, catalog.clone(), false);
        grant(&mut core, &catalog, "consumer");
        let receipt = submit(&mut core, operation("consumer", "expiry"));
        let input = world
            .path
            .join("private-jobs")
            .join(format!("{}.input.bin", receipt.run_id));
        let private = fs::read(&input).unwrap();
        let accepted = row(&mut core, &receipt.run_id)["toolArtifacts"].clone();
        clock.store(
            if expired {
                world.fixture.base.package.expires_at
            } else {
                world.fixture.now() - 1
            },
            Ordering::SeqCst,
        );
        assert!(core.take_run(&receipt.run_id).is_none());
        assert_eq!(core.state(&receipt.run_id), Some(JobState::Interrupted));
        assert_eq!(row(&mut core, &receipt.run_id)["toolArtifacts"], accepted);
        assert_eq!(fs::read(&input).unwrap(), private);
        assert_eq!(
            fs::read(image.join("bin/ffmpeg.exe")).unwrap(),
            world.fixture.contents["bin/ffmpeg.exe"]
        );
        let path = image.join("bin/ffmpeg.exe");
        writable(&path);
        assert!(write_handle(&path).is_err());
        readonly(&path);
        drop(core);
        // A normal management-only Host can retain historical metadata without
        // renewing tool trust, creating a tool context or deleting private input.
        let mut manager = RuntimeCore::managed(
            tokens(),
            DataStore::open(&world.path.join("runtime.sqlite")).unwrap(),
            true,
            false,
        )
        .unwrap();
        assert!(manager.pending_runs().is_empty());
        assert_eq!(manager.state(&receipt.run_id), Some(JobState::Interrupted));
        assert_eq!(
            row(&mut manager, &receipt.run_id)["toolArtifacts"],
            accepted
        );
        assert_eq!(fs::read(&input).unwrap(), private);
    }
}
#[test]
fn job_acceptance_triggers_cannot_mutate_the_image_inventory_or_ack_a_partial_lease() {
    for statement in [
        "SELECT RAISE(ABORT,'fixture')",
        "UPDATE core_metadata SET value='CORRUPT' WHERE key='tool-image-inventory-v1'",
        "UPDATE jobs SET metadata='CORRUPT' WHERE run_id=NEW.run_id",
    ] {
        let mut world = World::new();
        let image = prepare_image(&mut world);
        let catalog = catalog(&world, &["consumer"]);
        let mut core = core(&mut world, catalog.clone(), false);
        grant(&mut core, &catalog, "consumer");
        let before = core.with_tool_fixture_store(|store| {
            let before: String = store
                .connection
                .query_row(
                    "SELECT value FROM core_metadata WHERE key='tool-image-inventory-v1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            store
                .connection
                .execute_batch(&format!(
                    "CREATE TRIGGER corrupt_job AFTER INSERT ON jobs BEGIN {statement}; END;"
                ))
                .unwrap();
            before
        });
        let cli = open(&mut core, "cli");
        let outcome = core
            .handle(
                "cli",
                request(Call::Submit(operation("consumer", "uncertain")), Some(cli)),
            )
            .outcome;
        assert!(
            matches!(
                outcome,
                Outcome::Error(RuntimeError {
                    code: ErrorCode::AcceptanceUnknown
                })
            ),
            "{statement}: {outcome:?}"
        );
        assert!(core.stopping());
        assert!(core.pending_runs().is_empty());
        core.with_tool_fixture_store(|store| {
            assert_eq!(
                store
                    .connection
                    .query_row("SELECT count(*) FROM jobs", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                0
            );
            assert_eq!(
                store
                    .connection
                    .query_row(
                        "SELECT value FROM core_metadata WHERE key='tool-image-inventory-v1'",
                        [],
                        |r| r.get::<_, String>(0)
                    )
                    .unwrap(),
                before
            );
        });
        let path = image.join("bin/ffmpeg.exe");
        writable(&path);
        assert!(write_handle(&path).is_err());
        readonly(&path);
        assert_eq!(
            fs::read(&path).unwrap(),
            world.fixture.contents["bin/ffmpeg.exe"]
        );
    }
}
#[test]
fn failed_terminal_or_release_commit_retains_native_handles_and_the_exact_private_input() {
    for releasing in [false, true] {
        let mut world = World::new();
        let image = prepare_image(&mut world);
        let catalog = catalog(&world, &["consumer"]);
        let mut core = core(&mut world, catalog.clone(), false);
        grant(&mut core, &catalog, "consumer");
        let receipt = submit(&mut core, operation("consumer", "terminal"));
        assert!(core.take_run(&receipt.run_id).is_some());
        let input = world
            .path
            .join("private-jobs")
            .join(format!("{}.input.bin", receipt.run_id));
        let private = fs::read(&input).unwrap();
        core.with_tool_fixture_store(|store| {
            let predicate = if releasing { "json_extract(NEW.metadata,'$.toolArtifacts.retained')=0" }
                else { "json_extract(NEW.metadata,'$.snapshot.state')='succeeded'" };
            store.connection.execute_batch(&format!("CREATE TRIGGER corrupt_terminal AFTER UPDATE ON jobs WHEN {predicate} BEGIN UPDATE core_metadata SET value='CORRUPT' WHERE key='tool-image-inventory-v1'; END;")).unwrap();
        });
        core.finish(&receipt.run_id, Ok(json!("complete")));
        assert!(core.stopping());
        let stored = row(&mut core, &receipt.run_id);
        assert_eq!(stored["toolArtifacts"]["retained"], true);
        assert_eq!(
            stored["snapshot"]["state"],
            if releasing { "succeeded" } else { "running" }
        );
        assert_eq!(fs::read(&input).unwrap(), private);
        let path = image.join("bin/ffmpeg.exe");
        writable(&path);
        assert!(write_handle(&path).is_err());
        readonly(&path);
        core.with_tool_fixture_store(|store| {
            let inventory: String = store
                .connection
                .query_row(
                    "SELECT value FROM core_metadata WHERE key='tool-image-inventory-v1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_ne!(inventory, "CORRUPT");
        });
    }
}
