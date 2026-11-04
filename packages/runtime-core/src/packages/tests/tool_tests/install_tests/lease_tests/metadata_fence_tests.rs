//! A cached task cannot overwrite a changed durable predecessor, even when
//! the replacement is well-formed and the later write has no trigger itself.
use super::*;

fn changed_row_is_retained(action: &str) {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let receipt = submit(&mut core, operation("consumer", "original-key"));
    if action == "finish" {
        assert!(core.take_run(&receipt.run_id).is_some());
    }
    let input = world
        .path
        .join("private-jobs")
        .join(format!("{}.input.bin", receipt.run_id));
    let private = fs::read(&input).unwrap();
    let mut unknown = row(&mut core, &receipt.run_id);
    unknown["idempotencyKey"] = json!("unknown-committed-key");
    core.with_tool_fixture_store(|store| {
        store
            .connection
            .execute(
                "UPDATE jobs SET metadata=?1 WHERE run_id=?2",
                rusqlite::params![serde_json::to_string(&unknown).unwrap(), receipt.run_id],
            )
            .unwrap();
    });
    match action {
        "cancel" => core.cancel(&receipt.run_id),
        "dispatch" => assert!(core.take_run(&receipt.run_id).is_none()),
        "finish" => core.finish(&receipt.run_id, Ok(json!("fixture"))),
        _ => unreachable!(),
    }
    assert!(
        core.stopping(),
        "Unknown predecessor must stop admission at {action}"
    );
    assert_eq!(row(&mut core, &receipt.run_id), unknown, "{action}");
    assert_eq!(fs::read(&input).unwrap(), private, "{action}");
    let path = image.join("bin/ffmpeg.exe");
    writable(&path);
    assert!(write_handle(&path).is_err(), "{action}");
    readonly(&path);
    assert_eq!(
        fs::read(&path).unwrap(),
        world.fixture.contents["bin/ffmpeg.exe"]
    );
}
#[test]
fn cancellation_cannot_overwrite_a_changed_committed_job_row() {
    changed_row_is_retained("cancel");
}
#[test]
fn dispatch_cannot_overwrite_a_changed_committed_job_row() {
    changed_row_is_retained("dispatch");
}
#[test]
fn completion_cannot_overwrite_a_changed_committed_job_row() {
    changed_row_is_retained("finish");
}
#[test]
fn trust_observation_trigger_cannot_hide_an_intervening_job_change_before_dispatch() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let (mut core, clock) = core_with_clock(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let receipt = submit(&mut core, operation("consumer", "trust-effect"));
    let mut unknown = row(&mut core, &receipt.run_id);
    unknown["idempotencyKey"] = json!("changed-by-trust-trigger");
    core.with_tool_fixture_store(|store| {
        store.connection.execute_batch("CREATE TRIGGER change_job_after_trust AFTER UPDATE ON core_metadata WHEN NEW.key='package-trust-state-v1' BEGIN UPDATE jobs SET metadata=json_set(metadata,'$.idempotencyKey','changed-by-trust-trigger'); END;").unwrap();
    });
    clock.store(world.fixture.now() + 1, std::sync::atomic::Ordering::SeqCst);
    assert!(core.take_run(&receipt.run_id).is_none());
    assert!(core.stopping());
    assert_eq!(row(&mut core, &receipt.run_id), unknown);
    core.with_tool_fixture_store(|store| {
        assert_eq!(
            world.trust.snapshot(store).unwrap().observed_time,
            world.fixture.now() + 1
        )
    });
    let path = image.join("bin/ffmpeg.exe");
    writable(&path);
    assert!(write_handle(&path).is_err());
    readonly(&path);
    assert!(world
        .path
        .join("private-jobs")
        .join(format!("{}.input.bin", receipt.run_id))
        .is_file());
}
#[test]
fn byte_only_changes_are_not_normalized_away_by_the_predecessor_check() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let receipt = submit(&mut core, operation("consumer", "whitespace"));
    let changed = core.with_tool_fixture_store(|store| {
        let original: String = store
            .connection
            .query_row(
                "SELECT metadata FROM jobs WHERE run_id=?1",
                [&receipt.run_id],
                |row| row.get(0),
            )
            .unwrap();
        let changed = format!("{original}\n");
        store
            .connection
            .execute(
                "UPDATE jobs SET metadata=?1 WHERE run_id=?2",
                rusqlite::params![changed, receipt.run_id],
            )
            .unwrap();
        changed
    });
    core.cancel(&receipt.run_id);
    assert!(core.stopping());
    core.with_tool_fixture_store(|store| {
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT metadata FROM jobs WHERE run_id=?1",
                    [&receipt.run_id],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            changed
        )
    });
    let path = image.join("bin/ffmpeg.exe");
    writable(&path);
    assert!(write_handle(&path).is_err());
    readonly(&path);
}
#[test]
fn a_missing_predecessor_is_never_recreated_by_cancellation() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let receipt = submit(&mut core, operation("consumer", "missing-row"));
    core.with_tool_fixture_store(|store| {
        store
            .connection
            .execute("DELETE FROM jobs WHERE run_id=?1", [&receipt.run_id])
            .unwrap()
    });
    core.cancel(&receipt.run_id);
    assert!(core.stopping());
    core.with_tool_fixture_store(|store| {
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM jobs", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        )
    });
    let path = image.join("bin/ffmpeg.exe");
    writable(&path);
    assert!(write_handle(&path).is_err());
    readonly(&path);
    assert!(world
        .path
        .join("private-jobs")
        .join(format!("{}.input.bin", receipt.run_id))
        .is_file());
}
#[test]
fn create_only_acceptance_refuses_an_existing_row_even_when_its_bytes_are_identical() {
    let mut world = World::new();
    prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let receipt = submit(&mut core, operation("consumer", "create-only"));
    let original = row(&mut core, &receipt.run_id);
    core.with_tool_fixture_store(|store| {
        assert_eq!(
            crate::packages::tools::install::leases::persist_job(
                store,
                &receipt.run_id,
                &original,
                None,
                None
            ),
            Err(ErrorCode::StoreCorrupt)
        )
    });
    assert_eq!(row(&mut core, &receipt.run_id), original);
    core.cancel(&receipt.run_id);
    assert!(!core.stopping());
}
#[test]
fn service_completion_cannot_overwrite_unknown_terminal_metadata_or_release_artifacts() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let mut manifests = catalog(&world, &["consumer", "middle"])
        .list()
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    manifests
        .iter_mut()
        .find(|value| value["id"] == "consumer")
        .unwrap()["dependencies"]["services"] = json!([{
        "publisher":"fixture-publisher","id":"middle","version":"^1.0.0","service":"text-transform","interfaceVersion":"^1.0.0"
    }]);
    let catalog = BuiltinCatalog::from_host_manifests(manifests).unwrap();
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    grant_command(&mut core, &catalog, "middle", "svc:text-transform:convert");
    let receipt = submit(&mut core, operation("consumer", "pending-service"));
    assert!(core.take_run(&receipt.run_id).is_some());
    let call = core
        .prepare_service(
            &receipt.run_id,
            crate::services::ServiceTarget {
                publisher: "fixture-publisher".into(),
                id: "middle".into(),
                service: "text-transform".into(),
                operation: "convert".into(),
            },
            json!({"text":"fixture"}),
        )
        .unwrap();
    core.start_service(&call.spec.run_id).unwrap();
    core.finish(&receipt.run_id, Err(ErrorCode::ExecutionFailed));
    assert_eq!(core.active_service_calls(), 1);
    let mut unknown = row(&mut core, &receipt.run_id);
    assert_eq!(unknown["toolArtifacts"]["retained"], true);
    unknown["idempotencyKey"] = json!("unknown-terminal-key");
    core.with_tool_fixture_store(|store| {
        store
            .connection
            .execute(
                "UPDATE jobs SET metadata=?1 WHERE run_id=?2",
                rusqlite::params![serde_json::to_string(&unknown).unwrap(), receipt.run_id],
            )
            .unwrap()
    });
    assert!(core
        .finish_service(&call.spec.run_id, Err(ErrorCode::Aborted))
        .is_err());
    assert!(core.stopping());
    assert_eq!(row(&mut core, &receipt.run_id), unknown);
    let path = image.join("bin/ffmpeg.exe");
    writable(&path);
    assert!(write_handle(&path).is_err());
    readonly(&path);
}
#[test]
fn restoring_valid_noncanonical_json_binds_its_original_bytes_before_any_rewrite() {
    let mut world = World::new();
    prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let mut first = core(&mut world, catalog.clone(), false);
    grant(&mut first, &catalog, "consumer");
    let mut operation = operation("consumer", "noncanonical");
    operation.background = true;
    let receipt = submit(&mut first, operation.clone());
    let original = row(&mut first, &receipt.run_id);
    drop(first);
    world.reopen();
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute(
            "UPDATE jobs SET metadata=?1 WHERE run_id=?2",
            rusqlite::params![
                serde_json::to_string_pretty(&original).unwrap(),
                receipt.run_id
            ],
        )
        .unwrap();
    let mut manager = core(&mut world, catalog.clone(), true);
    assert!(manager.pending_runs().is_empty());
    assert_eq!(
        row(&mut manager, &receipt.run_id)["toolArtifacts"],
        original["toolArtifacts"]
    );
    assert!(row(&mut manager, &receipt.run_id)
        .get("durableMetadata")
        .is_none());
    drop(manager);
    world.reopen();
    let mut reopened = core(&mut world, catalog, false);
    assert_eq!(reopened.pending_runs(), vec![receipt.run_id.clone()]);
    assert_eq!(submit(&mut reopened, operation).run_id, receipt.run_id);
    reopened.cancel(&receipt.run_id);
    assert!(!reopened.stopping());
}
