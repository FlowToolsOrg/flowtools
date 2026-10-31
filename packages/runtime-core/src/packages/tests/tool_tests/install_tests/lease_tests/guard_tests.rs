use super::*;
use crate::packages::tools::install::{
    images::reconciliation::plan_image_reconciliation,
    reconciliation::plan_tool_archive_reconciliation,
};

fn rehash(value: &mut Value) {
    let mut lease = value["toolArtifacts"].clone();
    lease.as_object_mut().unwrap().remove("digest");
    value["toolArtifacts"]["digest"] = json!(digest(&lease));
}
fn replace_row(world: &World, id: &str, value: &str) {
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute(
            "UPDATE jobs SET metadata=?1 WHERE run_id=?2",
            rusqlite::params![value, id],
        )
        .unwrap();
}
#[test]
fn corrupt_duplicate_oversize_and_forged_lease_rows_block_both_orphan_plans_without_moving_bytes() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let receipt = submit(&mut core, operation("consumer", "corrupt"));
    let original = row(&mut core, &receipt.run_id);
    drop(core);
    world.reopen();
    let raw = world.metadata(KEY);
    let ledger = world.metadata("tool-image-inventory-v1");
    let source = fs::read(world.blob().join("package.zip")).unwrap();
    let input = world
        .path
        .join("private-jobs")
        .join(format!("{}.input.bin", receipt.run_id));
    let private = fs::read(&input).unwrap();
    for case in [
        "missing",
        "wrong-run",
        "foreign-scope",
        "wrong-seal",
        "duplicate-image",
        "released-queued",
        "duplicate-key",
        "oversize",
        "blob",
    ] {
        let mut value = original.clone();
        match case {
            "missing" => {
                value.as_object_mut().unwrap().remove("toolArtifacts");
            }
            "wrong-run" => {
                value["toolArtifacts"]["runId"] = json!(uuid::Uuid::new_v4().to_string())
            }
            "foreign-scope" => value["toolArtifacts"]["profileDigest"] = json!("f".repeat(64)),
            "wrong-seal" => {
                value["toolArtifacts"]["images"][0]["sealId"] =
                    json!(uuid::Uuid::new_v4().to_string())
            }
            "duplicate-image" => {
                let image = value["toolArtifacts"]["images"][0].clone();
                value["toolArtifacts"]["images"]
                    .as_array_mut()
                    .unwrap()
                    .push(image);
            }
            "released-queued" => value["toolArtifacts"]["retained"] = json!(false),
            _ => {}
        }
        if case != "missing" {
            rehash(&mut value);
        }
        let text = match case {
            "duplicate-key" => format!(
                "{{\"formatVersion\":1,{}",
                &serde_json::to_string(&value).unwrap()[1..]
            ),
            "oversize" => format!(
                "{}{}",
                serde_json::to_string(&value).unwrap(),
                " ".repeat(32769)
            ),
            _ => serde_json::to_string(&value).unwrap(),
        };
        replace_row(&world, &receipt.run_id, &text);
        if case == "blob" {
            world
                .store
                .as_ref()
                .unwrap()
                .connection
                .execute(
                    "UPDATE jobs SET metadata=CAST(metadata AS BLOB) WHERE run_id=?1",
                    [&receipt.run_id],
                )
                .unwrap();
        }
        assert!(world.pins().is_err(), "{case}");
        assert!(
            plan_image_reconciliation(&world.trust, world.store.as_mut().unwrap()).is_err(),
            "{case}"
        );
        assert!(
            plan_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap()).is_err(),
            "{case}"
        );
        assert_eq!(world.metadata(KEY), raw, "{case}");
        assert_eq!(world.metadata("tool-image-inventory-v1"), ledger, "{case}");
        assert_eq!(
            fs::read(world.blob().join("package.zip")).unwrap(),
            source,
            "{case}"
        );
        assert_eq!(
            fs::read(image.join("bin/ffmpeg.exe")).unwrap(),
            world.fixture.contents["bin/ffmpeg.exe"],
            "{case}"
        );
        assert_eq!(fs::read(&input).unwrap(), private, "{case}");
        assert!(world
            .metadata("tool-image-reconciliation-pending-v1")
            .is_none());
        assert!(world
            .metadata("tool-archive-reconciliation-pending-v1")
            .is_none());
    }
    replace_row(
        &world,
        &receipt.run_id,
        &serde_json::to_string(&original).unwrap(),
    );
    assert_eq!(world.pins().unwrap().len(), 1);
}
#[test]
fn actual_copy_and_supported_restore_retain_historical_roots_and_never_renew_a_lease() {
    for native_management in [false, true] {
        let mut world = World::new();
        let image = prepare_image(&mut world);
        let catalog = catalog(&world, &["consumer"]);
        let mut core = core(&mut world, catalog.clone(), false);
        grant(&mut core, &catalog, "consumer");
        let mut operation = operation("consumer", "restore");
        operation.background = true;
        let receipt = submit(&mut core, operation);
        let accepted = row(&mut core, &receipt.run_id)["toolArtifacts"].clone();
        let mut input = world
            .path
            .join("private-jobs")
            .join(format!("{}.input.bin", receipt.run_id));
        let private = fs::read(&input).unwrap();
        let backup = core.with_tool_fixture_store(|store| store.backup().unwrap());
        drop(core);
        let copied =
            std::env::temp_dir().join(format!("flowtools-tool-install-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&copied).unwrap();
        fs::copy(
            world.path.join("runtime.sqlite"),
            copied.join("runtime.sqlite"),
        )
        .unwrap();
        let mut copy = World::open(&copied, false, true);
        assert_eq!(
            copy.store.as_ref().unwrap().load_jobs().unwrap()[0]["toolArtifacts"],
            accepted
        );
        assert!(copy.pins().is_err());
        assert!(plan_image_reconciliation(&copy.trust, copy.store.as_mut().unwrap()).is_err());
        assert!(!copy.path.join("tool-image-store-v1").exists());
        if native_management {
            let backup_id = backup
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .strip_prefix("runtime-backup-")
                .unwrap()
                .to_owned();
            let report =
                crate::recovery::manage(&world.path, StorageAction::Restore { backup_id }).unwrap();
            let recovery = report.recovery.unwrap();
            assert!(recovery.grants_revoked && recovery.payloads_quarantined);
            input = world
                .path
                .join(format!("private-jobs-quarantine-{}", recovery.recovery_id))
                .join(input.file_name().unwrap());
        } else {
            DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
        }
        world.reopen();
        let rows = world.store.as_ref().unwrap().load_jobs().unwrap();
        assert_eq!(
            rows[0]["snapshot"]["state"],
            if native_management {
                "interrupted"
            } else {
                "queued"
            }
        );
        assert_eq!(rows[0]["toolArtifacts"], accepted);
        assert!(world.pins().is_err());
        assert!(plan_image_reconciliation(&world.trust, world.store.as_mut().unwrap()).is_err());
        let mut manager =
            RuntimeCore::managed(tokens(), world.store.take().unwrap(), true, false).unwrap();
        assert!(manager.pending_runs().is_empty());
        assert_eq!(
            row(&mut manager, &receipt.run_id)["toolArtifacts"],
            accepted
        );
        assert_eq!(fs::read(&input).unwrap(), private);
        assert_eq!(
            fs::read(image.join("bin/ffmpeg.exe")).unwrap(),
            world.fixture.contents["bin/ffmpeg.exe"]
        );
    }
}
#[test]
fn dependency_preview_uses_the_same_signed_sources_but_cannot_create_job_leases_or_images() {
    let mut world = World::new();
    world.install().unwrap();
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog, false);
    let cli = open(&mut core, "cli");
    let response = core
        .handle(
            "cli",
            request(
                Call::DependencyPlan(DependencyPlanRequest {
                    plugin_ids: vec!["consumer".into()],
                }),
                Some(cli),
            ),
        )
        .outcome;
    let Outcome::DependencyPlan(lock) = response else {
        panic!("{response:?}");
    };
    assert_eq!(lock.lock.tools.len(), 1);
    assert!(!world.path.join("tool-image-store-v1").exists());
    assert!(core
        .with_tool_fixture_store(|store| store.load_jobs().unwrap())
        .is_empty());
    assert!(core.provider_unload_plan("consumer").is_ok());
}
#[test]
fn a_valid_large_tool_graph_is_refused_before_receipt_private_input_or_unreadable_metadata() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let ids = (0..50)
        .map(|n| format!("consumer-{n:02}"))
        .collect::<Vec<_>>();
    let mut manifests = catalog(&world, &ids.iter().map(String::as_str).collect::<Vec<_>>())
        .list()
        .map(|(_, m)| m.clone())
        .collect::<Vec<_>>();
    manifests.iter_mut().find(|m|m["id"]==ids[0]).unwrap()["dependencies"]["services"]=json!(ids[1..].iter().map(|id|json!({
        "publisher":"fixture-publisher","id":id,"version":"^1.0.0","service":"text-transform","interfaceVersion":"^1.0.0"
    })).collect::<Vec<_>>());
    let catalog = BuiltinCatalog::from_host_manifests(manifests).unwrap();
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, &ids[0]);
    let cli = open(&mut core, "cli");
    let preview = core
        .handle(
            "cli",
            request(
                Call::DependencyPlan(DependencyPlanRequest {
                    plugin_ids: vec![ids[0].clone()],
                }),
                Some(cli.clone()),
            ),
        )
        .outcome;
    let Outcome::DependencyPlan(preview) = preview else {
        panic!("{preview:?}");
    };
    assert_eq!(preview.lock.tools.len(), 50);
    assert!(serde_json::to_vec(&preview.lock).unwrap().len() > 32768);
    let outcome = core
        .handle(
            "cli",
            request(
                Call::Submit(operation(&ids[0], "oversize-valid-graph")),
                Some(cli),
            ),
        )
        .outcome;
    assert!(
        matches!(
            outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::BudgetExceeded
            })
        ),
        "{outcome:?}"
    );
    assert!(!core.stopping());
    assert!(core.pending_runs().is_empty());
    assert!(core
        .with_tool_fixture_store(|store| store.load_jobs().unwrap())
        .is_empty());
    assert!(!fs::read_dir(world.path.join("private-jobs"))
        .unwrap()
        .any(|entry| entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|e| e == "bin")));
    let path = image.join("bin/ffmpeg.exe");
    writable(&path);
    drop(write_handle(&path).unwrap());
    readonly(&path);
}
#[test]
fn restored_queued_jobs_recheck_actual_sealed_bytes_instead_of_trusting_a_saved_reference() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let mut first = core(&mut world, catalog.clone(), false);
    grant(&mut first, &catalog, "consumer");
    let mut operation = operation("consumer", "changed-image");
    operation.background = true;
    let receipt = submit(&mut first, operation);
    let accepted = row(&mut first, &receipt.run_id)["toolArtifacts"].clone();
    drop(first);
    let path = image.join("bin/ffmpeg.exe");
    writable(&path);
    fs::write(&path, b"CHANGED_INERT_BYTES").unwrap();
    readonly(&path);
    world.reopen();
    let mut reopened = core(&mut world, catalog, false);
    assert!(reopened.pending_runs().is_empty());
    assert_eq!(reopened.state(&receipt.run_id), Some(JobState::Interrupted));
    assert_eq!(
        row(&mut reopened, &receipt.run_id)["toolArtifacts"],
        accepted
    );
    assert!(world
        .path
        .join("private-jobs")
        .join(format!("{}.input.bin", receipt.run_id))
        .is_file());
    assert_eq!(fs::read(&path).unwrap(), b"CHANGED_INERT_BYTES");
}
