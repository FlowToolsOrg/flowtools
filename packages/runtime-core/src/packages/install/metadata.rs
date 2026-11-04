//! Bounded installation-only SQL mutations under the existing writer transaction.
//! These guards reject additional trigger writes, including a replacement erased
//! by the same DELETE statement. They never authorize a filesystem effect.
use super::InstallError;
use rusqlite::{params, OptionalExtension, Transaction};

const MAX_BYTES: usize = 16_777_216;
const KEYS: &[&str] = &[
    "package-install-inventory-v1",
    "package-install-pending-v1",
    "package-reconciliation-pending-v1",
    "package-reconciliation-receipt-v1",
    "tool-archive-inventory-v1",
    "tool-archive-install-pending-v1",
    "tool-archive-reconciliation-pending-v1",
    "tool-archive-reconciliation-receipt-v1",
    "tool-image-inventory-v1",
    "tool-image-materialization-pending-v1",
    "tool-image-reconciliation-pending-v1",
    "tool-image-reconciliation-receipt-v1",
    "tool-consumer-inventory-v1",
    "tool-consumer-install-pending-v1",
    "tool-consumer-reconciliation-pending-v1",
    "tool-consumer-reconciliation-receipt-v1",
];
fn slot(key: &str) -> Result<(), InstallError> {
    if KEYS.contains(&key) {
        Ok(())
    } else {
        Err(InstallError::StateInvalid)
    }
}
fn raw(tx: &Transaction<'_>, key: &str) -> Result<Option<String>, InstallError> {
    let value: Option<Option<String>> = tx.query_row(
        "SELECT CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=16777216 THEN value ELSE NULL END FROM core_metadata WHERE key=?1",
        [key], |r| r.get(0),
    ).optional().map_err(|_| InstallError::Storage)?;
    match value {
        Some(None) => Err(InstallError::StateInvalid),
        Some(Some(value)) => Ok(Some(value)),
        None => Ok(None),
    }
}
fn one_change(tx: &Transaction<'_>, before: u64, affected: usize) -> Result<(), InstallError> {
    // changes() omits trigger effects. total_changes() includes them even when
    // they are overwritten, deleted or restored before the statement returns.
    if affected != 1 || before.checked_add(1) != Some(tx.total_changes()) {
        Err(InstallError::InventoryChanged)
    } else {
        Ok(())
    }
}
pub(crate) fn write(tx: &Transaction<'_>, key: &str, text: &str) -> Result<(), InstallError> {
    slot(key)?;
    if tx.is_autocommit() {
        return Err(InstallError::StateInvalid);
    }
    if text.len() > MAX_BYTES {
        return Err(InstallError::BudgetExceeded);
    }
    raw(tx, key)?;
    let before = tx.total_changes();
    let affected = tx.execute(
        "INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key,text],
    ).map_err(|_| InstallError::Storage)?;
    one_change(tx, before, affected)?;
    if raw(tx, key)?.as_deref() != Some(text) {
        return Err(InstallError::InventoryChanged);
    }
    Ok(())
}
pub(crate) fn remove_pending(tx: &Transaction<'_>, key: &str) -> Result<(), InstallError> {
    slot(key)?;
    if tx.is_autocommit() {
        return Err(InstallError::StateInvalid);
    }
    if !key.ends_with("-pending-v1") {
        return Err(InstallError::StateInvalid);
    }
    let original = raw(tx, key)?.ok_or(InstallError::InventoryChanged)?;
    let before = tx.total_changes();
    let affected = tx.execute(
        "DELETE FROM core_metadata WHERE key=?1 AND typeof(value)='text' AND CAST(value AS BLOB)=CAST(?2 AS BLOB)",
        params![key,original],
    ).map_err(|_| InstallError::Storage)?;
    one_change(tx, before, affected)?;
    if raw(tx, key)?.is_some() {
        return Err(InstallError::InventoryChanged);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DataStore;

    #[test]
    fn every_fixed_slot_preserves_exact_bytes_and_allows_only_its_single_row_change() {
        for key in KEYS {
            let mut store = DataStore::memory().unwrap();
            let tx = store.connection.transaction().unwrap();
            let encoded = " {\"formatVersion\":2,\"records\":[]}\n";
            write(&tx, key, encoded).unwrap();
            assert_eq!(raw(&tx, key).unwrap().as_deref(), Some(encoded));
            write(&tx, key, "{\"formatVersion\":1}").unwrap();
            if key.ends_with("-pending-v1") {
                remove_pending(&tx, key).unwrap();
                assert!(raw(&tx, key).unwrap().is_none());
                assert_eq!(
                    remove_pending(&tx, key),
                    Err(InstallError::InventoryChanged)
                );
            } else {
                assert_eq!(remove_pending(&tx, key), Err(InstallError::StateInvalid));
            }
            tx.commit().unwrap();
        }
    }
    #[test]
    fn insert_and_update_triggers_cannot_commit_even_hidden_or_reverted_foreign_changes() {
        for key in KEYS {
            for updating in [false, true] {
                let mut store = DataStore::memory().unwrap();
                store
                    .connection
                    .execute(
                        "INSERT INTO core_metadata(key,value) VALUES('sentinel','ORIGINAL')",
                        [],
                    )
                    .unwrap();
                if updating {
                    store
                        .connection
                        .execute(
                            "INSERT INTO core_metadata(key,value) VALUES(?1,'OLD')",
                            [key],
                        )
                        .unwrap();
                }
                let event = if updating { "UPDATE" } else { "INSERT" };
                store.connection.execute_batch(&format!(
                    "CREATE TRIGGER hidden_write AFTER {event} ON core_metadata WHEN NEW.key='{key}' BEGIN UPDATE core_metadata SET value='HIDDEN' WHERE key='sentinel'; UPDATE core_metadata SET value='ORIGINAL' WHERE key='sentinel'; END;"
                )).unwrap();
                let tx = store.connection.transaction().unwrap();
                assert_eq!(
                    write(&tx, key, "NEW"),
                    Err(InstallError::InventoryChanged),
                    "{key} {event}"
                );
                tx.rollback().unwrap();
                let stored: Option<String> = store
                    .connection
                    .query_row("SELECT value FROM core_metadata WHERE key=?1", [key], |r| {
                        r.get(0)
                    })
                    .optional()
                    .unwrap();
                assert_eq!(stored.as_deref(), updating.then_some("OLD"));
                let sentinel: String = store
                    .connection
                    .query_row(
                        "SELECT value FROM core_metadata WHERE key='sentinel'",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(sentinel, "ORIGINAL");
            }
        }
    }
    #[test]
    fn delete_triggers_cannot_hide_replacement_ignore_or_foreign_writes() {
        for key in KEYS.iter().filter(|key| key.ends_with("-pending-v1")) {
            for (timing, effect) in [
                (
                    "BEFORE",
                    format!("UPDATE core_metadata SET value='REPLACED' WHERE key='{key}'"),
                ),
                ("BEFORE", "SELECT RAISE(IGNORE)".into()),
                (
                    "AFTER",
                    format!("INSERT INTO core_metadata(key,value) VALUES('{key}','REPLACED')"),
                ),
                (
                    "AFTER",
                    "UPDATE core_metadata SET value='REPLACED' WHERE key='sentinel'".into(),
                ),
            ] {
                let mut store = DataStore::memory().unwrap();
                store
                    .connection
                    .execute(
                        "INSERT INTO core_metadata(key,value) VALUES('sentinel','ORIGINAL')",
                        [],
                    )
                    .unwrap();
                store
                    .connection
                    .execute(
                        "INSERT INTO core_metadata(key,value) VALUES(?1,'ORIGINAL_PENDING')",
                        [key],
                    )
                    .unwrap();
                store.connection.execute_batch(&format!("CREATE TRIGGER replace_on_delete {timing} DELETE ON core_metadata WHEN OLD.key='{key}' BEGIN {effect}; END;")).unwrap();
                let tx = store.connection.transaction().unwrap();
                assert_eq!(
                    remove_pending(&tx, key),
                    Err(InstallError::InventoryChanged),
                    "{key}: {effect}"
                );
                tx.rollback().unwrap();
                let original: String = store
                    .connection
                    .query_row("SELECT value FROM core_metadata WHERE key=?1", [key], |r| {
                        r.get(0)
                    })
                    .unwrap();
                assert_eq!(original, "ORIGINAL_PENDING");
                let sentinel: String = store
                    .connection
                    .query_row(
                        "SELECT value FROM core_metadata WHERE key='sentinel'",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(sentinel, "ORIGINAL");
            }
        }
    }
    #[test]
    fn an_already_ended_transaction_never_performs_an_unprotected_write_or_delete() {
        let mut store = DataStore::memory().unwrap();
        let tx = store.connection.transaction().unwrap();
        write(&tx, KEYS[1], "ORIGINAL_PENDING").unwrap();
        tx.execute_batch("COMMIT").unwrap();
        assert!(tx.is_autocommit());
        assert_eq!(write(&tx, KEYS[0], "NEW"), Err(InstallError::StateInvalid));
        assert_eq!(
            remove_pending(&tx, KEYS[1]),
            Err(InstallError::StateInvalid)
        );
        drop(tx);
        let original: String = store
            .connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key=?1",
                [KEYS[1]],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(original, "ORIGINAL_PENDING");
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM core_metadata WHERE key=?1",
                    [KEYS[0]],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }
    #[test]
    fn unknown_slots_and_unbounded_or_non_text_records_refuse_before_mutation() {
        let mut store = DataStore::memory().unwrap();
        let key = KEYS[0];
        let tx = store.connection.transaction().unwrap();
        assert_eq!(
            write(&tx, "caller-store", "{}"),
            Err(InstallError::StateInvalid)
        );
        assert_eq!(
            remove_pending(&tx, "caller-pending-v1"),
            Err(InstallError::StateInvalid)
        );
        assert_eq!(
            write(&tx, key, &"x".repeat(MAX_BYTES + 1)),
            Err(InstallError::BudgetExceeded)
        );
        tx.execute(
            "INSERT INTO core_metadata(key,value) VALUES(?1,zeroblob(16777217))",
            [key],
        )
        .unwrap();
        assert_eq!(write(&tx, key, "{}"), Err(InstallError::StateInvalid));
        tx.rollback().unwrap();
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM core_metadata WHERE key=?1",
                    [key],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        for sql in [
            "INSERT INTO core_metadata(key,value) VALUES('package-install-pending-v1',zeroblob(4))",
            "INSERT INTO core_metadata(key,value) VALUES('package-install-pending-v1',printf('%.*c',16777217,'x'))",
        ] {
            let tx = store.connection.transaction().unwrap();
            tx.execute(sql, []).unwrap();
            assert_eq!(remove_pending(&tx, KEYS[1]), Err(InstallError::StateInvalid));
            tx.rollback().unwrap();
        }
    }
}
