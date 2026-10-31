use super::super::reconciliation_tests::{authority, recover};
use super::*;
use crate::packages::install::tool_consumers::reconciliation::{
    abort_consumer_reconciliation, apply_consumer_reconciliation, apply_fixture_with_hook,
    plan_consumer_reconciliation, resume_consumer_reconciliation,
    ConsumerReconciliationPhase as Phase, ConsumerReconciliationPlan,
    ConsumerReconciliationSummary,
};
const JOURNAL: &str = "tool-consumer-reconciliation-pending-v1";
const RECEIPT: &str = "tool-consumer-reconciliation-receipt-v1";
fn plan(world: &mut World) -> Result<ConsumerReconciliationPlan, ToolInstallError> {
    plan_consumer_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn apply(
    world: &mut World,
    plan: ConsumerReconciliationPlan,
) -> Result<ConsumerReconciliationSummary, ToolInstallError> {
    apply_consumer_reconciliation(&world.trust, world.store.as_mut().unwrap(), plan)
}
fn resume(world: &mut World) -> Result<Option<ConsumerReconciliationSummary>, ToolInstallError> {
    resume_consumer_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn abort(world: &mut World) -> Result<bool, ToolInstallError> {
    abort_consumer_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn root(world: &World) -> PathBuf {
    world.path.join("tool-bound-plugin-store-v1")
}
fn orphan_world() -> World {
    orphan_world_and_source().0
}
fn orphan_world_and_source() -> (World, InstalledInertTool) {
    let mut world = world();
    let source = world.install().unwrap();
    let input = input(&world, "consumer", vec![], true, 1);
    install(&mut world, &input).unwrap();
    remove(&world, CONSUMERS);
    (world, source)
}
fn stage(world: &World, bytes: &[u8]) -> PathBuf {
    let path = root(world)
        .join("staging")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&path).unwrap();
    fs::write(path.join("package.zip"), bytes).unwrap();
    path
}
fn quarantine(world: &World) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(root(world).join("quarantine"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}
fn interrupt(world: &mut World, phase: Phase) {
    let reviewed = plan(world).unwrap();
    assert_eq!(
        apply_fixture_with_hook(
            &world.trust,
            world.store.as_mut().unwrap(),
            reviewed,
            |actual, _| if actual == phase {
                Err(InstallError::Storage.into())
            } else {
                Ok(())
            }
        )
        .err(),
        Some(ToolInstallError::Storage(InstallError::Storage))
    );
}
#[test]
fn empty_consumer_observation_never_creates_a_namespace_or_receipt() {
    let mut world = world();
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(
        reviewed.summary(),
        ConsumerReconciliationSummary {
            artifacts: 0,
            stages: 0,
            bytes: 0
        }
    );
    apply(&mut world, reviewed).unwrap();
    assert!(resume(&mut world).unwrap().is_none());
    assert!(!abort(&mut world).unwrap());
    assert!(!root(&world).exists());
    assert!(world.metadata(JOURNAL).is_none());
    assert!(world.metadata(RECEIPT).is_none());
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn reviewed_inert_orphans_quarantine_without_adopting_a_consumer_or_touching_its_tool_zip() {
    let mut world = orphan_world();
    let first = input(&world, "consumer", vec![], true, 1);
    let orphan = artifact(&world, &first);
    let bytes = fs::read(orphan.join("package.zip")).unwrap();
    let partial = stage(&world, b"PARTIAL_CONSUMER");
    let raw = world.metadata(KEY);
    let floors = world.metadata("package-trust-state-v1");
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(reviewed.summary().artifacts, 1);
    assert_eq!(reviewed.summary().stages, 1);
    assert!(orphan.exists());
    assert!(partial.exists());
    assert!(world.metadata(JOURNAL).is_none());
    assert_eq!(apply(&mut world, reviewed).unwrap().artifacts, 1);
    assert!(!orphan.exists());
    assert!(!partial.exists());
    assert_eq!(quarantine(&world).len(), 2);
    assert!(quarantine(&world)
        .iter()
        .any(|p| fs::read(p.join("package.zip")).unwrap() == bytes));
    assert!(inspect(&mut world).unwrap().is_empty());
    assert_eq!(world.metadata(KEY), raw);
    assert_eq!(world.metadata("package-trust-state-v1"), floors);
    assert!(world.metadata(CONSUMERS).is_none());
    let receipt = world.metadata(RECEIPT);
    let empty = plan(&mut world).unwrap();
    apply(&mut world, empty).unwrap();
    assert_eq!(world.metadata(RECEIPT), receipt);
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn actual_restore_quarantines_new_consumers_before_their_orphaned_raw_tools_and_keeps_old_locks() {
    for partial in [false, true] {
        let mut world = world();
        authority(&mut world);
        world.install().unwrap();
        let old = input(&world, "consumer", vec![], true, 1);
        install(&mut world, &old).unwrap();
        let old_inventory = world.metadata(CONSUMERS);
        let old_bytes = fs::read(artifact(&world, &old).join("package.zip")).unwrap();
        let backup = world.store.as_ref().unwrap().backup().unwrap();
        world.fixture.base.package.version = "9.0.0".into();
        world.fixture.base.package.release_sequence += 1;
        world.fixture.manifest["version"] = json!("9.0.0");
        world.fixture.rebuild();
        world.install().unwrap();
        let mut value = manifest("consumer", vec![]);
        value["version"] = json!("2.0.0");
        value["dependencies"]["tools"] = json!([{
            "publisher":world.fixture.base.package.publisher,"id":"ffmpeg","version":"*",
            "target":world.fixture.base.package.target,"buildFlavor":"fixture-static","digest":world.fixture.base.package.archive.sha256,
        }]);
        let new = input_value(&world, value, 2);
        if partial {
            assert!(with_hook(&mut world, &new, |phase, _| {
                if phase == super::Phase::StagePrepared {
                    Err(InstallError::Storage.into())
                } else {
                    Ok(())
                }
            })
            .is_err());
        } else {
            install(&mut world, &new).unwrap();
        }
        let now = world.fixture.now();
        let checkpoint = world
            .trust
            .export_recovery_checkpoint(world.store.as_mut().unwrap(), now + 1)
            .unwrap();
        drop(world.store.take());
        DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
        world.reopen();
        assert!(plan(&mut world).is_err());
        recover(&mut world, &checkpoint, now + 2);
        let floors = world.metadata("package-trust-state-v1");
        assert!(
            plan_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap()).is_err()
        );
        let reviewed = plan(&mut world).unwrap();
        assert_eq!(reviewed.summary().stages, usize::from(partial));
        assert_eq!(reviewed.summary().artifacts, usize::from(!partial));
        apply(&mut world, reviewed).unwrap();
        assert_eq!(world.metadata(CONSUMERS), old_inventory);
        assert_eq!(
            fs::read(artifact(&world, &old).join("package.zip")).unwrap(),
            old_bytes
        );
        let raw_plan =
            plan_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap()).unwrap();
        assert_eq!(raw_plan.summary().artifacts, 1);
        crate::packages::tools::install::reconciliation::apply_tool_archive_reconciliation(
            &world.trust,
            world.store.as_mut().unwrap(),
            raw_plan,
        )
        .unwrap();
        assert_eq!(world.metadata("package-trust-state-v1"), floors);
        assert_eq!(inspect(&mut world).unwrap().len(), 1);
        assert_eq!(quarantine(&world).len(), 1);
        assert!(fs::read_dir(world.root().join("quarantine"))
            .unwrap()
            .next()
            .is_some());
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
#[test]
fn the_entire_consumer_batch_must_still_match_before_the_first_native_move() {
    for attack in ["bytes", "extra", "group", "hardlink"] {
        let mut world = orphan_world();
        let input = input(&world, "consumer", vec![], true, 1);
        let partial = stage(&world, b"STAGE");
        let reviewed = plan(&mut world).unwrap();
        let source = artifact(&world, &input).join("package.zip");
        match attack {
            "bytes" => {
                writable(&source);
                fs::write(&source, b"ALTERED").unwrap();
            }
            "extra" => {
                fs::write(partial.join("outside-name.txt"), b"EXTRA").unwrap();
            }
            "group" => {
                stage(&world, b"NEW");
            }
            "hardlink" => {
                fs::hard_link(&source, world.path.join("outside-link.zip")).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(apply(&mut world, reviewed).is_err(), "{attack}");
        assert!(artifact(&world, &input).exists());
        assert!(partial.exists());
        assert!(quarantine(&world).is_empty());
        assert!(world.metadata(JOURNAL).is_none());
    }
}
#[test]
fn consumer_plan_cannot_cross_profiles_or_an_existing_inventory_revision() {
    let mut a = orphan_world();
    let mut b = orphan_world();
    let reviewed = plan(&mut a).unwrap();
    assert!(apply(&mut b, reviewed).is_err());
    assert!(quarantine(&a).is_empty());
    assert!(quarantine(&b).is_empty());
    let original = a.metadata("package-trust-fixture-scope");
    let reviewed = plan(&mut a).unwrap();
    set(&a, "package-trust-fixture-scope", "1");
    assert!(apply(&mut a, reviewed).is_err());
    assert!(quarantine(&a).is_empty());
    if let Some(text) = original {
        set(&a, "package-trust-fixture-scope", &text);
    } else {
        remove(&a, "package-trust-fixture-scope");
    }
    // A valid known record changes its installation observation, not its bytes.
    let mut world = world();
    world.install().unwrap();
    let known = input(&world, "middle", vec![], true, 1);
    install(&mut world, &known).unwrap();
    stage(&world, b"ORPHAN");
    let reviewed = plan(&mut world).unwrap();
    let mut value: Value = serde_json::from_str(&world.metadata(CONSUMERS).unwrap()).unwrap();
    value["records"][0]["installedAt"] =
        json!(value["records"][0]["installedAt"].as_u64().unwrap() + 1);
    set(&world, CONSUMERS, &value.to_string());
    assert!(apply(&mut world, reviewed).is_err());
    assert!(quarantine(&world).is_empty());
}
#[test]
fn consumer_reconciliation_resumes_exactly_after_each_barrier_and_explicit_abort_keeps_unmoved_groups(
) {
    for phase in [
        Phase::JournalCommitted,
        Phase::GroupQuarantined(0),
        Phase::BeforeCommit,
        Phase::Committed,
    ] {
        let mut world = orphan_world();
        stage(&world, b"SECOND");
        interrupt(&mut world, phase);
        world.reopen();
        if phase == Phase::Committed {
            assert!(resume(&mut world).unwrap().is_none());
        } else {
            assert_eq!(resume(&mut world).unwrap().unwrap().artifacts, 1);
        }
        assert!(world.metadata(JOURNAL).is_none());
        assert_eq!(quarantine(&world).len(), 2);
        assert!(resume(&mut world).unwrap().is_none());
    }
    let mut world = orphan_world();
    let remaining = stage(&world, b"REMAINING");
    interrupt(&mut world, Phase::GroupQuarantined(0));
    assert!(abort(&mut world).unwrap());
    assert!(remaining.exists());
    assert_eq!(quarantine(&world).len(), 1);
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(reviewed.summary().stages, 1);
    apply(&mut world, reviewed).unwrap();
    assert_eq!(quarantine(&world).len(), 2);
}
#[test]
fn a_pending_consumer_reconciliation_blocks_all_other_surfaces_even_when_its_text_is_corrupt() {
    let (mut world, source) = orphan_world_and_source();
    let input = input(&world, "consumer", vec![], true, 1);
    interrupt(&mut world, Phase::JournalCommitted);
    let original = world.metadata(JOURNAL).unwrap();
    let expected = Some(ToolInstallError::Storage(InstallError::RecoveryPending));
    for text in [original.as_str(), "CORRUPT"] {
        set(&world, JOURNAL, text);
        assert_eq!(world.install().err(), expected);
        assert_eq!(world.pins().err(), expected);
        assert_eq!(world.abort().err(), expected);
        assert_eq!(install(&mut world, &input).err(), expected);
        assert_eq!(inspect(&mut world).err(), expected);
        assert_eq!(super::abort(&mut world).err(), expected);
        assert_eq!(
            inspect_inert_tool_images(&world.trust, world.store.as_mut().unwrap()).err(),
            expected
        );
        let now = world.fixture.now();
        assert_eq!(
            materialize_fixture_with_hook(
                &world.trust,
                world.store.as_mut().unwrap(),
                &source,
                || now,
                |_, _| Ok(())
            )
            .err(),
            expected
        );
        assert_eq!(
            crate::packages::tools::install::images::abort_inert_tool_materialization(
                &world.trust,
                world.store.as_mut().unwrap()
            )
            .err(),
            expected
        );
        assert_eq!(
            plan_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap()).err(),
            expected
        );
        assert_eq!(
            plan_image_reconciliation(&world.trust, world.store.as_mut().unwrap()).err(),
            expected
        );
        assert_eq!(
            install::inspect_inert_inventory(&world.trust, world.store.as_mut().unwrap()).err(),
            Some(InstallError::RecoveryPending)
        );
        assert_eq!(
            install::abort_inert_install(&world.trust, world.store.as_mut().unwrap()).err(),
            Some(InstallError::RecoveryPending)
        );
        assert_eq!(
            install::plan_inert_reconciliation(&world.trust, world.store.as_mut().unwrap()).err(),
            Some(InstallError::RecoveryPending)
        );
        assert_eq!(
            plan(&mut world).err().map(|e| format!("{e:?}")),
            Some("Storage(RecoveryPending)".into())
        );
        assert_eq!(world.metadata(JOURNAL).as_deref(), Some(text));
        assert!(quarantine(&world).is_empty());
    }
    set(&world, JOURNAL, &original);
    resume(&mut world).unwrap();
    assert_eq!(quarantine(&world).len(), 1);
}
#[test]
fn every_foreign_pending_refuses_consumer_reconciliation_without_clearing_or_parsing_it() {
    for key in [
        "package-install-pending-v1",
        "package-reconciliation-pending-v1",
        PENDING,
        "tool-archive-reconciliation-pending-v1",
        "tool-image-materialization-pending-v1",
        "tool-image-reconciliation-pending-v1",
        CONSUMER_PENDING,
    ] {
        let mut world = orphan_world();
        let reviewed = plan(&mut world).unwrap();
        set(&world, key, "CORRUPT");
        let expected = Some(ToolInstallError::Storage(InstallError::RecoveryPending));
        assert_eq!(
            plan(&mut world).err().map(|e| format!("{e:?}")),
            Some("Storage(RecoveryPending)".into()),
            "{key}"
        );
        assert_eq!(apply(&mut world, reviewed).err(), expected);
        assert_eq!(resume(&mut world).err(), expected);
        assert_eq!(abort(&mut world).err(), expected);
        assert_eq!(world.metadata(key).as_deref(), Some("CORRUPT"));
        assert!(world.metadata(JOURNAL).is_none());
        assert!(quarantine(&world).is_empty());
    }
}
#[test]
fn malformed_consumer_reconciliation_journals_preserve_all_bytes_and_never_select_outside_paths() {
    let mut world = orphan_world();
    let input = input(&world, "consumer", vec![], true, 1);
    interrupt(&mut world, Phase::JournalCommitted);
    let original = world.metadata(JOURNAL).unwrap();
    for attack in [
        "version",
        "profile",
        "before",
        "escape",
        "destination",
        "duplicate",
        "hash",
        "size",
        "unknown",
        "duplicate-json",
        "oversize",
    ] {
        let mut value: Value = serde_json::from_str(&original).unwrap();
        match attack {
            "version" => value["formatVersion"] = json!(0),
            "profile" => value["profileDigest"] = json!("0".repeat(64)),
            "before" => value["beforeDigest"] = json!("0".repeat(64)),
            "escape" => value["orphans"][0]["source"] = json!("../outside"),
            "destination" => value["orphans"][0]["destinationId"] = json!("../outside"),
            "duplicate" => {
                let first = value["orphans"][0].clone();
                value["orphans"].as_array_mut().unwrap().push(first);
            }
            "hash" => value["orphans"][0]["files"]["package.zip"]["sha256"] = json!("BAD"),
            "size" => value["orphans"][0]["files"]["package.zip"]["size"] = json!(67_108_865_u64),
            "unknown" => value["approved"] = json!(true),
            _ => (),
        }
        let text = match attack {
            "duplicate-json" => format!("{{\"formatVersion\":1,{}", &original[1..]),
            "oversize" => " ".repeat(16_777_217),
            _ => value.to_string(),
        };
        set(&world, JOURNAL, &text);
        assert!(resume(&mut world).is_err(), "{attack}");
        assert!(abort(&mut world).is_err(), "{attack}");
        assert_eq!(world.metadata(JOURNAL).as_deref(), Some(text.as_str()));
        assert!(artifact(&world, &input).exists());
        assert!(quarantine(&world).is_empty());
    }
    set(&world, JOURNAL, &original);
    resume(&mut world).unwrap();
}
#[test]
fn a_real_signed_revocation_between_groups_stops_the_next_move_and_abort_binds_the_new_scope() {
    let mut world = orphan_world();
    let remaining = stage(&world, b"REMAINING");
    let snapshot = world.trust.snapshot(world.store.as_mut().unwrap()).unwrap();
    let mut root: RootPayload = serde_json::from_slice(
        &decode_envelope(
            &serde_json::to_vec(&world.fixture.base.golden["rootEnvelope"]).unwrap(),
            ROOT_PAYLOAD_TYPE,
        )
        .unwrap()
        .payload,
    )
    .unwrap();
    root.version += 1;
    root.previous_root_sha256 = Some(snapshot.root_digest);
    root.revoked_package_digests
        .push(world.fixture.base.package.archive.sha256.clone());
    let envelope = signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]);
    let now = world.fixture.now();
    let reviewed = plan(&mut world).unwrap();
    let trust = &world.trust;
    assert_eq!(
        apply_fixture_with_hook(
            trust,
            world.store.as_mut().unwrap(),
            reviewed,
            |phase, store| {
                if phase == Phase::GroupQuarantined(0) {
                    trust.rotate(store, &envelope, now).unwrap();
                }
                Ok(())
            }
        )
        .err(),
        Some(ToolInstallError::Storage(InstallError::InventoryChanged))
    );
    assert!(remaining.exists());
    assert_eq!(quarantine(&world).len(), 1);
    let pending: Value = serde_json::from_str(&world.metadata(JOURNAL).unwrap()).unwrap();
    assert!(resume(&mut world).is_err());
    let floors = world.metadata("package-trust-state-v1");
    assert!(abort(&mut world).unwrap());
    let receipt: Value = serde_json::from_str(&world.metadata(RECEIPT).unwrap()).unwrap();
    assert_eq!(receipt["outcome"], "aborted");
    assert_ne!(receipt["committedScopeDigest"], pending["scopeDigest"]);
    assert_eq!(world.metadata("package-trust-state-v1"), floors);
    assert!(remaining.exists());
    let reviewed = plan(&mut world).unwrap();
    apply(&mut world, reviewed).unwrap();
}
#[test]
fn restored_consumer_reconciliation_requires_controlled_trust_recovery_and_explicit_scope_cancellation(
) {
    let mut world = orphan_world();
    authority(&mut world);
    let remaining = stage(&world, b"REMAINING");
    interrupt(&mut world, Phase::GroupQuarantined(0));
    let pending = world.metadata(JOURNAL);
    let backup = world.store.as_ref().unwrap().backup().unwrap();
    let now = world.fixture.now();
    let checkpoint = world
        .trust
        .export_recovery_checkpoint(world.store.as_mut().unwrap(), now + 1)
        .unwrap();
    drop(world.store.take());
    DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
    world.reopen();
    assert!(resume(&mut world).is_err());
    assert!(abort(&mut world).is_err());
    assert_eq!(world.metadata(JOURNAL), pending);
    assert!(remaining.exists());
    recover(&mut world, &checkpoint, now + 2);
    assert!(resume(&mut world).is_err());
    assert!(abort(&mut world).unwrap());
    assert!(remaining.exists());
    assert_eq!(quarantine(&world).len(), 1);
}
#[test]
fn journal_creation_triggers_cannot_commit_changed_consumer_source_inventory_or_trust_before_effects(
) {
    for key in [
        CONSUMERS,
        KEY,
        "package-trust-state-v1",
        JOURNAL,
        "package-install-pending-v1",
    ] {
        let mut world = orphan_world();
        let reviewed = plan(&mut world).unwrap();
        let original = world.metadata(key);
        world.store.as_ref().unwrap().connection.execute_batch(&format!("CREATE TRIGGER substitute AFTER INSERT ON core_metadata WHEN NEW.key='{JOURNAL}' BEGIN INSERT INTO core_metadata(key,value) VALUES('{key}','CORRUPT') ON CONFLICT(key) DO UPDATE SET value=excluded.value; END;")).unwrap();
        assert!(apply(&mut world, reviewed).is_err(), "{key}");
        assert_eq!(world.metadata(key), original);
        assert!(world.metadata(JOURNAL).is_none());
        assert!(world.metadata(RECEIPT).is_none());
        assert!(quarantine(&world).is_empty());
    }
}
#[test]
fn receipt_and_journal_triggers_roll_back_final_metadata_after_quarantine() {
    for on_delete in [false, true] {
        for key in [
            CONSUMERS,
            KEY,
            "package-trust-state-v1",
            JOURNAL,
            RECEIPT,
            "package-install-pending-v1",
        ] {
            let mut world = orphan_world();
            interrupt(&mut world, Phase::BeforeCommit);
            let pending = world.metadata(JOURNAL);
            let original = world.metadata(key);
            let event = if on_delete {
                format!("AFTER DELETE ON core_metadata WHEN OLD.key='{JOURNAL}'")
            } else {
                format!("AFTER INSERT ON core_metadata WHEN NEW.key='{RECEIPT}'")
            };
            world.store.as_ref().unwrap().connection.execute_batch(&format!("CREATE TRIGGER substitute {event} BEGIN INSERT INTO core_metadata(key,value) VALUES('{key}','CORRUPT') ON CONFLICT(key) DO UPDATE SET value=excluded.value; END;")).unwrap();
            assert!(resume(&mut world).is_err(), "{on_delete}/{key}");
            assert_eq!(world.metadata(JOURNAL), pending);
            assert_eq!(world.metadata(key), original);
            assert_eq!(quarantine(&world).len(), 1);
            world
                .store
                .as_ref()
                .unwrap()
                .connection
                .execute_batch("DROP TRIGGER substitute")
                .unwrap();
            resume(&mut world).unwrap();
        }
    }
}
#[test]
fn every_physical_pool_still_counts_towards_the_original_budget_before_consumer_recovery() {
    for byte_budget in [false, true] {
        let mut world = orphan_world();
        let reviewed = plan(&mut world).unwrap();
        let parent = world.root().join("quarantine");
        if byte_budget {
            for _ in 0..8 {
                let group = parent.join(format!("{}-stage", uuid::Uuid::new_v4()));
                fs::create_dir(&group).unwrap();
                fs::File::create(group.join("package.zip"))
                    .unwrap()
                    .set_len(33_554_432)
                    .unwrap();
            }
        } else {
            for _ in 0..383 {
                fs::create_dir(parent.join(format!("{}-stage", uuid::Uuid::new_v4()))).unwrap();
            }
        }
        assert_eq!(
            apply(&mut world, reviewed).err(),
            Some(ToolInstallError::Storage(InstallError::BudgetExceeded))
        );
        assert!(world.metadata(JOURNAL).is_none());
        assert!(quarantine(&world).is_empty());
    }
}
#[test]
fn corruption_of_known_consumer_bytes_refuses_observing_or_moving_unrelated_orphans() {
    let mut world = world();
    world.install().unwrap();
    let known = input(&world, "middle", vec![], true, 1);
    install(&mut world, &known).unwrap();
    let partial = stage(&world, b"ORPHAN");
    let reviewed = plan(&mut world).unwrap();
    let source = artifact(&world, &known).join("package.zip");
    writable(&source);
    fs::write(source, b"CHANGED_KNOWN").unwrap();
    assert!(apply(&mut world, reviewed).is_err());
    assert!(plan(&mut world).is_err());
    assert!(partial.exists());
    assert!(quarantine(&world).is_empty());
    assert!(world.metadata(JOURNAL).is_none());
}
#[test]
fn a_quarantine_collision_never_replaces_destination_bytes_or_clears_the_pending_batch() {
    let mut world = orphan_world();
    let input = input(&world, "consumer", vec![], true, 1);
    let reviewed = plan(&mut world).unwrap();
    let parent = root(&world).join("quarantine");
    assert!(apply_fixture_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        reviewed,
        |phase, store| {
            if phase == Phase::JournalCommitted {
                let text: String = store
                    .connection
                    .query_row(
                        "SELECT value FROM core_metadata WHERE key=?1",
                        [JOURNAL],
                        |r| r.get(0),
                    )
                    .unwrap();
                let value: Value = serde_json::from_str(&text).unwrap();
                let destination = parent.join(format!(
                    "{}-artifact",
                    value["orphans"][0]["destinationId"].as_str().unwrap()
                ));
                fs::create_dir(&destination).unwrap();
                fs::write(destination.join("package.zip"), b"DESTINATION_CANARY").unwrap();
            }
            Ok(())
        }
    )
    .is_err());
    assert!(artifact(&world, &input).exists());
    assert_eq!(
        fs::read(quarantine(&world)[0].join("package.zip")).unwrap(),
        b"DESTINATION_CANARY"
    );
    let pending = world.metadata(JOURNAL);
    assert!(resume(&mut world).is_err());
    assert!(abort(&mut world).is_err());
    assert_eq!(world.metadata(JOURNAL), pending);
}
#[test]
fn native_group_and_leaf_junctions_cannot_redirect_consumer_observation_outside_the_namespace() {
    for leaf in [false, true] {
        let mut world = orphan_world();
        let outside = world.path.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("canary.txt"), b"OUTSIDE_UNCHANGED").unwrap();
        let link = if leaf {
            let group = root(&world)
                .join("staging")
                .join(uuid::Uuid::new_v4().to_string());
            fs::create_dir(&group).unwrap();
            group.join("package.zip")
        } else {
            root(&world).join("artifacts").join("a".repeat(64))
        };
        let status = Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        assert!(plan(&mut world).is_err());
        assert_eq!(
            fs::read(outside.join("canary.txt")).unwrap(),
            b"OUTSIDE_UNCHANGED"
        );
        assert!(world.metadata(JOURNAL).is_none());
        assert!(quarantine(&world).is_empty());
        fs::remove_dir(&link).unwrap();
    }
}
#[test]
fn consumer_reconciliation_crash_child() {
    let Ok(path) = std::env::var("FLOWTOOLS_CONSUMER_RECONCILE_PROFILE") else {
        return;
    };
    let phase = std::env::var("FLOWTOOLS_CONSUMER_RECONCILE_PHASE").unwrap();
    let profile = Path::new(&path);
    assert_eq!(profile.parent().unwrap(), std::env::temp_dir());
    assert!(profile
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("flowtools-tool-install-")
        .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok_and(|id| id.to_string() == s)));
    assert!(profile.join("runtime.sqlite").is_file());
    let mut world = open(profile, false, false);
    let reviewed = plan(&mut world).unwrap();
    let _ = apply_fixture_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        reviewed,
        |actual, _| {
            if format!("{actual:?}") == phase {
                std::process::exit(82);
            }
            Ok(())
        },
    );
    panic!("consumer reconciliation crash barrier was not reached");
}
#[test]
fn actual_process_exit_at_five_reconciliation_barriers_preserves_the_exact_batch_and_source_tool() {
    for phase in [
        Phase::JournalCommitted,
        Phase::GroupQuarantined(0),
        Phase::GroupQuarantined(1),
        Phase::BeforeCommit,
        Phase::Committed,
    ] {
        let mut world = orphan_world();
        let input = input(&world, "consumer", vec![], true, 1);
        stage(&world, b"PARTIAL_CONSUMER");
        let raw = world.metadata(KEY);
        drop(world.store.take());
        let mut child=Command::new(std::env::current_exe().unwrap())
            .args(["--exact","packages::tests::tool_tests::install_tests::consumer_tests::reconciliation_tests::consumer_reconciliation_crash_child","--nocapture","--test-threads=1"])
            .env("FLOWTOOLS_CONSUMER_RECONCILE_PROFILE",&world.path).env("FLOWTOOLS_CONSUMER_RECONCILE_PHASE",format!("{phase:?}"))
            .stdin(std::process::Stdio::null()).spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("owned consumer reconciliation fixture exceeded 20s: {phase:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(82), "{phase:?}");
        world.reopen();
        if phase == Phase::Committed {
            assert!(resume(&mut world).unwrap().is_none());
        } else {
            assert_eq!(resume(&mut world).unwrap().unwrap().artifacts, 1);
        }
        assert_eq!(quarantine(&world).len(), 2);
        assert!(quarantine(&world)
            .iter()
            .any(|p| fs::read(p.join("package.zip")).unwrap() == input.archive));
        assert_eq!(world.metadata(KEY), raw);
        assert!(world.metadata(CONSUMERS).is_none());
        assert!(world.metadata(JOURNAL).is_none());
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
