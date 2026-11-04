use super::*;
use std::cell::Cell;
const KEY: &str = "service-catalog-state-v1";

fn begin(core: &mut RuntimeCore, id: &str) -> String {
    let plan = core.provider_unload_plan(id).unwrap();
    core.begin_provider_change(plan.clone(), &plan.digest, false)
        .unwrap();
    plan.digest
}
fn raw(core: &RuntimeCore) -> Option<String> {
    core.data
        .connection
        .query_row("SELECT value FROM core_metadata WHERE key=?1", [KEY], |r| {
            r.get(0)
        })
        .optional()
        .unwrap()
}

#[test]
fn provider_journal_preparation_trigger_cannot_reach_the_migration_callback() {
    let path = profile();
    let mut core = load(&path).unwrap();
    core.data.connection.execute_batch("CREATE TRIGGER replace_prepared AFTER INSERT ON core_metadata WHEN NEW.key='service-catalog-state-v1' BEGIN UPDATE core_metadata SET value='FOREIGN_PREPARED' WHERE key=NEW.key; END;").unwrap();
    let ticket = begin(&mut core, "plugin-base64-encoder");
    let called = Cell::new(false);
    assert_eq!(
        core.commit_provider_change(&ticket, |_| {
            called.set(true);
            Ok(())
        }),
        Err(ErrorCode::StoreCorrupt)
    );
    assert!(!called.get());
    assert!(core.stopping);
    assert!(raw(&core).is_none());
}

#[test]
fn provider_journal_ignored_preparation_is_not_durable_acceptance() {
    let path = profile();
    let mut core = load(&path).unwrap();
    core.data.connection.execute_batch("CREATE TRIGGER ignore_prepared BEFORE INSERT ON core_metadata WHEN NEW.key='service-catalog-state-v1' BEGIN SELECT RAISE(IGNORE); END;").unwrap();
    let ticket = begin(&mut core, "plugin-base64-encoder");
    let called = Cell::new(false);
    assert_eq!(
        core.commit_provider_change(&ticket, |_| {
            called.set(true);
            Ok(())
        }),
        Err(ErrorCode::StoreCorrupt)
    );
    assert!(!called.get());
    assert!(core.stopping);
    assert!(raw(&core).is_none());
}

#[test]
fn provider_journal_committed_trigger_is_rolled_back_before_reopening_the_old_catalog() {
    let path = profile();
    let mut core = load(&path).unwrap();
    core.data.connection.execute_batch("CREATE TRIGGER replace_committed AFTER UPDATE ON core_metadata WHEN NEW.key='service-catalog-state-v1' AND json_extract(NEW.value,'$.phase')='committed' BEGIN UPDATE core_metadata SET value='FOREIGN_COMMITTED' WHERE key=NEW.key; END;").unwrap();
    let ticket = begin(&mut core, "plugin-base64-encoder");
    assert_eq!(
        core.commit_provider_change(&ticket, |_| Ok(())),
        Err(ErrorCode::StoreCorrupt)
    );
    assert!(!core.stopping);
    assert!(raw(&core).is_none());
    assert!(core
        .services
        .enabled(&crate::dependencies::DependencyIdentity {
            publisher: "flowtools".into(),
            id: "plugin-base64-encoder".into()
        }));
}

#[test]
fn provider_journal_migration_cannot_erase_a_substituted_predecessor() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let ticket = begin(&mut core, "plugin-base64-encoder");
    assert_eq!(
        core.commit_provider_change(&ticket, |tx| {
            tx.execute(
                "UPDATE core_metadata SET value='FOREIGN_MIGRATION' WHERE key=?1",
                [KEY],
            )
            .unwrap();
            Ok(())
        }),
        Err(ErrorCode::StoreCorrupt)
    );
    assert!(!core.stopping);
    assert!(raw(&core).is_none());
}

#[test]
fn provider_journal_rollback_delete_cannot_hide_a_trigger_substitution() {
    let path = profile();
    let mut core = load(&path).unwrap();
    core.data.connection.execute_batch("CREATE TRIGGER replace_restore BEFORE DELETE ON core_metadata WHEN OLD.key='service-catalog-state-v1' BEGIN UPDATE core_metadata SET value='FOREIGN_RESTORE' WHERE key=OLD.key; SELECT RAISE(IGNORE); END;").unwrap();
    let ticket = begin(&mut core, "plugin-base64-encoder");
    assert_eq!(
        core.commit_provider_change(&ticket, |_| Err(ErrorCode::StorageFailed)),
        Err(ErrorCode::StoreCorrupt)
    );
    assert!(core.stopping);
    let journal: Value = serde_json::from_str(&raw(&core).unwrap()).unwrap();
    assert_eq!(journal["phase"], "prepared");
    drop(core);
    assert_eq!(load(&path).err(), Some(ErrorCode::RecoveryPending));
}

#[test]
fn provider_journal_corrupt_or_over_budget_predecessors_refuse_before_callback_and_keep_bytes() {
    for value in [
        rusqlite::types::Value::Text("PRIVATE_CORRUPT_JOURNAL".into()),
        rusqlite::types::Value::Text("x".repeat(65537)),
        rusqlite::types::Value::Blob(vec![7; 131072]),
    ] {
        let path = profile();
        let mut core = load(&path).unwrap();
        core.data
            .connection
            .execute(
                "INSERT INTO core_metadata(key,value) VALUES(?1,?2)",
                rusqlite::params![KEY, value],
            )
            .unwrap();
        let original: (String, String) = core
            .data
            .connection
            .query_row(
                "SELECT typeof(value),hex(value) FROM core_metadata WHERE key=?1",
                [KEY],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let ticket = begin(&mut core, "plugin-base64-encoder");
        let called = Cell::new(false);
        assert_eq!(
            core.commit_provider_change(&ticket, |_| {
                called.set(true);
                Ok(())
            }),
            Err(ErrorCode::StoreCorrupt)
        );
        assert!(!called.get());
        assert!(core.stopping);
        let after: (String, String) = core
            .data
            .connection
            .query_row(
                "SELECT typeof(value),hex(value) FROM core_metadata WHERE key=?1",
                [KEY],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(after, original);
    }
}

#[test]
fn provider_journal_valid_noncanonical_predecessor_is_restored_byte_for_byte() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let ticket = begin(&mut core, "plugin-base64-encoder");
    core.commit_provider_change(&ticket, |_| Ok(())).unwrap();
    let previous = format!(" \n{}\n ", raw(&core).unwrap());
    core.data
        .connection
        .execute(
            "UPDATE core_metadata SET value=?1 WHERE key=?2",
            rusqlite::params![previous, KEY],
        )
        .unwrap();
    drop(core);
    let mut core = load(&path).unwrap();
    let ticket = begin(&mut core, "plugin-text-ops");
    assert_eq!(
        core.commit_provider_change(&ticket, |_| Err(ErrorCode::StorageFailed)),
        Err(ErrorCode::StorageFailed)
    );
    assert!(!core.stopping);
    assert_eq!(raw(&core).as_deref(), Some(previous.as_str()));
    drop(core);
    assert!(load(&path).is_ok());
}

#[test]
fn provider_journal_missing_committed_receipt_cannot_be_recreated_by_a_cached_actor() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let ticket = begin(&mut core, "plugin-base64-encoder");
    core.commit_provider_change(&ticket, |_| Ok(())).unwrap();
    core.data
        .connection
        .execute("DELETE FROM core_metadata WHERE key=?1", [KEY])
        .unwrap();
    let ticket = begin(&mut core, "plugin-text-ops");
    let called = Cell::new(false);
    assert_eq!(
        core.commit_provider_change(&ticket, |_| {
            called.set(true);
            Ok(())
        }),
        Err(ErrorCode::StoreCorrupt)
    );
    assert!(!called.get());
    assert!(core.stopping);
    assert!(raw(&core).is_none());
}

#[test]
fn provider_journal_trigger_side_effects_refuse_even_when_the_journal_readback_matches() {
    let path = profile();
    let mut core = load(&path).unwrap();
    core.data.connection.execute_batch("CREATE TRIGGER extra_write AFTER INSERT ON core_metadata WHEN NEW.key='service-catalog-state-v1' BEGIN INSERT INTO core_metadata(key,value) VALUES('foreign-side-effect','PRIVATE'); END;").unwrap();
    let ticket = begin(&mut core, "plugin-base64-encoder");
    let called = Cell::new(false);
    assert_eq!(
        core.commit_provider_change(&ticket, |_| {
            called.set(true);
            Ok(())
        }),
        Err(ErrorCode::StoreCorrupt)
    );
    assert!(!called.get());
    assert!(core.stopping);
    assert!(raw(&core).is_none());
    assert_eq!(
        core.data
            .connection
            .query_row(
                "SELECT count(*) FROM core_metadata WHERE key='foreign-side-effect'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn provider_journal_rollback_update_preserves_prepared_bytes_when_a_trigger_replaces_them() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let ticket = begin(&mut core, "plugin-base64-encoder");
    core.commit_provider_change(&ticket, |_| Ok(())).unwrap();
    let previous = raw(&core).unwrap();
    core.data.connection.execute_batch("CREATE TRIGGER replace_restore AFTER UPDATE ON core_metadata WHEN NEW.key='service-catalog-state-v1' AND json_extract(NEW.value,'$.phase')='committed' BEGIN UPDATE core_metadata SET value='FOREIGN_RESTORE' WHERE key=NEW.key; END;").unwrap();
    let ticket = begin(&mut core, "plugin-text-ops");
    assert_eq!(
        core.commit_provider_change(&ticket, |_| Err(ErrorCode::StorageFailed)),
        Err(ErrorCode::StoreCorrupt)
    );
    assert!(core.stopping);
    let actual = raw(&core).unwrap();
    assert_ne!(actual, previous);
    let journal: Value = serde_json::from_str(&actual).unwrap();
    assert_eq!(journal["phase"], "prepared");
    assert_eq!(core.provider_journal.as_deref(), Some(actual.as_str()));
    drop(core);
    assert_eq!(load(&path).err(), Some(ErrorCode::RecoveryPending));
}

#[test]
fn provider_journal_live_actor_refuses_a_valid_but_byte_different_committed_receipt() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let ticket = begin(&mut core, "plugin-base64-encoder");
    core.commit_provider_change(&ticket, |_| Ok(())).unwrap();
    let different = format!("\n{}\n", raw(&core).unwrap());
    core.data
        .connection
        .execute(
            "UPDATE core_metadata SET value=?1 WHERE key=?2",
            rusqlite::params![different, KEY],
        )
        .unwrap();
    let ticket = begin(&mut core, "plugin-text-ops");
    let called = Cell::new(false);
    assert_eq!(
        core.commit_provider_change(&ticket, |_| {
            called.set(true);
            Ok(())
        }),
        Err(ErrorCode::StoreCorrupt)
    );
    assert!(!called.get());
    assert!(core.stopping);
    assert_eq!(raw(&core).as_deref(), Some(different.as_str()));
    drop(core);
    // Restart validates and binds the actual old-format bytes. This is not a
    // claim of protection against an owner rewriting the whole database.
    assert!(load(&path).is_ok());
}

#[test]
fn provider_journal_restore_refuses_nontext_and_over_budget_rows_without_erasing_them() {
    for value in [
        rusqlite::types::Value::Text("x".repeat(65537)),
        rusqlite::types::Value::Blob(vec![9; 131072]),
    ] {
        let path = profile();
        let core = load(&path).unwrap();
        core.data
            .connection
            .execute(
                "INSERT INTO core_metadata(key,value) VALUES(?1,?2)",
                rusqlite::params![KEY, value],
            )
            .unwrap();
        let original: String = core
            .data
            .connection
            .query_row(
                "SELECT hex(value) FROM core_metadata WHERE key=?1",
                [KEY],
                |row| row.get(0),
            )
            .unwrap();
        drop(core);
        assert_eq!(load(&path).err(), Some(ErrorCode::StoreCorrupt));
        let db = rusqlite::Connection::open(path.join("runtime.sqlite")).unwrap();
        let retained: String = db
            .query_row(
                "SELECT hex(value) FROM core_metadata WHERE key=?1",
                [KEY],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retained, original);
    }
}
