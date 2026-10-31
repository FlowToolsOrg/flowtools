use super::*;
use crate::packages::{
    tools::install::images::{
        abort_inert_tool_materialization, inspect_inert_tool_images, materialize_fixture_with_hook,
    },
    tools::install::reconciliation::{
        abort_tool_archive_reconciliation, apply_fixture_with_hook,
        apply_tool_archive_reconciliation, plan_tool_archive_reconciliation,
        resume_tool_archive_reconciliation, ReconciliationPhase, ToolArchiveReconciliationPlan,
        ToolArchiveReconciliationSummary,
    },
    trust::{TrustRecoveryApproval, RECOVERY_PAYLOAD_TYPE},
};
const JOURNAL: &str = "tool-archive-reconciliation-pending-v1";
const RECEIPT: &str = "tool-archive-reconciliation-receipt-v1";
fn installed() -> World {
    let mut world = World::new();
    world.install().unwrap();
    world
}
fn plan(world: &mut World) -> Result<ToolArchiveReconciliationPlan, ToolInstallError> {
    plan_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn apply(
    world: &mut World,
    plan: ToolArchiveReconciliationPlan,
) -> Result<ToolArchiveReconciliationSummary, ToolInstallError> {
    apply_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap(), plan)
}
fn resume(world: &mut World) -> Result<Option<ToolArchiveReconciliationSummary>, ToolInstallError> {
    resume_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn abort(world: &mut World) -> Result<bool, ToolInstallError> {
    abort_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn orphan(world: &World, stage: bool, bytes: &[u8]) -> PathBuf {
    let path = world
        .root()
        .join(if stage { "staging" } else { "artifacts" })
        .join(if stage {
            uuid::Uuid::new_v4().to_string()
        } else {
            hash(bytes)
        });
    fs::create_dir(&path).unwrap();
    fs::write(path.join("package.zip"), bytes).unwrap();
    path
}
fn quarantine(world: &World) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(world.root().join("quarantine"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}
fn set(world: &World, key: &str, text: &str) {
    world.store.as_ref().unwrap().connection.execute(
        "INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [key,text]
    ).unwrap();
}
fn interrupt(world: &mut World, phase: ReconciliationPhase) {
    let reviewed = plan(world).unwrap();
    assert_eq!(
        apply_fixture_with_hook(
            &world.trust,
            world.store.as_mut().unwrap(),
            reviewed,
            |observed, _| {
                if phase == observed {
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
pub(super) fn authority(world: &mut World) {
    world.trust = PackageTrustStore::with_recovery_authority(
        BootstrapPins {
            keys: serde_json::from_value(world.fixture.base.golden["pins"].clone()).unwrap(),
            threshold: 2,
        },
        BootstrapPins {
            keys: vec![public(6), public(7)],
            threshold: 2,
        },
    )
    .unwrap();
}
pub(super) fn recover(world: &mut World, checkpoint: &[u8], now: u64) {
    let request = world
        .trust
        .begin_trust_recovery(world.store.as_mut().unwrap(), now)
        .unwrap();
    let root = world.trust_root_digest();
    let approval = signed(
        RECOVERY_PAYLOAD_TYPE,
        &TrustRecoveryApproval {
            format_version: 1,
            request_digest: request.request_digest().into(),
            checkpoint_sha256: hash(checkpoint),
            root_sha256: root,
            authority_sha256: request.authority_digest().into(),
            issued_at: now,
            expires_at: now + 60,
        },
        &[1, 2, 6, 7],
    );
    world
        .trust
        .recover_trust(world.store.as_mut().unwrap(), checkpoint, &approval, || now)
        .unwrap();
    world.fixture.base.policy.clock.now = now;
}
impl World {
    fn trust_root_digest(&self) -> String {
        let bytes = serde_json::to_vec(&self.fixture.base.golden["rootEnvelope"]).unwrap();
        hash(&decode_envelope(&bytes, ROOT_PAYLOAD_TYPE).unwrap().payload)
    }
}
#[test]
fn empty_plan_is_read_only_and_does_not_create_pools_journals_or_receipts() {
    let mut world = World::new();
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(
        reviewed.summary(),
        ToolArchiveReconciliationSummary {
            artifacts: 0,
            stages: 0,
            bytes: 0
        }
    );
    apply(&mut world, reviewed).unwrap();
    assert!(!world.root().exists());
    assert!(world.metadata(JOURNAL).is_none());
    assert!(world.metadata(RECEIPT).is_none());
    assert!(resume(&mut world).unwrap().is_none());
    assert!(!abort(&mut world).unwrap());
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn actual_backup_restore_preserves_floors_and_quarantines_newer_signed_zip_or_partial_stage() {
    for stage in [false, true] {
        let mut world = installed();
        authority(&mut world);
        let old_inventory = world.metadata(KEY);
        let original_blob = world.blob();
        let original_bytes = fs::read(original_blob.join("package.zip")).unwrap();
        let backup = world.store.as_ref().unwrap().backup().unwrap();
        world.fixture.base.package.version = "9.0.0".into();
        world.fixture.base.package.release_sequence += 1;
        world.fixture.manifest["version"] = json!("9.0.0");
        world.fixture.rebuild();
        if stage {
            assert!(world
                .with_hook(|phase, _| if phase == ToolInstallPhase::StagePrepared {
                    Err(InstallError::Storage.into())
                } else {
                    Ok(())
                })
                .is_err());
        } else {
            world.install().unwrap();
        }
        let now = world.fixture.now();
        let checkpoint = world
            .trust
            .export_recovery_checkpoint(world.store.as_mut().unwrap(), now + 1)
            .unwrap();
        drop(world.store.take());
        DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
        world.reopen();
        assert!(matches!(
            plan(&mut world),
            Err(ToolInstallError::Admission(ToolAdmissionError::Package(
                PackageError::TrustRecoveryRequired
            )))
        ));
        assert!(world.metadata(JOURNAL).is_none());
        recover(&mut world, &checkpoint, now + 2);
        let floors = world.metadata("package-trust-state-v1");
        let marker = world.metadata("package-trust-recovery-required");
        let reviewed = plan(&mut world).unwrap();
        assert_eq!(reviewed.summary().stages, usize::from(stage));
        assert_eq!(reviewed.summary().artifacts, usize::from(!stage));
        let result = apply(&mut world, reviewed).unwrap();
        assert_eq!(result.artifacts + result.stages, 1);
        assert_eq!(world.metadata(KEY), old_inventory);
        assert_eq!(world.metadata("package-trust-state-v1"), floors);
        assert_eq!(world.metadata("package-trust-recovery-required"), marker);
        assert_eq!(
            fs::read(original_blob.join("package.zip")).unwrap(),
            original_bytes
        );
        assert_eq!(
            fs::read(quarantine(&world)[0].join("package.zip")).unwrap(),
            world.fixture.base.archive
        );
        assert_eq!(world.pins().unwrap().len(), 1);
        // Quarantine did not install/adopt the new version; fresh signed bytes
        // are required to add it, and the old bytes cannot lower the floor.
        let mut old = ToolFixture::new(CompressionMethod::Stored);
        old.base.policy.clock.now = world.fixture.now();
        assert_eq!(
            old.admit(world.store.as_mut().unwrap(), &world.trust).err(),
            Some(ToolAdmissionError::Package(PackageError::ReleaseRollback))
        );
        world.install().unwrap();
        assert_eq!(world.pins().unwrap().len(), 2);
        assert_eq!(quarantine(&world).len(), 1);
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
#[test]
fn complete_batch_is_checked_before_any_move_and_changed_or_extra_bytes_are_preserved() {
    for mode in ["bytes", "extra", "known", "new-group"] {
        let mut world = installed();
        let first = orphan(&world, false, b"FIRST");
        let second = orphan(&world, true, b"SECOND");
        let reviewed = plan(&mut world).unwrap();
        match mode {
            "bytes" => fs::write(second.join("package.zip"), b"CHANGED").unwrap(),
            "extra" => fs::write(second.join("unexpected.exe"), b"EXTRA").unwrap(),
            "known" => {
                let p = world.blob().join("package.zip");
                writable(&p);
                fs::write(p, b"CHANGED").unwrap();
            }
            "new-group" => {
                orphan(&world, true, b"NEW");
            }
            _ => unreachable!(),
        }
        assert!(apply(&mut world, reviewed).is_err(), "{mode}");
        assert!(first.exists());
        assert!(second.exists());
        assert!(quarantine(&world).is_empty());
        assert!(world.metadata(JOURNAL).is_none());
    }
}
#[test]
fn cross_profile_and_changed_companion_inventory_or_trust_scope_refuse_the_plan() {
    let mut a = installed();
    let mut b = installed();
    let source = orphan(&a, true, b"STAGE");
    let reviewed = plan(&mut a).unwrap();
    assert!(apply(&mut b, reviewed).is_err());
    for key in [
        "package-install-inventory-v1",
        "package-trust-fixture-scope",
    ] {
        let reviewed = plan(&mut a).unwrap();
        set(&a, key, "{}");
        assert!(apply(&mut a, reviewed).is_err());
        assert!(source.exists());
        assert!(quarantine(&a).is_empty());
        a.store
            .as_ref()
            .unwrap()
            .connection
            .execute("DELETE FROM core_metadata WHERE key=?1", [key])
            .unwrap();
    }
}
#[test]
fn pending_batch_blocks_plugin_raw_tool_image_and_other_reconciliation_surfaces() {
    let mut world = installed();
    orphan(&world, true, b"STAGE");
    interrupt(&mut world, ReconciliationPhase::JournalCommitted);
    let expected = Some(ToolInstallError::Storage(InstallError::RecoveryPending));
    assert_eq!(world.install().err(), expected);
    assert_eq!(world.pins().err(), expected);
    assert_eq!(world.abort().err(), expected);
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
    assert_eq!(
        crate::packages::install::plan_inert_reconciliation(
            &world.trust,
            world.store.as_mut().unwrap()
        )
        .err()
        .map(|e| format!("{e:?}")),
        Some("RecoveryPending".into())
    );
    assert_eq!(
        inspect_inert_tool_images(&world.trust, world.store.as_mut().unwrap()).err(),
        expected
    );
    assert_eq!(
        abort_inert_tool_materialization(&world.trust, world.store.as_mut().unwrap()).err(),
        expected
    );
    assert_eq!(
        plan(&mut world).err().map(|e| format!("{e:?}")),
        Some("Storage(RecoveryPending)".into())
    );
    assert_eq!(resume(&mut world).unwrap().unwrap().stages, 1);
    assert!(world.metadata(JOURNAL).is_none());
    world.pins().unwrap();
}
#[test]
fn every_other_pending_journal_blocks_planning_and_is_never_repaired() {
    for key in [
        PENDING,
        "package-install-pending-v1",
        "package-reconciliation-pending-v1",
        "tool-image-materialization-pending-v1",
    ] {
        let mut world = installed();
        let source = orphan(&world, true, b"STAGE");
        set(&world, key, "CORRUPT");
        assert!(plan(&mut world).is_err());
        assert!(resume(&mut world).is_err());
        assert!(abort(&mut world).is_err());
        assert_eq!(world.metadata(key).as_deref(), Some("CORRUPT"));
        assert!(source.exists());
        assert!(quarantine(&world).is_empty());
    }
}
#[test]
fn cancellation_keeps_mixed_locations_and_requires_a_new_plan_for_remaining_sources() {
    let mut world = installed();
    orphan(&world, false, b"ARTIFACT");
    let remaining = orphan(&world, true, b"STAGE");
    interrupt(&mut world, ReconciliationPhase::GroupQuarantined(0));
    let inventory = world.metadata(KEY);
    assert!(abort(&mut world).unwrap());
    assert!(remaining.exists());
    assert_eq!(quarantine(&world).len(), 1);
    assert_eq!(world.metadata(KEY), inventory);
    assert_eq!(
        serde_json::from_str::<Value>(&world.metadata(RECEIPT).unwrap()).unwrap()["outcome"],
        "aborted"
    );
    assert!(world.pins().is_err());
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(reviewed.summary().stages, 1);
    apply(&mut world, reviewed).unwrap();
    assert_eq!(quarantine(&world).len(), 2);
    world.pins().unwrap();
}
#[test]
fn changed_scope_refuses_resume_but_explicit_abort_keeps_current_trust_and_bytes() {
    let mut world = installed();
    let source = orphan(&world, true, b"STAGE");
    interrupt(&mut world, ReconciliationPhase::JournalCommitted);
    set(&world, "package-trust-fixture-scope", "1");
    assert!(resume(&mut world).is_err());
    assert!(source.exists());
    assert!(abort(&mut world).unwrap());
    assert!(source.exists());
    assert!(quarantine(&world).is_empty());
    assert_eq!(
        world.metadata("package-trust-fixture-scope").as_deref(),
        Some("1")
    );
    let reviewed = plan(&mut world).unwrap();
    apply(&mut world, reviewed).unwrap();
}
#[test]
fn signed_root_revocation_between_groups_stops_the_next_effect_without_lowering_trust() {
    let mut world = installed();
    orphan(&world, false, b"ARTIFACT");
    let remaining = orphan(&world, true, b"STAGE");
    let bytes = serde_json::to_vec(&world.fixture.base.golden["rootEnvelope"]).unwrap();
    let mut root: RootPayload =
        serde_json::from_slice(&decode_envelope(&bytes, ROOT_PAYLOAD_TYPE).unwrap().payload)
            .unwrap();
    root.version += 1;
    root.previous_root_sha256 = Some(world.trust_root_digest());
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
            if phase == ReconciliationPhase::GroupQuarantined(0) {
                trust
                    .rotate(store, &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]), now)
                    .unwrap();
            }
            Ok(())
        }
    )
    .is_err());
    assert!(remaining.exists());
    assert_eq!(quarantine(&world).len(), 1);
    let current = world.metadata("package-trust-state-v1");
    assert!(world.metadata(JOURNAL).is_some());
    assert!(resume(&mut world).is_err());
    assert!(abort(&mut world).unwrap());
    assert_eq!(world.metadata("package-trust-state-v1"), current);
    assert!(remaining.exists());
}
#[test]
fn forged_corrupt_or_oversize_journal_never_targets_installed_or_outside_bytes() {
    let mut world = installed();
    let source = orphan(&world, true, b"STAGE");
    interrupt(&mut world, ReconciliationPhase::JournalCommitted);
    let original = world.metadata(JOURNAL).unwrap();
    for mode in [
        "known",
        "escape",
        "duplicate",
        "unknown",
        "duplicate-json",
        "oversize",
        "empty",
    ] {
        let mut altered: Value = serde_json::from_str(&original).unwrap();
        match mode {
            "known" => {
                altered["orphans"][0]["kind"] = json!("artifact");
                altered["orphans"][0]["source"] = json!(world.fixture.base.package.archive.sha256);
            }
            "escape" => altered["orphans"][0]["source"] = json!("../outside"),
            "duplicate" => {
                let o = altered["orphans"][0].clone();
                altered["orphans"].as_array_mut().unwrap().push(o);
            }
            "unknown" => altered["approved"] = json!(true),
            "empty" => altered["orphans"] = json!([]),
            _ => (),
        }
        let text = match mode {
            "duplicate-json" => format!("{{\"formatVersion\":1,{}", &original[1..]),
            "oversize" => " ".repeat(16_777_217),
            _ => altered.to_string(),
        };
        set(&world, JOURNAL, &text);
        assert!(resume(&mut world).is_err(), "{mode}");
        assert!(abort(&mut world).is_err(), "{mode}");
        assert!(source.exists());
        assert!(world.blob().exists());
        assert!(quarantine(&world).is_empty());
    }
    set(&world, JOURNAL, &original);
    resume(&mut world).unwrap();
}
#[test]
fn sql_triggers_cannot_commit_inventory_trust_companion_receipt_or_journal_substitution() {
    for key in [
        KEY,
        "package-trust-state-v1",
        "tool-image-inventory-v1",
        "package-install-inventory-v1",
        RECEIPT,
        JOURNAL,
        "package-install-pending-v1",
    ] {
        let mut world = installed();
        orphan(&world, true, b"STAGE");
        interrupt(&mut world, ReconciliationPhase::BeforeCommit);
        let original = world.metadata(key);
        let journal = world.metadata(JOURNAL);
        world.store.as_ref().unwrap().connection.execute_batch(&format!(
            "CREATE TRIGGER substitute AFTER DELETE ON core_metadata WHEN OLD.key='{JOURNAL}' BEGIN INSERT INTO core_metadata(key,value) VALUES('{key}','CORRUPT') ON CONFLICT(key) DO UPDATE SET value='CORRUPT'; END;"
        )).unwrap();
        assert!(resume(&mut world).is_err(), "{key}");
        assert_eq!(world.metadata(key), original);
        assert_eq!(world.metadata(JOURNAL), journal);
        assert_eq!(quarantine(&world).len(), 1);
        world
            .store
            .as_ref()
            .unwrap()
            .connection
            .execute_batch("DROP TRIGGER substitute")
            .unwrap();
        resume(&mut world).unwrap();
        world.pins().unwrap();
    }
}
#[test]
fn journal_insert_failure_precedes_every_move_and_abort_receipt_failure_keeps_exact_batch() {
    let mut world = installed();
    let source = orphan(&world, true, b"STAGE");
    let reviewed = plan(&mut world).unwrap();
    let old_inventory = world.metadata(KEY);
    world.store.as_ref().unwrap().connection.execute_batch(&format!(
        "CREATE TRIGGER substitute_insert AFTER INSERT ON core_metadata WHEN NEW.key='{JOURNAL}' BEGIN UPDATE core_metadata SET value='CORRUPT' WHERE key='{KEY}'; END;"
    )).unwrap();
    assert!(apply(&mut world, reviewed).is_err());
    assert_eq!(world.metadata(KEY), old_inventory);
    assert!(world.metadata(JOURNAL).is_none());
    assert!(source.exists());
    assert!(quarantine(&world).is_empty());
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute_batch("DROP TRIGGER substitute_insert")
        .unwrap();
    orphan(&world, false, b"ARTIFACT");
    interrupt(&mut world, ReconciliationPhase::GroupQuarantined(0));
    let original = world.metadata(JOURNAL);
    world.store.as_ref().unwrap().connection.execute_batch(&format!(
        "CREATE TRIGGER fail_receipt BEFORE INSERT ON core_metadata WHEN NEW.key='{RECEIPT}' BEGIN SELECT RAISE(ABORT,'fixture'); END;"
    )).unwrap();
    assert!(abort(&mut world).is_err());
    assert_eq!(world.metadata(JOURNAL), original);
    assert!(source.exists());
    assert_eq!(quarantine(&world).len(), 1);
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute_batch("DROP TRIGGER fail_receipt")
        .unwrap();
    assert!(abort(&mut world).unwrap());
    assert!(source.exists());
    assert_eq!(quarantine(&world).len(), 1);
}
#[test]
fn corrupt_source_inventory_or_missing_pool_never_cleans_or_bootstraps() {
    for missing in [false, true] {
        let mut world = installed();
        let source = orphan(&world, true, b"STAGE");
        if missing {
            fs::rename(world.root(), world.path.join("preserved-pool")).unwrap();
        } else {
            set(&world, KEY, "{}");
        }
        assert!(plan(&mut world).is_err());
        assert!(world.metadata(JOURNAL).is_none());
        if !missing {
            assert!(source.exists());
        }
    }
}
#[test]
fn shared_three_pool_group_budget_including_quarantine_is_preserved() {
    let mut world = installed();
    let source = orphan(&world, true, b"STAGE");
    let pool = world.path.join("package-store-v1");
    fs::create_dir(&pool).unwrap();
    for name in ["artifacts", "staging", "quarantine"] {
        fs::create_dir(pool.join(name)).unwrap();
    }
    // Two existing raw groups plus 383 plugin quarantine groups exceed 384.
    for _ in 0..383 {
        fs::create_dir(
            pool.join("quarantine")
                .join(format!("{}-stage", uuid::Uuid::new_v4())),
        )
        .unwrap();
    }
    assert_eq!(
        plan(&mut world).err().map(|e| format!("{e:?}")),
        Some("Storage(BudgetExceeded)".into())
    );
    assert!(source.exists());
    assert!(world.metadata(JOURNAL).is_none());
}
#[test]
fn junction_hardlink_and_unknown_group_names_cannot_redirect_quarantine() {
    for mode in ["junction", "hardlink", "name"] {
        let mut world = installed();
        let outside = world.path.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("package.zip"), b"OUTSIDE").unwrap();
        let group = world.root().join("staging").join(if mode == "name" {
            "unknown-group".into()
        } else {
            uuid::Uuid::new_v4().to_string()
        });
        if mode == "junction" {
            assert!(Command::new("cmd")
                .args(["/c", "mklink", "/J"])
                .arg(&group)
                .arg(&outside)
                .status()
                .unwrap()
                .success());
        } else {
            fs::create_dir(&group).unwrap();
            if mode == "hardlink" {
                fs::hard_link(outside.join("package.zip"), group.join("package.zip")).unwrap();
            } else {
                fs::write(group.join("package.zip"), b"UNKNOWN").unwrap();
            }
        }
        assert!(plan(&mut world).is_err(), "{mode}");
        assert_eq!(fs::read(outside.join("package.zip")).unwrap(), b"OUTSIDE");
        assert!(group.exists());
        assert!(quarantine(&world).is_empty());
        assert!(world.metadata(JOURNAL).is_none());
    }
}
#[test]
fn materialized_image_references_are_retained_and_unrecognized_images_refuse_raw_cleanup() {
    let mut world = installed();
    let installed = world.install().unwrap();
    drop(
        materialize_fixture_with_hook(
            &world.trust,
            world.store.as_mut().unwrap(),
            &installed,
            || world.fixture.now(),
            |_, _| Ok(()),
        )
        .unwrap(),
    );
    let image_metadata = world.metadata("tool-image-inventory-v1");
    orphan(&world, true, b"STAGE");
    let reviewed = plan(&mut world).unwrap();
    apply(&mut world, reviewed).unwrap();
    assert_eq!(world.metadata("tool-image-inventory-v1"), image_metadata);
    assert_eq!(
        inspect_inert_tool_images(&world.trust, world.store.as_mut().unwrap())
            .unwrap()
            .len(),
        1
    );
    let unrecognized = world
        .path
        .join("tool-image-store-v1/staging")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&unrecognized).unwrap();
    let source = orphan(&world, true, b"NEXT");
    assert!(plan(&mut world).is_err());
    assert!(source.exists());
    assert!(unrecognized.exists());
}
const PHASES: [ReconciliationPhase; 5] = [
    ReconciliationPhase::JournalCommitted,
    ReconciliationPhase::GroupQuarantined(0),
    ReconciliationPhase::GroupQuarantined(1),
    ReconciliationPhase::BeforeCommit,
    ReconciliationPhase::Committed,
];
#[test]
fn reconciliation_process_crash_child() {
    let Some(path) = std::env::var_os("FLOWTOOLS_TOOL_RECONCILIATION_CHILD_ROOT") else {
        return;
    };
    let index: usize = std::env::var("FLOWTOOLS_TOOL_RECONCILIATION_BARRIER")
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
    panic!("child missed actual barrier");
}
#[test]
fn actual_process_exits_at_all_five_barriers_resume_only_exact_pending_batches() {
    for index in 0..PHASES.len() {
        let mut world = installed();
        let inventory = world.metadata(KEY);
        orphan(&world, false, b"ARTIFACT");
        orphan(&world, true, b"STAGE");
        drop(world.store.take());
        let status=Command::new(std::env::current_exe().unwrap())
            .args(["--exact","packages::tests::tool_tests::install_tests::reconciliation_tests::reconciliation_process_crash_child","--nocapture"])
            .env("FLOWTOOLS_TOOL_RECONCILIATION_CHILD_ROOT",&world.path)
            .env("FLOWTOOLS_TOOL_RECONCILIATION_BARRIER",index.to_string()).status().unwrap();
        assert_eq!(status.code(), Some(90 + index as i32));
        world.reopen();
        if index == 4 {
            assert!(world.metadata(JOURNAL).is_none());
            assert!(resume(&mut world).unwrap().is_none());
        } else {
            assert!(world.pins().is_err());
            assert_eq!(resume(&mut world).unwrap().unwrap().stages, 1);
        }
        assert_eq!(quarantine(&world).len(), 2);
        assert_eq!(world.metadata(KEY), inventory);
        world.pins().unwrap();
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
