use super::super::reconciliation_tests::{authority, recover};
use super::*;
use crate::packages::tools::install::images::reconciliation::{
    abort_image_reconciliation, apply_fixture_with_hook, apply_image_reconciliation,
    plan_image_reconciliation, resume_image_reconciliation, ImageReconciliationPhase as Phase,
    ImageReconciliationPlan, ImageReconciliationSummary,
};
use crate::packages::tools::install::reconciliation as raw;
const JOURNAL: &str = "tool-image-reconciliation-pending-v1";
const RECEIPT: &str = "tool-image-reconciliation-receipt-v1";
fn plan(world: &mut World) -> Result<ImageReconciliationPlan, ToolInstallError> {
    plan_image_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn apply(
    world: &mut World,
    reviewed: ImageReconciliationPlan,
) -> Result<ImageReconciliationSummary, ToolInstallError> {
    apply_image_reconciliation(&world.trust, world.store.as_mut().unwrap(), reviewed)
}
fn resume(world: &mut World) -> Result<Option<ImageReconciliationSummary>, ToolInstallError> {
    resume_image_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn abort(world: &mut World) -> Result<bool, ToolInstallError> {
    abort_image_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn set(world: &World, key: &str, text: &str) {
    world.store.as_ref().unwrap().connection.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[key,text]).unwrap();
}
fn remove(world: &World, key: &str) {
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute("DELETE FROM core_metadata WHERE key=?1", [key])
        .unwrap();
}
fn orphan_image() -> World {
    let mut world = World::new();
    let source = world.install().unwrap();
    drop(materialize(&mut world, &source).unwrap());
    remove(&world, IMAGE_KEY);
    world
}
fn partial_stage(world: &World) -> PathBuf {
    let root = world
        .path
        .join("tool-image-store-v1/staging")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    fs::create_dir(root.join("bin/nested")).unwrap();
    fs::write(root.join("bin/nested/partial.dat"), b"PARTIAL_OWNED_BYTES").unwrap();
    root
}
fn quarantine(world: &World) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(world.path.join("tool-image-store-v1/quarantine"))
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}
fn interrupt(world: &mut World, at: Phase) {
    let reviewed = plan(world).unwrap();
    assert_eq!(
        apply_fixture_with_hook(
            &world.trust,
            world.store.as_mut().unwrap(),
            reviewed,
            |phase, _| {
                if phase == at {
                    Err(InstallError::Storage.into())
                } else {
                    Ok(())
                }
            }
        )
        .err(),
        Some(ToolInstallError::Storage(InstallError::Storage))
    );
}
#[test]
fn empty_observation_is_read_only_and_never_creates_an_image_namespace() {
    let mut world = World::new();
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(
        reviewed.summary(),
        ImageReconciliationSummary {
            artifacts: 0,
            stages: 0,
            bytes: 0
        }
    );
    apply(&mut world, reviewed).unwrap();
    assert!(resume(&mut world).unwrap().is_none());
    assert!(!abort(&mut world).unwrap());
    assert!(!world.path.join("tool-image-store-v1").exists());
    assert!(world.metadata(JOURNAL).is_none());
    assert!(world.metadata(RECEIPT).is_none());
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn real_restore_quarantines_newer_images_before_their_raw_zips_and_preserves_old_seals() {
    for stage in [false, true] {
        let mut world = World::new();
        authority(&mut world);
        let old = world.install().unwrap();
        drop(materialize(&mut world, &old).unwrap());
        let old_image = image(&world);
        let old_bytes = fs::read(old_image.join("bin/ffmpeg.exe")).unwrap();
        let old_inventory = world.metadata(IMAGE_KEY);
        let old_raw = world.metadata(KEY);
        let backup = world.store.as_ref().unwrap().backup().unwrap();
        world.fixture.base.package.version = "9.0.0".into();
        world.fixture.base.package.release_sequence += 1;
        world.fixture.manifest["version"] = json!("9.0.0");
        world.fixture.rebuild();
        let new_source = world.install().unwrap();
        if stage {
            assert!(with_hook(&mut world, &new_source, |phase, _| {
                if phase == MaterializePhase::StagePrepared {
                    Err(InstallError::Storage.into())
                } else {
                    Ok(())
                }
            })
            .is_err());
        } else {
            drop(materialize(&mut world, &new_source).unwrap());
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
            raw::plan_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap())
                .is_err()
        );
        let reviewed = plan(&mut world).unwrap();
        assert_eq!(reviewed.summary().stages, usize::from(stage));
        assert_eq!(reviewed.summary().artifacts, usize::from(!stage));
        apply(&mut world, reviewed).unwrap();
        assert_eq!(quarantine(&world).len(), 1);
        assert_eq!(
            fs::read(quarantine(&world)[0].join("bin/ffmpeg.exe")).unwrap(),
            world.fixture.contents["bin/ffmpeg.exe"]
        );
        let raw_plan =
            raw::plan_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap())
                .unwrap();
        assert_eq!(raw_plan.summary().artifacts, 1);
        raw::apply_tool_archive_reconciliation(
            &world.trust,
            world.store.as_mut().unwrap(),
            raw_plan,
        )
        .unwrap();
        assert_eq!(world.metadata(IMAGE_KEY), old_inventory);
        assert_eq!(world.metadata(KEY), old_raw);
        assert_eq!(world.metadata("package-trust-state-v1"), floors);
        assert_eq!(
            fs::read(old_image.join("bin/ffmpeg.exe")).unwrap(),
            old_bytes
        );
        assert_eq!(inspect(&mut world).unwrap(), vec![old.pin()]);
        assert_eq!(world.pins().unwrap(), vec![old.pin()]);
        let mut rollback = ToolFixture::new(CompressionMethod::Stored);
        rollback.base.policy.clock.now = world.fixture.now();
        assert_eq!(
            rollback
                .admit(world.store.as_mut().unwrap(), &world.trust)
                .err(),
            Some(ToolAdmissionError::Package(PackageError::ReleaseRollback))
        );
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
#[test]
fn complete_batch_refuses_changed_bytes_attributes_security_extra_nodes_or_groups_before_effects() {
    for attack in [
        "bytes",
        "readonly",
        "security",
        "extra",
        "new-group",
        "hardlink",
    ] {
        let mut world = orphan_image();
        let second = partial_stage(&world);
        let reviewed = plan(&mut world).unwrap();
        let path = image(&world).join("bin/ffmpeg.exe");
        match attack {
            "bytes" => {
                writable(&path);
                fs::write(&path, b"CHANGED").unwrap();
            }
            "readonly" => writable(&path),
            "security" => harness::protect(
                &path,
                &format!("D:P(A;;FA;;;{})(A;;FR;;;WD)", harness::user_sid().unwrap()),
                false,
            )
            .unwrap(),
            "extra" => fs::write(second.join("unexpected.txt"), b"EXTRA").unwrap(),
            "new-group" => {
                partial_stage(&world);
            }
            "hardlink" => fs::hard_link(&path, world.path.join("outside-link.exe")).unwrap(),
            _ => unreachable!(),
        }
        assert!(apply(&mut world, reviewed).is_err(), "{attack}");
        assert!(image(&world).exists());
        assert!(second.exists());
        assert!(quarantine(&world).is_empty());
        assert!(world.metadata(JOURNAL).is_none());
    }
}
#[test]
fn active_images_still_require_exact_seals_and_cannot_be_observed_as_orphans() {
    let mut world = World::new();
    let source = world.install().unwrap();
    drop(materialize(&mut world, &source).unwrap());
    let second = partial_stage(&world);
    let reviewed = plan(&mut world).unwrap();
    let path = image(&world).join("seal.json");
    writable(&path);
    fs::write(&path, b"TAMPERED_SEAL").unwrap();
    assert!(apply(&mut world, reviewed).is_err());
    assert!(plan(&mut world).is_err());
    assert!(second.exists());
    assert!(quarantine(&world).is_empty());
    assert!(world.metadata(JOURNAL).is_none());
}
#[test]
fn partially_written_inherited_trees_resume_after_each_node_policy_without_adoption() {
    for node in 0..4 {
        let mut world = orphan_image();
        // Keep only a genuinely partial inherited-policy stage in this batch.
        let reviewed = plan(&mut world).unwrap();
        apply(&mut world, reviewed).unwrap();
        let stage = partial_stage(&world);
        interrupt(&mut world, Phase::BeforeNodeProtection(0, node));
        assert!(!stage.exists());
        world.reopen();
        assert_eq!(resume(&mut world).unwrap().unwrap().stages, 1);
        assert!(world.metadata(JOURNAL).is_none());
        let partial = quarantine(&world)
            .into_iter()
            .find(|p| p.join("bin/nested/partial.dat").exists())
            .unwrap();
        assert_eq!(
            fs::read(partial.join("bin/nested/partial.dat")).unwrap(),
            b"PARTIAL_OWNED_BYTES"
        );
        assert!(inspect(&mut world).unwrap().is_empty());
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
#[test]
fn pending_images_block_plugin_raw_and_materialization_surfaces_even_when_corrupt() {
    let mut world = World::new();
    let source = world.install().unwrap();
    drop(materialize(&mut world, &source).unwrap());
    remove(&world, IMAGE_KEY);
    interrupt(&mut world, Phase::JournalCommitted);
    let original = world.metadata(JOURNAL).unwrap();
    for text in [original.as_str(), "CORRUPT"] {
        set(&world, JOURNAL, text);
        let expected = Some(ToolInstallError::Storage(InstallError::RecoveryPending));
        assert_eq!(world.install().err(), expected);
        assert_eq!(world.pins().err(), expected);
        assert_eq!(world.abort().err(), expected);
        assert_eq!(inspect(&mut world).err(), expected);
        assert_eq!(super::abort(&mut world).err(), expected);
        assert_eq!(materialize(&mut world, &source).err(), expected);
        assert_eq!(
            raw::resume_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap())
                .err(),
            expected
        );
        assert_eq!(
            raw::abort_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap())
                .err(),
            expected
        );
        assert_eq!(
            inspect_inert_inventory(&world.trust, world.store.as_mut().unwrap()).err(),
            Some(InstallError::RecoveryPending)
        );
        assert_eq!(
            crate::packages::install::resume_inert_reconciliation(
                &world.trust,
                world.store.as_mut().unwrap()
            )
            .err(),
            Some(InstallError::RecoveryPending)
        );
        assert_eq!(
            crate::packages::install::abort_inert_reconciliation(
                &world.trust,
                world.store.as_mut().unwrap()
            )
            .err(),
            Some(InstallError::RecoveryPending)
        );
        assert_eq!(world.metadata(JOURNAL).as_deref(), Some(text));
    }
    set(&world, JOURNAL, &original);
    resume(&mut world).unwrap();
}
#[test]
fn other_pending_journals_are_never_cleared_or_bypassed() {
    for key in [
        PENDING,
        "package-install-pending-v1",
        "package-reconciliation-pending-v1",
        IMAGE_PENDING,
        "tool-archive-reconciliation-pending-v1",
    ] {
        let mut world = orphan_image();
        set(&world, key, "CORRUPT");
        assert!(plan(&mut world).is_err());
        assert!(resume(&mut world).is_err());
        assert!(abort(&mut world).is_err());
        assert_eq!(world.metadata(key).as_deref(), Some("CORRUPT"));
        assert!(image(&world).exists());
        assert!(quarantine(&world).is_empty());
    }
}
#[test]
fn cross_profile_inventory_and_scope_changes_refuse_without_native_effects() {
    let mut a = orphan_image();
    let mut b = orphan_image();
    let reviewed = plan(&mut a).unwrap();
    assert!(apply(&mut b, reviewed).is_err());
    for key in [
        "package-install-inventory-v1",
        "package-trust-fixture-scope",
    ] {
        let reviewed = plan(&mut a).unwrap();
        set(&a, key, "{}");
        assert!(apply(&mut a, reviewed).is_err());
        assert!(quarantine(&a).is_empty());
        assert!(image(&a).exists());
        remove(&a, key);
    }
    let backup = a.store.as_ref().unwrap().backup().unwrap();
    drop(a.store.take());
    DataStore::restore_backup(&a.path.join("runtime.sqlite"), &backup).unwrap();
    a.reopen();
    assert!(plan(&mut a).is_err());
    assert!(quarantine(&a).is_empty());
}
#[test]
fn changed_scope_can_only_abort_and_leaves_remaining_sources_in_place() {
    let mut world = orphan_image();
    let remaining = partial_stage(&world);
    interrupt(&mut world, Phase::BeforeNodeProtection(0, 1));
    set(&world, "package-trust-fixture-scope", "1");
    assert!(resume(&mut world).is_err());
    assert!(remaining.exists());
    assert!(abort(&mut world).unwrap());
    assert!(!abort(&mut world).unwrap());
    assert!(remaining.exists());
    assert_eq!(quarantine(&world).len(), 1);
    assert_eq!(
        world.metadata("package-trust-fixture-scope").as_deref(),
        Some("1")
    );
    assert_eq!(
        serde_json::from_str::<Value>(&world.metadata(RECEIPT).unwrap()).unwrap()["outcome"],
        "aborted"
    );
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(reviewed.summary().stages, 1);
    apply(&mut world, reviewed).unwrap();
}

#[test]
fn frozen_metadata_is_rechecked_immediately_before_each_native_policy_effect() {
    for key in [
        "package-trust-fixture-scope",
        "package-install-pending-v1",
        KEY,
    ] {
        let mut world = orphan_image();
        let original = world.metadata(key);
        let reviewed = plan(&mut world).unwrap();
        let result = apply_fixture_with_hook(
            &world.trust,
            world.store.as_mut().unwrap(),
            reviewed,
            |phase, store| {
                if phase == Phase::BeforeNodeProtection(0, 0) {
                    store.connection.execute("INSERT INTO core_metadata(key,value) VALUES(?1,'CHANGED') ON CONFLICT(key) DO UPDATE SET value='CHANGED'",[key]).unwrap();
                }
                Ok(())
            },
        );
        assert_eq!(
            result.err(),
            Some(ToolInstallError::Storage(InstallError::InventoryChanged))
        );
        assert!(world.metadata(JOURNAL).is_some());
        assert_eq!(quarantine(&world).len(), 1);
        // No policy write was allowed; this exact captured descriptor is still
        // original. Restore test metadata, then the same batch can resume.
        if let Some(text) = original {
            set(&world, key, &text);
        } else {
            remove(&world, key);
        }
        if key == "package-trust-fixture-scope" {
            let mut other = World::new();
            let source = other.install().unwrap();
            drop(materialize(&mut other, &source).unwrap());
            let root = quarantine(&world).pop().unwrap();
            assert!(lpac_report(&world, &other, &root, true)["bytes"].is_array());
        }
        resume(&mut world).unwrap();
        assert!(inspect(&mut world).unwrap().is_empty());
    }
}
#[test]
fn changed_security_after_partial_privatization_is_preserved_and_refuses_resume_and_abort() {
    let mut world = orphan_image();
    interrupt(&mut world, Phase::BeforeNodeProtection(0, 1));
    let root = quarantine(&world).pop().unwrap();
    let path = root.join("bin/ffmpeg.exe");
    harness::protect(
        &path,
        &format!(
            "D:P(A;;FA;;;{})(A;;FRFX;;;WD)",
            harness::user_sid().unwrap()
        ),
        false,
    )
    .unwrap();
    let pending = world.metadata(JOURNAL);
    let bytes = fs::read(&path).unwrap();
    assert!(resume(&mut world).is_err());
    assert!(abort(&mut world).is_err());
    assert_eq!(world.metadata(JOURNAL), pending);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(world.metadata(RECEIPT).is_none());
}
#[test]
fn signed_revocation_between_node_effects_stops_the_batch_without_resetting_trust() {
    let mut world = orphan_image();
    let bytes = serde_json::to_vec(&world.fixture.base.golden["rootEnvelope"]).unwrap();
    let payload = decode_envelope(&bytes, ROOT_PAYLOAD_TYPE).unwrap().payload;
    let mut root: RootPayload = serde_json::from_slice(&payload).unwrap();
    root.version += 1;
    root.previous_root_sha256 = Some(hash(&payload));
    root.revoked_package_digests
        .push(world.fixture.base.package.archive.sha256.clone());
    let trust = world.fixture.trust();
    let now = world.fixture.now();
    let reviewed = plan(&mut world).unwrap();
    assert!(apply_fixture_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        reviewed,
        |phase, store| {
            if phase == Phase::BeforeNodeProtection(0, 1) {
                trust
                    .rotate(store, &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]), now)
                    .unwrap();
            }
            Ok(())
        }
    )
    .is_err());
    let current = world.metadata("package-trust-state-v1");
    assert!(world.metadata(JOURNAL).is_some());
    assert!(resume(&mut world).is_err());
    assert!(abort(&mut world).unwrap());
    assert_eq!(world.metadata("package-trust-state-v1"), current);
    assert_eq!(quarantine(&world).len(), 1);
    assert!(inspect(&mut world).unwrap().is_empty());
}
#[test]
fn restored_pending_batch_requires_current_controlled_recovery_then_explicit_abort() {
    let mut world = orphan_image();
    authority(&mut world);
    interrupt(&mut world, Phase::JournalCommitted);
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
    assert!(image(&world).exists());
    assert!(quarantine(&world).is_empty());
    recover(&mut world, &checkpoint, now + 2);
    let current = world.metadata("package-trust-state-v1");
    assert!(resume(&mut world).is_err());
    assert!(abort(&mut world).unwrap());
    assert!(image(&world).exists());
    assert!(quarantine(&world).is_empty());
    assert_eq!(world.metadata("package-trust-state-v1"), current);
    let reviewed = plan(&mut world).unwrap();
    apply(&mut world, reviewed).unwrap();
}
#[test]
fn actual_lpac_loses_artifact_read_access_before_a_failed_journal_clear_and_after_abort() {
    let mut world = orphan_image();
    let mut other = World::new();
    let source = other.install().unwrap();
    drop(materialize(&mut other, &source).unwrap());
    let before = lpac_report(&world, &other, &image(&world), true);
    assert!(before["bytes"].is_array());
    interrupt(&mut world, Phase::GroupMoved(0));
    let pending = world.metadata(JOURNAL);
    let root = quarantine(&world).pop().unwrap();
    assert!(lpac_report(&world, &other, &root, true)["bytes"].is_array());
    world.store.as_ref().unwrap().connection.execute_batch(&format!("CREATE TRIGGER fail_image_reconciliation_clear BEFORE DELETE ON core_metadata WHEN OLD.key='{JOURNAL}' BEGIN SELECT RAISE(ABORT,'fixture'); END;")).unwrap();
    assert!(abort(&mut world).is_err());
    assert_eq!(world.metadata(JOURNAL), pending);
    let denied = lpac_report(&world, &other, &root, true);
    assert!(denied["bytes"].is_null());
    assert!(denied["identity"].is_null());
    assert_eq!(
        fs::read(root.join("bin/ffmpeg.exe")).unwrap(),
        world.fixture.contents["bin/ffmpeg.exe"]
    );
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute_batch("DROP TRIGGER fail_image_reconciliation_clear;")
        .unwrap();
    assert!(abort(&mut world).unwrap());
    assert!(world.metadata(JOURNAL).is_none());
    assert!(lpac_report(&world, &other, &root, true)["bytes"].is_null());
    assert!(inspect(&mut world).unwrap().is_empty());
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn malformed_journals_and_forged_paths_identities_or_known_images_never_trigger_effects() {
    let mut world = orphan_image();
    interrupt(&mut world, Phase::JournalCommitted);
    let original = world.metadata(JOURNAL).unwrap();
    for mode in [
        "escape",
        "duplicate",
        "unknown",
        "identity",
        "rootless",
        "parentless",
        "oversize",
        "duplicate-json",
        "empty",
    ] {
        let mut text: Value = serde_json::from_str(&original).unwrap();
        match mode {
            "escape" => text["orphans"][0]["source"] = json!("../outside"),
            "duplicate" => {
                let o = text["orphans"][0].clone();
                text["orphans"].as_array_mut().unwrap().push(o);
            }
            "unknown" => text["approved"] = json!(true),
            "identity" => {
                text["orphans"][0]["observation"]["files"][0]["pin"]["identity"]["fileId"] =
                    json!("INVALID")
            }
            "rootless" => {
                text["orphans"][0]["observation"]["directories"]
                    .as_array_mut()
                    .unwrap()
                    .remove(0);
            }
            "parentless" => {
                text["orphans"][0]["observation"]["files"][0]["pin"]["content"]["path"] =
                    json!("missing/file.dat")
            }
            "empty" => text["orphans"] = json!([]),
            _ => (),
        }
        let text = match mode {
            "oversize" => " ".repeat(16_777_217),
            "duplicate-json" => format!("{{\"formatVersion\":1,{}", &original[1..]),
            _ => text.to_string(),
        };
        set(&world, JOURNAL, &text);
        assert!(resume(&mut world).is_err(), "{mode}");
        assert!(abort(&mut world).is_err(), "{mode}");
        assert!(image(&world).exists());
        assert!(quarantine(&world).is_empty());
        assert_eq!(world.metadata(JOURNAL).as_deref(), Some(text.as_str()));
    }
    set(&world, JOURNAL, &original);
    resume(&mut world).unwrap();
}
#[test]
fn sql_trigger_substitution_rolls_back_receipt_journal_inventory_and_scope_changes() {
    for key in [
        IMAGE_KEY,
        KEY,
        "package-install-inventory-v1",
        "package-trust-state-v1",
        RECEIPT,
        JOURNAL,
        "package-install-pending-v1",
    ] {
        let mut world = orphan_image();
        interrupt(&mut world, Phase::BeforeCommit);
        let original = world.metadata(key);
        let journal = world.metadata(JOURNAL);
        world.store.as_ref().unwrap().connection.execute_batch(&format!("CREATE TRIGGER substitute AFTER DELETE ON core_metadata WHEN OLD.key='{JOURNAL}' BEGIN INSERT INTO core_metadata(key,value) VALUES('{key}','CORRUPT') ON CONFLICT(key) DO UPDATE SET value='CORRUPT'; END;")).unwrap();
        assert!(resume(&mut world).is_err(), "{key}");
        assert_eq!(world.metadata(key), original);
        assert_eq!(world.metadata(JOURNAL), journal);
        assert_eq!(quarantine(&world).len(), 1);
        world
            .store
            .as_ref()
            .unwrap()
            .connection
            .execute_batch("DROP TRIGGER substitute;")
            .unwrap();
        resume(&mut world).unwrap();
        assert!(inspect(&mut world).unwrap().is_empty());
    }
}
#[test]
fn journal_insert_failure_precedes_native_moving_and_cannot_commit_substituted_inventory() {
    let mut world = orphan_image();
    let original = world.metadata(KEY);
    let reviewed = plan(&mut world).unwrap();
    world.store.as_ref().unwrap().connection.execute_batch(&format!("CREATE TRIGGER substitute_insert AFTER INSERT ON core_metadata WHEN NEW.key='{JOURNAL}' BEGIN UPDATE core_metadata SET value='CORRUPT' WHERE key='{KEY}'; END;")).unwrap();
    assert!(apply(&mut world, reviewed).is_err());
    assert_eq!(world.metadata(KEY), original);
    assert!(world.metadata(JOURNAL).is_none());
    assert!(image(&world).exists());
    assert!(quarantine(&world).is_empty());
}
#[test]
fn image_and_raw_quarantine_still_share_the_existing_group_and_byte_budgets() {
    for byte_budget in [false, true] {
        let mut world = orphan_image();
        let parent = world.root().join("quarantine");
        if byte_budget {
            for index in 0..9 {
                let group = parent.join(format!("{}-stage", uuid::Uuid::new_v4()));
                fs::create_dir(&group).unwrap();
                let file = fs::File::create(group.join("package.zip")).unwrap();
                file.set_len(if index == 8 { 1 } else { 33_554_432 })
                    .unwrap();
            }
        } else {
            for _ in 0..383 {
                fs::create_dir(parent.join(format!("{}-stage", uuid::Uuid::new_v4()))).unwrap();
            }
        }
        assert_eq!(
            plan(&mut world).err().map(|e| format!("{e:?}")),
            Some("Storage(BudgetExceeded)".into())
        );
        assert!(image(&world).exists());
        assert!(quarantine(&world).is_empty());
        assert!(world.metadata(JOURNAL).is_none());
    }
}
#[test]
fn native_junctions_never_turn_observation_into_outside_file_cleanup() {
    let mut world = orphan_image();
    let stage = partial_stage(&world);
    let outside = world.path.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("canary.txt"), b"KEEP_OUTSIDE").unwrap();
    let link = stage.join("junction");
    filesystem::junction(&link, &outside).unwrap();
    assert!(plan(&mut world).is_err());
    assert!(world.metadata(JOURNAL).is_none());
    assert!(quarantine(&world).is_empty());
    assert_eq!(
        fs::read(outside.join("canary.txt")).unwrap(),
        b"KEEP_OUTSIDE"
    );
    fs::remove_dir(&link).unwrap();
}
const PHASES: [Phase; 6] = [
    Phase::JournalCommitted,
    Phase::GroupMoved(0),
    Phase::BeforeNodeProtection(0, 1),
    Phase::GroupPrivatized(0),
    Phase::BeforeCommit,
    Phase::Committed,
];
#[test]
fn image_reconciliation_crash_child() {
    let Some(path) = std::env::var_os("FLOWTOOLS_IMAGE_RECONCILIATION_CHILD") else {
        return;
    };
    let index: usize = std::env::var("FLOWTOOLS_IMAGE_RECONCILIATION_BARRIER")
        .unwrap()
        .parse()
        .unwrap();
    assert!(index < PHASES.len());
    let mut world = World::open(Path::new(&path), false, false);
    let reviewed = plan(&mut world).unwrap();
    apply_fixture_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        reviewed,
        |phase, _| {
            if phase == PHASES[index] {
                std::process::exit(90 + index as i32);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("child missed barrier");
}
#[test]
fn real_process_exit_at_six_barriers_preserves_exact_batch_and_retries_privacy_before_commit() {
    for index in 0..PHASES.len() {
        let mut world = orphan_image();
        let original = world.fixture.contents["bin/ffmpeg.exe"].clone();
        drop(world.store.take());
        let status=Command::new(std::env::current_exe().unwrap()).args(["--exact","packages::tests::tool_tests::install_tests::image_tests::reconciliation_tests::image_reconciliation_crash_child","--nocapture"]).env("FLOWTOOLS_IMAGE_RECONCILIATION_CHILD",&world.path).env("FLOWTOOLS_IMAGE_RECONCILIATION_BARRIER",index.to_string()).status().unwrap();
        assert_eq!(status.code(), Some(90 + index as i32));
        world.reopen();
        if index == PHASES.len() - 1 {
            assert!(resume(&mut world).unwrap().is_none());
        } else {
            assert!(world.metadata(JOURNAL).is_some());
            assert_eq!(resume(&mut world).unwrap().unwrap().artifacts, 1);
        }
        assert!(world.metadata(JOURNAL).is_none());
        assert_eq!(quarantine(&world).len(), 1);
        assert_eq!(
            fs::read(quarantine(&world)[0].join("bin/ffmpeg.exe")).unwrap(),
            original
        );
        assert!(inspect(&mut world).unwrap().is_empty());
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
