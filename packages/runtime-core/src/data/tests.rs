use super::*;
use crate::protocol::LegacySource;
use serde_json::json;

fn profile() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "flowtools-validation-data-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
fn write(key: &str, revision: u32, value: Value) -> DataMutation {
    DataMutation {
        key: key.into(),
        expected_revision: revision,
        value,
    }
}

#[test]
fn namespaces_cas_and_transaction_rollback_preserve_the_original_values() {
    let mut store = DataStore::memory().unwrap();
    let first = store
        .write("flowtools:alpha", &write("todos", 0, json!(["one"])))
        .unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(store.read("flowtools:beta", "todos").unwrap().revision, 0);
    assert_eq!(
        store
            .write("flowtools:alpha", &write("todos", 0, json!(["lost"])))
            .unwrap_err(),
        ErrorCode::RevisionConflict
    );
    let transaction = vec![write("other", 0, json!(1)), write("todos", 0, json!([]))];
    assert_eq!(
        store
            .transaction("flowtools:alpha", &transaction)
            .unwrap_err(),
        ErrorCode::RevisionConflict
    );
    assert_eq!(store.read("flowtools:alpha", "other").unwrap().revision, 0);
    assert_eq!(
        store.read("flowtools:alpha", "todos").unwrap().value,
        json!(["one"])
    );
    assert_eq!(
        store
            .transaction(
                "flowtools:alpha",
                &[
                    write("todos", 1, json!(["two"])),
                    write("other", 0, json!(2))
                ]
            )
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn restart_preserves_data_and_second_writer_is_rejected() {
    let path = profile().join("runtime.sqlite");
    let mut store = DataStore::open(&path).unwrap();
    store
        .write("flowtools:alpha", &write("todos", 0, json!("private-data")))
        .unwrap();
    assert!(matches!(DataStore::open(&path), Err(ErrorCode::StoreBusy)));
    drop(store);
    let store = DataStore::open(&path).unwrap();
    assert_eq!(
        store.read("flowtools:alpha", "todos").unwrap().value,
        json!("private-data")
    );
    let tables: Vec<String> = store
        .connection
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for table in ["core_metadata", "grants", "jobs", "plugin_data", "imports"] {
        assert!(tables.contains(&table.into()));
    }
}

#[test]
fn n_minus_one_migration_backups_and_injected_failure_roll_back() {
    let directory = profile();
    let path = directory.join("runtime.sqlite");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch(SCHEMA_V1).unwrap();
    connection
        .execute(
            "INSERT INTO core_metadata(key,value) VALUES('fixture','preserved')",
            [],
        )
        .unwrap();
    drop(connection);
    assert!(matches!(
        DataStore::open_with_hook(&path, || Err(ErrorCode::StorageFailed)),
        Err(ErrorCode::StorageFailed)
    ));
    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='fixture'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "preserved"
    );
    drop(connection);
    let store = DataStore::open(&path).unwrap();
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        SCHEMA_VERSION
    );
    assert!(std::fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .any(|item| item
            .file_name()
            .to_string_lossy()
            .starts_with("runtime-backup-")));
}

#[test]
fn corrupt_or_newer_databases_are_preserved_and_explicit_backup_recovery_works() {
    let directory = profile();
    let path = directory.join("runtime.sqlite");
    let mut store = DataStore::open(&path).unwrap();
    store
        .write("flowtools:alpha", &write("todos", 0, json!(["recover"])))
        .unwrap();
    let backup = store.backup().unwrap();
    drop(store);
    std::fs::write(&path, b"corrupt-original-canary").unwrap();
    assert!(matches!(
        DataStore::open(&path),
        Err(ErrorCode::StoreCorrupt)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"corrupt-original-canary");
    let quarantine = DataStore::restore_backup(&path, &backup).unwrap();
    assert_eq!(
        std::fs::read(quarantine).unwrap(),
        b"corrupt-original-canary"
    );
    let store = DataStore::open(&path).unwrap();
    assert_eq!(
        store.read("flowtools:alpha", "todos").unwrap().value,
        json!(["recover"])
    );
    drop(store);
    let connection = Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    drop(connection);
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        DataStore::open(&path),
        Err(ErrorCode::SchemaUnsupported)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn imports_are_explicit_validated_idempotent_and_never_overwrite_existing_or_corrupt_data() {
    let mut store = DataStore::memory().unwrap();
    let items = json!([{"todo":"legacy", "deadline":""}]);
    let request = LegacyImport {
        source: LegacySource::CliV0,
        source_digest: crate::catalog::digest(&items),
        value: items.clone(),
    };
    let imported = store
        .import_todos("flowtools:plugin-todo-list", &request)
        .unwrap();
    assert_eq!(imported.revision, 1);
    assert_eq!(
        store
            .import_todos("flowtools:plugin-todo-list", &request)
            .unwrap()
            .revision,
        1
    );
    let different = json!([{"todo":"other", "deadline":""}]);
    assert_eq!(
        store
            .import_todos(
                "flowtools:plugin-todo-list",
                &LegacyImport {
                    value: different.clone(),
                    source_digest: crate::catalog::digest(&different),
                    ..request.clone()
                }
            )
            .unwrap_err(),
        ErrorCode::RevisionConflict
    );
    assert_eq!(
        store
            .import_todos(
                "flowtools:other",
                &LegacyImport {
                    source_digest: "spoof".into(),
                    ..request.clone()
                }
            )
            .unwrap_err(),
        ErrorCode::InputInvalid
    );
    assert_eq!(
        store
            .import_todos(
                "flowtools:other",
                &LegacyImport {
                    value: json!([{"todo":3}]),
                    ..request
                }
            )
            .unwrap_err(),
        ErrorCode::InputInvalid
    );
    assert_eq!(
        store
            .read("flowtools:plugin-todo-list", "todos")
            .unwrap()
            .value,
        items
    );
}

#[test]
fn malicious_keys_and_large_or_duplicate_transactions_fail_without_writes() {
    let mut store = DataStore::memory().unwrap();
    for key in [
        "../grants",
        "core_metadata;DROP TABLE grants",
        "",
        "foreign:todos",
        "todos/other",
    ] {
        assert_eq!(
            store
                .write("flowtools:alpha", &write(key, 0, json!(1)))
                .unwrap_err(),
            ErrorCode::InputInvalid
        );
    }
    assert_eq!(
        store
            .write(
                "flowtools:alpha",
                &write("todos", 0, json!("x".repeat(MAX_DATA_BYTES)))
            )
            .unwrap_err(),
        ErrorCode::BudgetExceeded
    );
    assert_eq!(
        store
            .transaction(
                "flowtools:alpha",
                &[write("todos", 0, json!(1)), write("todos", 0, json!(2))]
            )
            .unwrap_err(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        store.transaction("flowtools:alpha", &[]).unwrap_err(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(store.read("flowtools:alpha", "todos").unwrap().revision, 0);
}

#[test]
fn pending_recovery_refuses_fresh_database_creation() {
    let directory = profile();
    std::fs::write(directory.join("runtime-recovery-pending"), b"pending").unwrap();
    let path = directory.join("runtime.sqlite");
    assert!(matches!(
        DataStore::open(&path),
        Err(ErrorCode::StorageFailed)
    ));
    assert!(!path.exists());
}

#[test]
fn namespace_storage_quota_rolls_back_the_excess_record() {
    let mut store = DataStore::memory().unwrap();
    for index in 0..256 {
        store
            .write(
                "flowtools:alpha",
                &write(&format!("key-{index}"), 0, json!(true)),
            )
            .unwrap();
    }
    assert_eq!(
        store
            .write("flowtools:alpha", &write("overflow", 0, json!(true)))
            .unwrap_err(),
        ErrorCode::BudgetExceeded
    );
    assert_eq!(
        store.read("flowtools:alpha", "overflow").unwrap().revision,
        0
    );
}
