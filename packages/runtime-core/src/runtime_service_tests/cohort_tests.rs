use super::*;
use std::cell::Cell;
const IDS: [&str; 2] = ["plugin-base64-encoder", "plugin-text-ops"];

fn updated() -> BuiltinCatalog {
    let manifests = BuiltinCatalog::embedded()
        .list()
        .map(|(id, manifest)| {
            let mut manifest = manifest.clone();
            if IDS.contains(&id.as_str()) {
                manifest["version"] = json!("0.2.0");
            }
            manifest
        })
        .collect();
    BuiltinCatalog::from_host_manifests(manifests).unwrap()
}
fn pending(core: &RuntimeCore) -> Option<String> {
    core.data
        .connection
        .query_row(
            "SELECT value FROM core_metadata WHERE key='service-catalog-state-v1'",
            [],
            |row| row.get(0),
        )
        .optional()
        .unwrap()
}
fn begin(core: &mut RuntimeCore) -> ProviderCohortPlan {
    let plan = core.stage_provider_cohort(updated()).unwrap();
    core.begin_provider_cohort(&plan, plan.digest()).unwrap();
    plan
}
fn assert_reopened(core: &mut RuntimeCore) {
    assert!(!core.stopping);
    assert!(core.services.matches_catalog(&BuiltinCatalog::embedded()));
    for id in IDS {
        let plan = core.provider_unload_plan(id).unwrap();
        core.begin_provider_change(plan.clone(), &plan.digest, true)
            .unwrap();
        core.abort_provider_change(&plan.digest);
    }
}

#[test]
fn cohort_commits_two_versions_once_and_restart_requires_the_exact_new_catalog() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let plan = begin(&mut core);
    assert_eq!(plan.changes().len(), 2);
    let called = Cell::new(0);
    core.commit_provider_cohort(&plan, |tx| {
        called.set(called.get() + 1);
        tx.execute(
            "INSERT INTO core_metadata(key,value) VALUES('cohort-business','migrated')",
            [],
        )
        .unwrap();
        Ok(())
    })
    .unwrap();
    assert_eq!(called.get(), 1);
    assert_eq!(core.retained_catalogs.len(), 1);
    assert_eq!(core.catalog.snapshot_digest(), updated().snapshot_digest());
    assert!(core.services.matches_catalog(&updated()));
    assert!(core.provider_cohorts.is_empty());
    assert!(plan
        .changes()
        .iter()
        .all(|change| core.services.presented_plan(&change.digest).is_err()));
    let journal: Value = serde_json::from_str(&pending(&core).unwrap()).unwrap();
    assert_eq!(journal["formatVersion"], 1);
    assert_eq!(journal["phase"], "committed");
    assert_eq!(journal["activeCatalog"], updated().snapshot_digest());
    assert!(journal["pendingCatalog"].is_null());
    drop(core);
    assert_eq!(load(&path).err(), Some(ErrorCode::RecoveryPending));
    let restarted = RuntimeCore::managed_catalog(
        HashMap::new(),
        DataStore::open(&path.join("runtime.sqlite")).unwrap(),
        false,
        false,
        updated(),
    )
    .unwrap();
    assert!(restarted.services.matches_catalog(&updated()));
    assert_eq!(
        restarted
            .data
            .connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='cohort-business'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "migrated"
    );
}

#[test]
fn cohort_waits_for_every_original_root_without_calling_migration_or_releasing_other_pins() {
    let path = profile();
    let mut core = load(&path).unwrap();
    for (index, id) in IDS.iter().enumerate() {
        let lock = ServiceRegistry::plan(&core.catalog, &[(*id).into()]).unwrap();
        core.services
            .accept(&format!("held-{index}"), lock, 1024)
            .unwrap();
    }
    let plan = begin(&mut core);
    assert_eq!(
        core.commit_provider_cohort(&plan, |_| panic!("old roots still own pins")),
        Err(ErrorCode::RuntimeBusy)
    );
    core.services.release_root("held-0");
    assert_eq!(
        core.commit_provider_cohort(&plan, |_| panic!("second root still owns its pin")),
        Err(ErrorCode::RuntimeBusy)
    );
    assert!(pending(&core).is_none());
    assert_eq!(
        core.catalog.snapshot_digest(),
        BuiltinCatalog::embedded().snapshot_digest()
    );
    core.services.release_root("held-1");
    core.commit_provider_cohort(&plan, |_| Ok(())).unwrap();
}

#[test]
fn cohort_confirmation_cannot_be_reused_after_the_accepted_graph_changes() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let plan = core.stage_provider_cohort(updated()).unwrap();
    assert_eq!(
        core.begin_provider_cohort(&plan, "incorrect"),
        Err(ErrorCode::ApprovalRequired)
    );
    let lock = ServiceRegistry::plan(&core.catalog, &[IDS[0].into()]).unwrap();
    core.services.accept("new-accepted", lock, 1024).unwrap();
    assert_eq!(
        core.begin_provider_cohort(&plan, plan.digest()),
        Err(ErrorCode::ApprovalRequired)
    );
    assert!(plan
        .changes()
        .iter()
        .all(|change| core.services.presented_plan(&change.digest).is_err()));
    core.abort_provider_cohort(&plan).unwrap();
    assert_reopened(&mut core);
}

#[test]
fn cohort_failed_migration_restores_exact_previous_bytes_and_all_old_members() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let unload = core.provider_unload_plan("plugin-random-picker").unwrap();
    core.begin_provider_change(unload.clone(), &unload.digest, false)
        .unwrap();
    core.commit_provider_change(&unload.digest, |_| Ok(()))
        .unwrap();
    let before = pending(&core).unwrap();
    let plan = begin(&mut core);
    assert_eq!(
        core.commit_provider_cohort(&plan, |tx| {
            tx.execute(
                "INSERT INTO core_metadata(key,value) VALUES('cohort-business','uncommitted')",
                [],
            )
            .unwrap();
            Err(ErrorCode::StorageFailed)
        }),
        Err(ErrorCode::StorageFailed)
    );
    assert_eq!(pending(&core).as_deref(), Some(before.as_str()));
    assert_eq!(
        core.data
            .connection
            .query_row(
                "SELECT count(*) FROM core_metadata WHERE key='cohort-business'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_reopened(&mut core);
    assert_eq!(core.retained_catalogs.len(), 0);
}

#[test]
fn cohort_unknown_commit_keeps_prepared_journal_and_stops_both_catalogs() {
    let path = profile();
    let mut core = load(&path).unwrap();
    core.data.connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE child(id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED);").unwrap();
    let plan = begin(&mut core);
    assert_eq!(
        core.commit_provider_cohort(&plan, |tx| {
            tx.execute("INSERT INTO child(id) VALUES(999)", []).unwrap();
            Ok(())
        }),
        Err(ErrorCode::StorageFailed)
    );
    assert!(core.stopping);
    assert_eq!(
        core.abort_provider_cohort(&plan),
        Err(ErrorCode::RuntimeBusy)
    );
    assert_eq!(core.retained_catalogs.len(), 0);
    let journal: Value = serde_json::from_str(&pending(&core).unwrap()).unwrap();
    assert_eq!(journal["phase"], "prepared");
    drop(core);
    for catalog in [BuiltinCatalog::embedded(), updated()] {
        assert_eq!(
            RuntimeCore::managed_catalog(
                HashMap::new(),
                DataStore::open(&path.join("runtime.sqlite")).unwrap(),
                false,
                false,
                catalog
            )
            .err(),
            Some(ErrorCode::RecoveryPending)
        );
    }
}

#[test]
fn cohort_members_cannot_be_split_through_single_provider_operations() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let unload = core.provider_unload_plan(IDS[0]).unwrap();
    let plan = core.stage_provider_cohort(updated()).unwrap();
    assert_eq!(
        core.begin_provider_change(unload.clone(), &unload.digest, true),
        Err(ErrorCode::ApprovalRequired)
    );
    for change in plan.changes() {
        assert_eq!(
            core.begin_provider_change(change.clone(), &change.digest, false),
            Err(ErrorCode::ApprovalRequired)
        );
    }
    core.begin_provider_cohort(&plan, plan.digest()).unwrap();
    for change in plan.changes() {
        core.abort_provider_change(&change.digest);
        assert_eq!(
            core.commit_provider_change(&change.digest, |_| panic!("cohort cannot be split")),
            Err(ErrorCode::ApprovalRequired)
        );
        assert!(core.services.presented_plan(&change.digest).is_ok());
    }
    core.abort_provider_cohort(&plan).unwrap();
    assert_reopened(&mut core);
}

#[test]
fn cohort_refuses_downgrades_identity_changes_and_empty_or_incomplete_catalogs() {
    let path = profile();
    let mut core = load(&path).unwrap();
    for (field, value) in [
        ("version", "0.0.1"),
        ("version", "0.1.0"),
        ("publisher", "another-publisher"),
    ] {
        let mut manifests = updated()
            .list()
            .map(|(_, manifest)| manifest.clone())
            .collect::<Vec<_>>();
        let changed = manifests
            .iter_mut()
            .find(|manifest| manifest["id"] == IDS[0])
            .unwrap();
        changed[field] = json!(value);
        changed["name"] = json!("Changed metadata requires a new version");
        let catalog = BuiltinCatalog::from_host_manifests(manifests).unwrap();
        assert_eq!(
            core.stage_provider_cohort(catalog).err(),
            Some(ErrorCode::DependencyInvalid)
        );
    }
    assert_eq!(
        core.stage_provider_cohort(BuiltinCatalog::embedded()).err(),
        Some(ErrorCode::DependencyInvalid)
    );
    let incomplete = BuiltinCatalog::from_host_manifests(
        updated()
            .list()
            .skip(1)
            .map(|(_, manifest)| manifest.clone())
            .collect(),
    )
    .unwrap();
    assert_eq!(
        core.stage_provider_cohort(incomplete).err(),
        Some(ErrorCode::DependencyInvalid)
    );
    assert!(core.provider_cohorts.is_empty());
    assert!(pending(&core).is_none());
}

#[test]
fn cohort_migration_cannot_substitute_policy_or_tool_authority() {
    for sql in [
        "INSERT INTO grants(identity,epoch,state) VALUES('foreign',1,'foreign')",
        "UPDATE core_metadata SET value='foreign' WHERE key='catalog-digest'",
        "INSERT INTO core_metadata(key,value) VALUES('package-trust-foreign','foreign')",
    ] {
        let path = profile();
        let mut core = load(&path).unwrap();
        let plan = begin(&mut core);
        assert_eq!(
            core.commit_provider_cohort(&plan, |tx| {
                tx.execute(sql, []).unwrap();
                Ok(())
            }),
            Err(ErrorCode::StoreCorrupt)
        );
        assert!(pending(&core).is_none());
        assert_eq!(
            core.data
                .connection
                .query_row(
                    "SELECT count(*) FROM grants WHERE identity='foreign'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            core.data
                .connection
                .query_row(
                    "SELECT count(*) FROM core_metadata WHERE value='foreign'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_reopened(&mut core);
    }
}

#[test]
fn cohort_foreign_handles_and_overlapping_stages_do_not_replace_the_original_plan() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let plan = core.stage_provider_cohort(updated()).unwrap();
    assert_eq!(
        core.stage_provider_cohort(updated()).err(),
        Some(ErrorCode::RuntimeBusy)
    );
    let other_path = profile();
    let mut other = load(&other_path).unwrap();
    assert_eq!(
        other.begin_provider_cohort(&plan, plan.digest()),
        Err(ErrorCode::InstanceMismatch)
    );
    assert_eq!(
        other.abort_provider_cohort(&plan),
        Err(ErrorCode::InstanceMismatch)
    );
    core.begin_provider_cohort(&plan, plan.digest()).unwrap();
    core.commit_provider_cohort(&plan, |_| Ok(())).unwrap();
    assert!(other.provider_cohorts.is_empty());
    assert!(pending(&other).is_none());
}

#[test]
fn cohort_authority_rows_are_bounded_before_migration_without_erasing_bad_bytes() {
    for state in [
        rusqlite::types::Value::Text("x".repeat(65537)),
        rusqlite::types::Value::Blob(vec![9; 65537]),
    ] {
        let path = profile();
        let mut core = load(&path).unwrap();
        let plan = begin(&mut core);
        core.data
            .connection
            .execute(
                "INSERT INTO grants(identity,epoch,state) VALUES('foreign',1,?1)",
                [&state],
            )
            .unwrap();
        assert_eq!(
            core.commit_provider_cohort(&plan, |_| panic!("invalid authority row")),
            Err(ErrorCode::StoreCorrupt)
        );
        assert!(pending(&core).is_none());
        assert_eq!(
            core.data
                .connection
                .query_row(
                    "SELECT state FROM grants WHERE identity='foreign'",
                    [],
                    |row| row.get::<_, rusqlite::types::Value>(0)
                )
                .unwrap(),
            state
        );
        core.abort_provider_cohort(&plan).unwrap();
    }
}

#[test]
fn cohort_unconfirmed_journal_restore_keeps_all_members_blocked_and_prepared() {
    let path = profile();
    let mut core = load(&path).unwrap();
    core.data.connection.execute_batch("CREATE TRIGGER corrupt_restore BEFORE DELETE ON core_metadata WHEN OLD.key='service-catalog-state-v1' BEGIN UPDATE core_metadata SET value='foreign' WHERE key=OLD.key; SELECT RAISE(IGNORE); END;").unwrap();
    let plan = begin(&mut core);
    assert_eq!(
        core.commit_provider_cohort(&plan, |_| Err(ErrorCode::ExecutionFailed)),
        Err(ErrorCode::StoreCorrupt)
    );
    assert!(core.stopping);
    assert_eq!(
        core.abort_provider_cohort(&plan),
        Err(ErrorCode::RuntimeBusy)
    );
    for change in plan.changes() {
        assert!(core.services.presented_plan(&change.digest).is_ok());
    }
    let journal: Value = serde_json::from_str(&pending(&core).unwrap()).unwrap();
    assert_eq!(journal["phase"], "prepared");
    assert_eq!(
        core.catalog.snapshot_digest(),
        BuiltinCatalog::embedded().snapshot_digest()
    );
    drop(core);
    assert_eq!(load(&path).err(), Some(ErrorCode::RecoveryPending));
}
