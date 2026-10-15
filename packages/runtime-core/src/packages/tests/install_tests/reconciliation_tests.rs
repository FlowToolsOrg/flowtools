use super::*;
use crate::packages::{
    install::{
        abort_inert_reconciliation, apply_inert_reconciliation, plan_inert_reconciliation,
        reconcile_with_hook, resume_inert_reconciliation, InertReconciliationPlan,
        ReconciliationPhase,
    },
    trust::{TrustRecoveryApproval, RECOVERY_PAYLOAD_TYPE},
};
use std::collections::BTreeSet;

const JOURNAL: &str = "package-reconciliation-pending-v1";
const RECEIPT: &str = "package-reconciliation-receipt-v1";
fn installed() -> World {
    let mut world = World::new();
    let input = world.input(manifest("consumer", vec![]), 1);
    world.install(&input).unwrap();
    world
}
fn plan(world: &mut World) -> Result<InertReconciliationPlan, InstallError> {
    plan_inert_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn apply(
    world: &mut World,
    plan: InertReconciliationPlan,
) -> Result<crate::packages::install::InertReconciliationSummary, InstallError> {
    apply_inert_reconciliation(&world.trust, world.store.as_mut().unwrap(), plan)
}
fn resume(
    world: &mut World,
) -> Result<Option<crate::packages::install::InertReconciliationSummary>, InstallError> {
    resume_inert_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn abort(world: &mut World) -> Result<bool, InstallError> {
    abort_inert_reconciliation(&world.trust, world.store.as_mut().unwrap())
}
fn orphan(world: &World, stage: bool, bytes: &[u8]) -> PathBuf {
    let path = world
        .path
        .join("package-store-v1")
        .join(if stage { "staging" } else { "artifacts" })
        .join(if stage {
            uuid::Uuid::new_v4().to_string()
        } else {
            hash(bytes)
        });
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("package.zip"), bytes).unwrap();
    path
}
fn quarantine(world: &World) -> Vec<PathBuf> {
    let mut paths = std::fs::read_dir(world.path.join("package-store-v1/quarantine"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}
fn recovery_authority(world: &mut World) {
    world.trust = PackageTrustStore::with_recovery_authority(
        BootstrapPins {
            keys: serde_json::from_value(world.fixture.golden["pins"].clone()).unwrap(),
            threshold: 2,
        },
        BootstrapPins {
            keys: vec![public(6), public(7)],
            threshold: 2,
        },
    )
    .unwrap();
}
fn checkpoint_recovery(world: &mut World, checkpoint: &[u8], now: u64) {
    let request = world
        .trust
        .begin_trust_recovery(world.store.as_mut().unwrap(), now)
        .unwrap();
    let approval = signed(
        RECOVERY_PAYLOAD_TYPE,
        &TrustRecoveryApproval {
            format_version: 1,
            request_digest: request.request_digest().into(),
            checkpoint_sha256: hash(checkpoint),
            root_sha256: world.fixture.root.payload_digest().into(),
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
    world.fixture.policy.clock.now = now;
}

#[test]
fn actual_backup_restore_quarantines_newer_signed_zip_without_adopting_or_lowering_floors() {
    for interrupted in [false, true] {
        let mut world = installed();
        recovery_authority(&mut world);
        let before = world.raw(INVENTORY);
        let input = world.input(manifest("consumer", vec![]), 1);
        let original = std::fs::read(world.artifact(&input).join("package.zip")).unwrap();
        let backup = world.store.as_ref().unwrap().backup().unwrap();
        let candidate = world.input(revision(), 2);
        let now = world.fixture.policy.clock.now;
        if interrupted {
            assert_eq!(
                install_with_hook(
                    &world.trust,
                    world.store.as_mut().unwrap(),
                    candidate.borrow(),
                    &world.compatibility,
                    || now,
                    |phase, _| if phase == InstallPhase::StagePrepared {
                        Err(InstallError::Storage)
                    } else {
                        Ok(())
                    }
                )
                .err(),
                Some(InstallError::Storage)
            );
        } else {
            world.install(&candidate).unwrap();
        }
        let checkpoint = world
            .trust
            .export_recovery_checkpoint(world.store.as_mut().unwrap(), now + 1)
            .unwrap();
        world.store.take();
        DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
        world.reopen();
        assert_eq!(
            plan(&mut world).err().map(|e| format!("{e:?}")),
            Some("Package(TrustRecoveryRequired)".into())
        );
        assert!(world.raw(JOURNAL).is_none());
        checkpoint_recovery(&mut world, &checkpoint, now + 2);
        assert_eq!(world.inspect().err(), Some(InstallError::StateInvalid));
        let floors = world.raw("package-trust-state-v1");
        let marker = world.raw("package-trust-recovery-required");
        let reviewed = plan(&mut world).unwrap();
        assert_eq!(reviewed.summary().artifacts, usize::from(!interrupted));
        assert_eq!(reviewed.summary().stages, usize::from(interrupted));
        apply(&mut world, reviewed).unwrap();
        assert_eq!(world.raw(INVENTORY), before);
        assert_eq!(world.raw("package-trust-state-v1"), floors);
        assert_eq!(world.raw("package-trust-recovery-required"), marker);
        assert_eq!(world.inspect().unwrap().packages().len(), 1);
        assert_eq!(quarantine(&world).len(), 1);
        assert_eq!(
            std::fs::read(quarantine(&world)[0].join("package.zip")).unwrap(),
            candidate.archive
        );
        assert_eq!(
            std::fs::read(world.artifact(&input).join("package.zip")).unwrap(),
            original
        );
        assert_eq!(
            world.install(&input).err(),
            Some(InstallError::Package(PackageError::ReleaseRollback))
        );
        world.install(&candidate).unwrap();
        assert_eq!(world.inspect().unwrap().packages().len(), 2);
        assert_eq!(quarantine(&world).len(), 1);
    }
}

#[test]
fn restored_multiversion_inventory_keeps_original_signed_transitive_locks() {
    let mut world = World::new();
    recovery_authority(&mut world);
    let provider = world.input(manifest("provider", vec![]), 1);
    let middle = world.input(manifest("middle", vec![dependency("provider")]), 1);
    let consumer = world.input(manifest("consumer", vec![dependency("middle")]), 1);
    for input in [&provider, &middle, &consumer] {
        world.install(input).unwrap();
    }
    let inventory = world.raw(INVENTORY).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&inventory).unwrap()["formatVersion"],
        2
    );
    let locks = serde_json::to_value(world.inspect().unwrap().locks()).unwrap();
    let backup = world.store.as_ref().unwrap().backup().unwrap();
    let mut value = manifest("provider", vec![]);
    value["version"] = json!("1.1.0");
    let updated = world.input(value, 2);
    world.install(&updated).unwrap();
    let now = world.fixture.policy.clock.now;
    let checkpoint = world
        .trust
        .export_recovery_checkpoint(world.store.as_mut().unwrap(), now + 1)
        .unwrap();
    world.store.take();
    DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
    world.reopen();
    checkpoint_recovery(&mut world, &checkpoint, now + 2);
    let trust = world.raw("package-trust-state-v1");
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(reviewed.summary().artifacts, 1);
    apply(&mut world, reviewed).unwrap();
    assert_eq!(world.raw(INVENTORY).unwrap(), inventory);
    assert_eq!(world.raw("package-trust-state-v1"), trust);
    let snapshot = world.inspect().unwrap();
    assert_eq!(snapshot.packages().len(), 3);
    assert_eq!(serde_json::to_value(snapshot.locks()).unwrap(), locks);
    for input in [&provider, &middle, &consumer] {
        assert_eq!(
            std::fs::read(world.artifact(input).join("package.zip")).unwrap(),
            input.archive
        );
    }
    assert_eq!(
        std::fs::read(quarantine(&world)[0].join("package.zip")).unwrap(),
        updated.archive
    );
    // The updated ZIP needs fresh admission; quarantine never creates a lock.
    world.install(&updated).unwrap();
    let snapshot = world.inspect().unwrap();
    assert_eq!(snapshot.packages().len(), 4);
    for original in locks.as_array().unwrap() {
        assert!(serde_json::to_value(snapshot.locks())
            .unwrap()
            .as_array()
            .unwrap()
            .contains(original));
    }
    assert_eq!(quarantine(&world).len(), 1);
    for table in ["grants", "jobs", "plugin_data", "imports"] {
        let count: u32 = world
            .store
            .as_ref()
            .unwrap()
            .connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "{table}");
    }
}

#[test]
fn legacy_inventory_refuses_every_reconciliation_entry_without_rewriting_bytes() {
    let mut world = installed();
    let source = orphan(&world, true, b"INERT-STAGE");
    let reviewed = plan(&mut world).unwrap();
    let mut legacy: Value = serde_json::from_str(&world.raw(INVENTORY).unwrap()).unwrap();
    legacy["formatVersion"] = json!(1);
    let raw = serde_json::to_string(&legacy).unwrap();
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute(
            "UPDATE core_metadata SET value=?1 WHERE key=?2",
            rusqlite::params![raw, INVENTORY],
        )
        .unwrap();
    let trust = world.raw("package-trust-state-v1");
    assert_eq!(
        apply(&mut world, reviewed).err(),
        Some(InstallError::StateInvalid)
    );
    assert_eq!(plan(&mut world).err(), Some(InstallError::StateInvalid));
    assert_eq!(resume(&mut world).err(), Some(InstallError::StateInvalid));
    assert_eq!(abort(&mut world).err(), Some(InstallError::StateInvalid));
    assert_eq!(world.raw(INVENTORY).unwrap(), raw);
    assert_eq!(world.raw("package-trust-state-v1"), trust);
    assert!(world.raw(JOURNAL).is_none());
    assert!(world.raw(RECEIPT).is_none());
    assert_eq!(
        std::fs::read(source.join("package.zip")).unwrap(),
        b"INERT-STAGE"
    );
    assert!(quarantine(&world).is_empty());
}

#[test]
fn read_only_plan_and_explicit_apply_preserve_bytes_inventory_and_idempotence() {
    let mut world = installed();
    let a = orphan(&world, false, b"UNTRUSTED-INERT-ARCHIVE");
    let b = orphan(&world, true, b"PARTIAL-STAGE");
    let before = world.raw(INVENTORY);
    let trust = world.raw("package-trust-state-v1");
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(reviewed.summary().artifacts, 1);
    assert_eq!(reviewed.summary().stages, 1);
    assert_eq!(reviewed.summary().bytes, 36);
    assert!(a.exists() && b.exists());
    assert!(world.raw(JOURNAL).is_none());
    assert!(world.raw(RECEIPT).is_none());
    let result = apply(&mut world, reviewed).unwrap();
    assert_eq!(result.artifacts, 1);
    assert!(!a.exists() && !b.exists());
    let contents = quarantine(&world)
        .iter()
        .map(|p| std::fs::read(p.join("package.zip")).unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contents,
        BTreeSet::from([
            b"UNTRUSTED-INERT-ARCHIVE".to_vec(),
            b"PARTIAL-STAGE".to_vec()
        ])
    );
    assert_eq!(world.raw(INVENTORY), before);
    assert_eq!(world.raw("package-trust-state-v1"), trust);
    assert_eq!(world.inspect().unwrap().packages().len(), 1);
    assert!(world.raw(JOURNAL).is_none());
    let receipt: Value = serde_json::from_str(&world.raw(RECEIPT).unwrap()).unwrap();
    assert_eq!(receipt["outcome"], "completed");
    assert!(!world
        .raw(RECEIPT)
        .unwrap()
        .contains(&world.path.to_string_lossy().to_string()));
    let empty = plan(&mut world).unwrap();
    assert_eq!(empty.summary().bytes, 0);
    apply(&mut world, empty).unwrap();
    assert!(resume(&mut world).unwrap().is_none());
    assert!(!abort(&mut world).unwrap());
}

#[test]
fn changed_bytes_and_unknown_file_names_fail_whole_preflight_before_any_move() {
    for mode in ["bytes", "extra", "directory", "hardlink", "name", "known"] {
        let mut world = installed();
        let a = orphan(&world, false, b"A");
        let b = orphan(&world, true, b"B");
        let reviewed = plan(&mut world).unwrap();
        match mode {
            "bytes" => std::fs::write(b.join("package.zip"), b"CHANGED").unwrap(),
            "extra" => std::fs::write(b.join("unexpected.exe"), b"INERT").unwrap(),
            "directory" => {
                std::fs::remove_file(b.join("package.zip")).unwrap();
                std::fs::create_dir(b.join("package.zip")).unwrap();
            }
            "hardlink" => {
                let original = world.path.join("outside-canary");
                std::fs::write(&original, b"OUTSIDE").unwrap();
                std::fs::remove_file(b.join("package.zip")).unwrap();
                std::fs::hard_link(&original, b.join("package.zip")).unwrap();
            }
            "name" => {
                std::fs::rename(&b, b.with_file_name("not-a-uuid")).unwrap();
            }
            "known" => {
                let input = world.input(manifest("consumer", vec![]), 1);
                change(&world.artifact(&input).join("package.zip"), b"BROKEN");
            }
            _ => unreachable!(),
        }
        assert!(apply(&mut world, reviewed).is_err(), "{mode}");
        assert!(a.exists());
        assert!(quarantine(&world).is_empty());
        assert!(world.raw(JOURNAL).is_none());
        if mode != "bytes" {
            assert!(plan(&mut world).is_err(), "{mode}");
        }
    }
}

#[test]
fn plan_cannot_cross_profile_or_inventory_revision_or_interleave_installation() {
    let mut a = installed();
    let mut b = installed();
    let source = orphan(&a, true, b"STAGE");
    let reviewed = plan(&mut a).unwrap();
    assert_eq!(
        apply(&mut b, reviewed).err(),
        Some(InstallError::InventoryChanged)
    );
    assert!(source.exists());
    let reviewed = plan(&mut a).unwrap();
    // Clear the unreferenced group before an ordinary new installation.
    let current = plan(&mut a).unwrap();
    apply(&mut a, current).unwrap();
    let candidate = a.input(revision(), 2);
    a.install(&candidate).unwrap();
    assert_eq!(
        apply(&mut a, reviewed).err(),
        Some(InstallError::InventoryChanged)
    );
    let now = a.fixture.policy.clock.now;
    let mut v = manifest("provider", vec![]);
    v["version"] = json!("1.1.0");
    let input = a.input(v, 2);
    install_with_hook(
        &a.trust,
        a.store.as_mut().unwrap(),
        input.borrow(),
        &a.compatibility,
        || now,
        |phase, _| {
            if phase == InstallPhase::JournalCommitted {
                Err(InstallError::Storage)
            } else {
                Ok(())
            }
        },
    )
    .err()
    .unwrap();
    assert_eq!(
        plan(&mut a).err().map(|e| format!("{e:?}")),
        Some("RecoveryPending".into())
    );
    assert_eq!(resume(&mut a).err(), Some(InstallError::RecoveryPending));
    assert_eq!(abort(&mut a).err(), Some(InstallError::RecoveryPending));
    a.abort().unwrap();
}

#[test]
fn interrupted_batch_blocks_ordinary_calls_and_resumes_after_partial_quarantine() {
    let mut world = installed();
    orphan(&world, false, b"A");
    orphan(&world, true, b"B");
    let before = world.raw(INVENTORY);
    let reviewed = plan(&mut world).unwrap();
    reconcile_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        reviewed,
        |phase, _| {
            if phase == ReconciliationPhase::GroupQuarantined(0) {
                Err(InstallError::Storage)
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(quarantine(&world).len(), 1);
    assert_eq!(world.inspect().err(), Some(InstallError::RecoveryPending));
    let input = world.input(revision(), 2);
    assert_eq!(
        world.install(&input).err(),
        Some(InstallError::RecoveryPending)
    );
    assert_eq!(world.abort().err(), Some(InstallError::RecoveryPending));
    assert!(plan(&mut world).is_err());
    world.reopen();
    let result = resume(&mut world).unwrap().unwrap();
    assert_eq!(result.artifacts + result.stages, 2);
    assert_eq!(quarantine(&world).len(), 2);
    assert_eq!(world.raw(INVENTORY), before);
    assert!(world.raw(JOURNAL).is_none());
    assert!(resume(&mut world).unwrap().is_none());
    world.inspect().unwrap();
}

#[test]
fn receipt_write_or_journal_delete_failure_rolls_back_metadata_after_safe_moves() {
    for trigger in [
        "CREATE TRIGGER fail BEFORE INSERT ON core_metadata WHEN NEW.key='package-reconciliation-receipt-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END;",
        "CREATE TRIGGER fail BEFORE DELETE ON core_metadata WHEN OLD.key='package-reconciliation-pending-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END;",
    ] {
        let mut world = installed(); orphan(&world,true,b"STAGE");
        let before = world.raw(INVENTORY);
        world.store.as_ref().unwrap().connection.execute_batch(trigger).unwrap();
        let reviewed = plan(&mut world).unwrap();
        assert_eq!(apply(&mut world,reviewed).err(),Some(InstallError::Storage));
        assert_eq!(quarantine(&world).len(),1);
        assert_eq!(world.raw(INVENTORY),before);
        assert!(world.raw(RECEIPT).is_none());
        assert!(world.raw(JOURNAL).is_some());
        world.store.as_ref().unwrap().connection.execute_batch("DROP TRIGGER fail").unwrap();
        world.reopen(); resume(&mut world).unwrap().unwrap();
        assert_eq!(quarantine(&world).len(),1);
        assert_eq!(world.raw(INVENTORY),before);
        world.inspect().unwrap();
    }
}

#[test]
fn explicit_abort_retains_moved_and_unmoved_blobs_and_requires_a_new_plan() {
    let mut world = installed();
    orphan(&world, false, b"A");
    let remaining = orphan(&world, true, b"B");
    let reviewed = plan(&mut world).unwrap();
    reconcile_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        reviewed,
        |phase, _| {
            if phase == ReconciliationPhase::GroupQuarantined(0) {
                Err(InstallError::Storage)
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert!(abort(&mut world).unwrap());
    assert!(remaining.exists());
    assert_eq!(quarantine(&world).len(), 1);
    assert!(world.raw(JOURNAL).is_none());
    let receipt: Value = serde_json::from_str(&world.raw(RECEIPT).unwrap()).unwrap();
    assert_eq!(receipt["outcome"], "aborted");
    assert_eq!(world.inspect().err(), Some(InstallError::StateInvalid));
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(reviewed.summary().stages, 1);
    apply(&mut world, reviewed).unwrap();
    assert_eq!(quarantine(&world).len(), 2);
    world.inspect().unwrap();
}

#[test]
fn forged_journal_cannot_target_a_referenced_artifact_or_other_profile() {
    let mut a = installed();
    let mut b = installed();
    let source = orphan(&a, true, b"STAGE");
    let reviewed = plan(&mut a).unwrap();
    reconcile_with_hook(&a.trust, a.store.as_mut().unwrap(), reviewed, |phase, _| {
        if phase == ReconciliationPhase::JournalCommitted {
            Err(InstallError::Storage)
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    let original = a.raw(JOURNAL).unwrap();
    b.store.take();
    let clone = b.path.join("clone.sqlite");
    a.store
        .as_ref()
        .unwrap()
        .connection
        .backup(rusqlite::MAIN_DB, &clone, None)
        .unwrap();
    b.store = Some(DataStore::open(&clone).unwrap());
    assert_eq!(resume(&mut b).err(), Some(InstallError::InventoryChanged));
    assert_eq!(abort(&mut b).err(), Some(InstallError::InventoryChanged));
    assert!(quarantine(&b).is_empty());
    for mode in [
        "known",
        "path",
        "extra",
        "duplicate",
        "duplicate-raw",
        "oversize",
    ] {
        let mut altered: Value = serde_json::from_str(&original).unwrap();
        match mode {
            "known" => {
                let input = a.input(manifest("consumer", vec![]), 1);
                altered["orphans"][0]["kind"] = json!("artifact");
                altered["orphans"][0]["source"] = json!(input.payload.archive.sha256);
            }
            "path" => altered["orphans"][0]["source"] = json!("../escape"),
            "extra" => altered["approved"] = json!(true),
            "duplicate" => {
                let o = altered["orphans"][0].clone();
                altered["orphans"].as_array_mut().unwrap().push(o);
            }
            "duplicate-raw" | "oversize" => (),
            _ => unreachable!(),
        }
        let text = match mode {
            "duplicate-raw" => format!("{{\"formatVersion\":1,{}", &original[1..]),
            "oversize" => " ".repeat(16_777_217),
            _ => altered.to_string(),
        };
        a.store
            .as_ref()
            .unwrap()
            .connection
            .execute(
                "UPDATE core_metadata SET value=?1 WHERE key=?2",
                [text, JOURNAL.into()],
            )
            .unwrap();
        assert!(resume(&mut a).is_err(), "{mode}");
        assert!(abort(&mut a).is_err(), "{mode}");
        assert!(source.exists());
        assert!(quarantine(&a).is_empty());
    }
    a.store
        .as_ref()
        .unwrap()
        .connection
        .execute(
            "UPDATE core_metadata SET value=?1 WHERE key=?2",
            [original, JOURNAL.into()],
        )
        .unwrap();
    resume(&mut a).unwrap().unwrap();
}

#[test]
fn collision_and_file_changes_after_journal_keep_pending_and_never_overwrite() {
    for mode in ["collision", "changed"] {
        let mut world = installed();
        let source = orphan(&world, true, b"STAGE");
        let reviewed = plan(&mut world).unwrap();
        reconcile_with_hook(
            &world.trust,
            world.store.as_mut().unwrap(),
            reviewed,
            |phase, store| {
                if phase == ReconciliationPhase::JournalCommitted {
                    if mode == "collision" {
                        let text: String = store
                            .connection
                            .query_row(
                                "SELECT value FROM core_metadata WHERE key=?1",
                                [JOURNAL],
                                |r| r.get(0),
                            )
                            .unwrap();
                        let v: Value = serde_json::from_str(&text).unwrap();
                        let destination = source
                            .parent()
                            .unwrap()
                            .parent()
                            .unwrap()
                            .join("quarantine")
                            .join(format!(
                                "{}-stage",
                                v["orphans"][0]["destinationId"].as_str().unwrap()
                            ));
                        std::fs::create_dir(&destination).unwrap();
                        std::fs::write(destination.join("package.zip"), b"KEEP").unwrap();
                    } else {
                        std::fs::write(source.join("package.zip"), b"CHANGED").unwrap();
                    }
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert!(source.exists());
        assert!(world.raw(JOURNAL).is_some());
        assert!(world.raw(RECEIPT).is_none());
        if mode == "collision" {
            assert_eq!(
                std::fs::read(quarantine(&world)[0].join("package.zip")).unwrap(),
                b"KEEP"
            );
        } else {
            assert!(quarantine(&world).is_empty());
        }
        assert!(resume(&mut world).is_err());
    }
}

#[test]
fn bounded_physical_groups_and_bytes_refuse_before_journal_creation() {
    for mode in ["groups", "bytes"] {
        let mut world = installed();
        if mode == "groups" {
            for _ in 0..384 {
                orphan(&world, true, b"");
            }
        } else {
            for _ in 0..4 {
                let p = orphan(&world, true, b"");
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(p.join("package.zip"))
                    .unwrap()
                    .set_len(MAX_ARCHIVE_BYTES)
                    .unwrap();
            }
        }
        assert_eq!(
            plan(&mut world).err().map(|e| format!("{e:?}")),
            Some("BudgetExceeded".into())
        );
        assert!(world.raw(JOURNAL).is_none());
        assert!(quarantine(&world).is_empty());
    }
}

#[test]
fn restored_pending_batch_requires_new_trust_approval_and_explicit_abort_before_replanning() {
    let mut world = installed();
    recovery_authority(&mut world);
    orphan(&world, false, b"A");
    orphan(&world, true, b"B");
    let reviewed = plan(&mut world).unwrap();
    reconcile_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        reviewed,
        |phase, _| {
            if phase == ReconciliationPhase::GroupQuarantined(0) {
                Err(InstallError::Storage)
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    let now = world.fixture.policy.clock.now;
    let checkpoint = world
        .trust
        .export_recovery_checkpoint(world.store.as_mut().unwrap(), now + 1)
        .unwrap();
    let backup = world.store.as_ref().unwrap().backup().unwrap();
    world.store.take();
    DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
    world.reopen();
    assert_eq!(
        resume(&mut world).err(),
        Some(InstallError::Package(PackageError::TrustRecoveryRequired))
    );
    assert_eq!(
        abort(&mut world).err(),
        Some(InstallError::Package(PackageError::TrustRecoveryRequired))
    );
    checkpoint_recovery(&mut world, &checkpoint, now + 2);
    assert_eq!(
        resume(&mut world).err(),
        Some(InstallError::InventoryChanged)
    );
    assert!(plan(&mut world).is_err());
    let floors = world.raw("package-trust-state-v1");
    let marker = world.raw("package-trust-recovery-required");
    assert!(abort(&mut world).unwrap());
    assert_eq!(quarantine(&world).len(), 1);
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(reviewed.summary().stages, 1);
    apply(&mut world, reviewed).unwrap();
    assert_eq!(quarantine(&world).len(), 2);
    assert_eq!(world.raw("package-trust-state-v1"), floors);
    assert_eq!(world.raw("package-trust-recovery-required"), marker);
    world.inspect().unwrap();
}

#[test]
fn junction_groups_and_leaf_redirects_refuse_without_touching_outside_canaries() {
    for leaf in [false, true] {
        let mut world = installed();
        let source = orphan(&world, true, b"STAGE");
        let outside = world.path.join("outside-fixture");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("canary"), b"KEEP").unwrap();
        std::fs::remove_file(source.join("package.zip")).unwrap();
        let redirected = if leaf {
            source.join("package.zip")
        } else {
            std::fs::remove_dir(&source).unwrap();
            source.clone()
        };
        let output = Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&redirected)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            plan(&mut world).err().map(|e| format!("{e:?}")),
            Some("PathRejected".into())
        );
        assert_eq!(std::fs::read(outside.join("canary")).unwrap(), b"KEEP");
        assert!(world.raw(JOURNAL).is_none());
        assert!(quarantine(&world).is_empty());
        std::fs::remove_dir(&redirected).unwrap();
    }
}

#[test]
fn journal_creation_and_abort_receipt_failures_preserve_every_inert_location() {
    for aborting in [false, true] {
        let mut world = installed();
        let source = orphan(&world, true, b"STAGE");
        let reviewed = plan(&mut world).unwrap();
        if aborting {
            reconcile_with_hook(
                &world.trust,
                world.store.as_mut().unwrap(),
                reviewed,
                |phase, _| {
                    if phase == ReconciliationPhase::JournalCommitted {
                        Err(InstallError::Storage)
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            world.store.as_ref().unwrap().connection.execute_batch("CREATE TRIGGER fail BEFORE INSERT ON core_metadata WHEN NEW.key='package-reconciliation-receipt-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
            let pending = world.raw(JOURNAL);
            assert_eq!(abort(&mut world).err(), Some(InstallError::Storage));
            assert_eq!(world.raw(JOURNAL), pending);
        } else {
            world.store.as_ref().unwrap().connection.execute_batch("CREATE TRIGGER fail BEFORE INSERT ON core_metadata WHEN NEW.key='package-reconciliation-pending-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
            assert_eq!(
                apply(&mut world, reviewed).err(),
                Some(InstallError::Storage)
            );
            assert!(world.raw(JOURNAL).is_none());
        }
        assert!(source.exists());
        assert_eq!(std::fs::read(source.join("package.zip")).unwrap(), b"STAGE");
        assert!(world.raw(RECEIPT).is_none());
        assert!(quarantine(&world).is_empty());
    }
}

#[test]
fn memory_profiles_refuse_and_empty_disk_plans_do_not_create_a_blob_store() {
    let mut world = World::new();
    let reviewed = plan(&mut world).unwrap();
    assert_eq!(reviewed.summary().bytes, 0);
    apply(&mut world, reviewed).unwrap();
    assert!(!world.path.join("package-store-v1").exists());
    assert!(world.raw(JOURNAL).is_none());
    assert!(world.raw(RECEIPT).is_none());
    let mut memory = DataStore::memory().unwrap();
    let root = signed(ROOT_PAYLOAD_TYPE, world.fixture.root.payload(), &[1, 2]);
    world
        .trust
        .bootstrap(&mut memory, &root, world.fixture.policy.clock.now)
        .unwrap();
    assert_eq!(
        plan_inert_reconciliation(&world.trust, &mut memory)
            .err()
            .map(|e| format!("{e:?}")),
        Some("ProfileRequired".into())
    );
}

#[test]
fn missing_quarantine_parent_refuses_before_persisting_a_batch() {
    let mut world = installed();
    let source = orphan(&world, true, b"STAGE");
    let reviewed = plan(&mut world).unwrap();
    std::fs::remove_dir(world.path.join("package-store-v1/quarantine")).unwrap();
    assert_eq!(
        apply(&mut world, reviewed).err(),
        Some(InstallError::ArtifactInvalid)
    );
    assert_eq!(
        plan(&mut world).err().map(|e| format!("{e:?}")),
        Some("ArtifactInvalid".into())
    );
    assert!(source.exists());
    assert!(world.raw(JOURNAL).is_none());
    assert!(world.raw(RECEIPT).is_none());
}

#[test]
fn reconciliation_crash_child() {
    let Ok(profile) = std::env::var("FLOWTOOLS_RECONCILIATION_TEST_PROFILE") else {
        return;
    };
    let path = PathBuf::from(profile);
    assert_eq!(path.parent().unwrap(), std::env::temp_dir());
    let id = path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("flowtools-validation-inert-install-")
        .unwrap();
    assert_eq!(uuid::Uuid::parse_str(id).unwrap().to_string(), id);
    let mut world = World::open(&path, false);
    let reviewed = plan(&mut world).unwrap();
    let barrier = std::env::var("FLOWTOOLS_RECONCILIATION_TEST_BARRIER").unwrap();
    reconcile_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        reviewed,
        |phase, _| {
            if format!("{phase:?}") == barrier {
                std::process::exit(24);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("requested reconciliation barrier not reached");
}

#[test]
fn actual_process_exit_at_each_barrier_keeps_journal_or_complete_receipt() {
    for phase in [
        ReconciliationPhase::JournalCommitted,
        ReconciliationPhase::GroupQuarantined(0),
        ReconciliationPhase::GroupQuarantined(1),
        ReconciliationPhase::BeforeCommit,
        ReconciliationPhase::Committed,
    ] {
        let mut world = installed();
        orphan(&world, false, b"A");
        orphan(&world, true, b"B");
        let before = world.raw(INVENTORY);
        world.store.take();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "packages::tests::install_tests::reconciliation_tests::reconciliation_crash_child",
                "--nocapture",
            ])
            .env("FLOWTOOLS_RECONCILIATION_TEST_PROFILE", &world.path)
            .env(
                "FLOWTOOLS_RECONCILIATION_TEST_BARRIER",
                format!("{phase:?}"),
            )
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(24),
            "{phase:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        world.reopen();
        if phase == ReconciliationPhase::Committed {
            assert!(world.raw(JOURNAL).is_none());
            assert!(world.raw(RECEIPT).is_some());
        } else {
            assert!(world.raw(JOURNAL).is_some());
            assert!(world.raw(RECEIPT).is_none());
            assert_eq!(world.inspect().err(), Some(InstallError::RecoveryPending));
            resume(&mut world).unwrap().unwrap();
        }
        assert_eq!(world.raw(INVENTORY), before);
        assert_eq!(quarantine(&world).len(), 2);
        world.inspect().unwrap();
    }
}
