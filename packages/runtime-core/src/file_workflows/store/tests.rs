use super::*;
use crate::file_workflows::prepare_rename;
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    directory: PathBuf,
    profile: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("flowtools-file-store-{}", uuid::Uuid::new_v4()));
        let directory = root.join("selected");
        let profile = root.join("profile");
        fs::create_dir_all(&directory).unwrap();
        fs::create_dir(&profile).unwrap();
        fs::write(
            directory.join("private-name-canary.txt"),
            b"private-content-canary",
        )
        .unwrap();
        Self {
            root,
            directory,
            profile,
        }
    }
    fn open(&self) -> DataStore {
        DataStore::open(&self.profile.join("runtime.sqlite")).unwrap()
    }
    fn draft(&self, prefix: &str) -> RenameDraft {
        prepare_rename(&self.directory, prefix, &self.profile).unwrap()
    }
    fn create(&self, store: &mut DataStore) -> FilePlanRecord {
        FilePlanStore::persist(
            store,
            &uuid::Uuid::new_v4().to_string(),
            self.draft("整理_"),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(self.root.parent().unwrap(), std::env::temp_dir());
        assert!(self
            .root
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("flowtools-file-store-"));
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn value(store: &DataStore) -> String {
    store
        .connection
        .query_row(
            "SELECT value FROM core_metadata WHERE key=?1",
            [KEY],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn plans_survive_restart_with_private_definitions_and_no_file_effects() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let record = fixture.create(&mut store);
    let metadata = value(&store);
    for canary in [
        "private-name-canary",
        "private-content-canary",
        "selected",
        "整理_",
    ] {
        assert!(!metadata.contains(canary));
        for entry in fs::read_dir(fixture.profile.join("private-jobs")).unwrap() {
            assert!(
                !String::from_utf8_lossy(&fs::read(entry.unwrap().path()).unwrap())
                    .contains(canary)
            );
        }
    }
    let ids = FilePlanStore::retained_input_ids(&store).unwrap();
    private(&store).unwrap().remove_orphans(&ids).unwrap();
    drop(store);
    let store = fixture.open();
    assert_eq!(
        FilePlanStore::list(&store).unwrap().as_slice(),
        std::slice::from_ref(&record)
    );
    let preview = FilePlanStore::preview(&store, &record.id, 1).unwrap();
    assert_eq!(preview.digest, record.digest);
    assert_eq!(preview.files[0].target, "整理_private-name-canary.txt");
    assert!(FilePlanStore::recheck(&store, &record.id, 1).is_ok());
    assert!(fixture.directory.join("private-name-canary.txt").is_file());
    assert!(!fixture
        .directory
        .join("整理_private-name-canary.txt")
        .exists());
}

#[test]
fn stable_key_never_replaces_a_plan_or_rearms_a_cancelled_plan() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let record = fixture.create(&mut store);
    assert_eq!(
        FilePlanStore::persist(&mut store, &record.id, fixture.draft("整理_")).unwrap(),
        record
    );
    assert_eq!(
        FilePlanStore::persist(&mut store, &record.id, fixture.draft("other_")).unwrap_err(),
        ErrorCode::IdempotencyConflict
    );
    assert_eq!(
        FilePlanStore::persist(&mut store, "../escape", fixture.draft("整理_")).unwrap_err(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        FilePlanStore::cancel(&mut store, &record.id, 2).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    let cancelled = FilePlanStore::cancel(&mut store, &record.id, 1).unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.revision, 2);
    assert_eq!(
        FilePlanStore::persist(&mut store, &record.id, fixture.draft("整理_")).unwrap(),
        cancelled
    );
    assert_eq!(
        FilePlanStore::preview(&store, &record.id, 2).unwrap_err(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        FilePlanStore::cancel(&mut store, &record.id, 1).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    drop(store);
    assert_eq!(FilePlanStore::list(&fixture.open()).unwrap(), [cancelled]);
}

#[test]
fn changed_bytes_preserve_the_reviewed_plan_and_fail_fresh_observation() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let record = fixture.create(&mut store);
    let before = value(&store);
    fs::write(
        fixture.directory.join("private-name-canary.txt"),
        b"modified",
    )
    .unwrap();
    assert_eq!(
        FilePlanStore::preview(&store, &record.id, 1)
            .unwrap()
            .total_bytes,
        22
    );
    assert_eq!(
        FilePlanStore::recheck(&store, &record.id, 1).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    assert_eq!(value(&store), before);
    assert_eq!(
        fs::read(fixture.directory.join("private-name-canary.txt")).unwrap(),
        b"modified"
    );
}

#[test]
fn copy_and_restore_preserve_inputs_but_need_exact_disposal_challenges() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let record = fixture.create(&mut store);
    let ids = FilePlanStore::retained_input_ids(&store).unwrap();
    drop(store);
    let copied_path = fixture.profile.join("copied.sqlite");
    fs::copy(fixture.profile.join("runtime.sqlite"), &copied_path).unwrap();
    let mut copy = DataStore::open(&copied_path).unwrap();
    assert_eq!(
        FilePlanStore::list(&copy).unwrap_err(),
        ErrorCode::RecoveryPending
    );
    assert_eq!(FilePlanStore::retained_input_ids(&copy).unwrap(), ids);
    private(&copy).unwrap().remove_orphans(&ids).unwrap();
    let challenge = FilePlanStore::recovery(&copy).unwrap().unwrap().challenge;
    assert_eq!(
        FilePlanStore::discard_restored(&mut copy, &"0".repeat(64)).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    copy.connection
        .execute(
            "INSERT INTO core_metadata(key,value) VALUES('recovery-generation',?1)",
            [uuid::Uuid::new_v4().to_string()],
        )
        .unwrap();
    assert_eq!(
        FilePlanStore::discard_restored(&mut copy, &challenge).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    let fresh = FilePlanStore::recovery(&copy).unwrap().unwrap();
    let disposed = FilePlanStore::discard_restored(&mut copy, &fresh.challenge).unwrap();
    assert!(disposed[0].cancelled);
    assert_eq!(disposed[0].id, record.id);
    assert_eq!(FilePlanStore::retained_input_ids(&copy).unwrap(), ids);
    assert_eq!(
        FilePlanStore::preview(&copy, &record.id, 2).unwrap_err(),
        ErrorCode::InvalidRequest
    );
    assert!(FilePlanStore::recovery(&copy).unwrap().is_none());
}

#[test]
fn corrupt_state_or_private_bytes_never_reset_or_authorize_cleanup() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let record = fixture.create(&mut store);
    let bytes = value(&store);
    let input = FilePlanStore::retained_input_ids(&store)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let payload = fixture
        .profile
        .join("private-jobs")
        .join(format!("{input}.input.bin"));
    let original = fs::read(&payload).unwrap();
    fs::write(&payload, b"corrupt DPAPI").unwrap();
    assert!(FilePlanStore::preview(&store, &record.id, 1).is_err());
    assert_eq!(value(&store), bytes);
    fs::write(&payload, &original).unwrap();
    let mut bad: serde_json::Value = serde_json::from_str(&bytes).unwrap();
    bad["entries"][0]["inputId"] = serde_json::json!("../escape");
    let bad = bad.to_string();
    store
        .connection
        .execute(
            "UPDATE core_metadata SET value=?1 WHERE key=?2",
            rusqlite::params![bad, KEY],
        )
        .unwrap();
    assert_eq!(
        FilePlanStore::retained_input_ids(&store).unwrap_err(),
        ErrorCode::StoreCorrupt
    );
    assert_eq!(
        FilePlanStore::list(&store).unwrap_err(),
        ErrorCode::StoreCorrupt
    );
    assert_eq!(value(&store), bad);
    assert_eq!(fs::read(&payload).unwrap(), original);
}

#[test]
fn failed_sql_commit_publishes_nothing_and_retry_keeps_one_plan() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let key = uuid::Uuid::new_v4().to_string();
    store
        .connection
        .execute_batch("PRAGMA query_only=ON")
        .unwrap();
    assert_eq!(
        FilePlanStore::persist(&mut store, &key, fixture.draft("整理_")).unwrap_err(),
        ErrorCode::StorageFailed
    );
    assert!(FilePlanStore::list(&store).unwrap().is_empty());
    store
        .connection
        .execute_batch("PRAGMA query_only=OFF")
        .unwrap();
    let record = FilePlanStore::persist(&mut store, &key, fixture.draft("整理_")).unwrap();
    let before = value(&store);
    store
        .connection
        .execute_batch("PRAGMA query_only=ON")
        .unwrap();
    assert_eq!(
        FilePlanStore::cancel(&mut store, &key, 1).unwrap_err(),
        ErrorCode::StorageFailed
    );
    assert_eq!(value(&store), before);
    assert_eq!(FilePlanStore::list(&store).unwrap(), [record]);
    store
        .connection
        .execute_batch("PRAGMA query_only=OFF")
        .unwrap();
    private(&store)
        .unwrap()
        .remove_orphans(&FilePlanStore::retained_input_ids(&store).unwrap())
        .unwrap();
    assert_eq!(
        fs::read_dir(fixture.profile.join("private-jobs"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn plan_quota_and_duplicate_metadata_are_fatal_without_eviction() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    for _ in 0..MAX_PLANS {
        fixture.create(&mut store);
    }
    let before = value(&store);
    assert_eq!(
        FilePlanStore::persist(
            &mut store,
            &uuid::Uuid::new_v4().to_string(),
            fixture.draft("整理_")
        )
        .unwrap_err(),
        ErrorCode::BudgetExceeded
    );
    assert_eq!(value(&store), before);
    let mut state: serde_json::Value = serde_json::from_str(&before).unwrap();
    state["entries"][1] = state["entries"][0].clone();
    store
        .connection
        .execute(
            "UPDATE core_metadata SET value=?1 WHERE key=?2",
            rusqlite::params![state.to_string(), KEY],
        )
        .unwrap();
    assert_eq!(
        FilePlanStore::retained_input_ids(&store).unwrap_err(),
        ErrorCode::StoreCorrupt
    );
    assert_eq!(
        fs::read_dir(fixture.profile.join("private-jobs"))
            .unwrap()
            .count(),
        MAX_PLANS
    );
}

#[test]
fn real_host_start_retains_plan_inputs_and_refuses_corrupt_cleanup() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let record = fixture.create(&mut store);
    let ids = FilePlanStore::retained_input_ids(&store).unwrap();
    let core =
        crate::runtime::RuntimeCore::managed(Default::default(), store, false, false).unwrap();
    drop(core);
    let store = fixture.open();
    assert_eq!(
        FilePlanStore::preview(&store, &record.id, 1)
            .unwrap()
            .digest,
        record.digest
    );
    store
        .connection
        .execute(
            "INSERT INTO core_metadata(key,value) VALUES('recovery-generation',?1)",
            [uuid::Uuid::new_v4().to_string()],
        )
        .unwrap();
    let core =
        crate::runtime::RuntimeCore::managed(Default::default(), store, false, false).unwrap();
    drop(core);
    let store = fixture.open();
    assert_eq!(
        FilePlanStore::list(&store).unwrap_err(),
        ErrorCode::RecoveryPending
    );
    assert_eq!(FilePlanStore::retained_input_ids(&store).unwrap(), ids);
    for id in &ids {
        assert!(private(&store).unwrap().read(id, "input").is_ok());
    }
    store
        .connection
        .execute(
            "UPDATE core_metadata SET value='corrupt' WHERE key=?1",
            [KEY],
        )
        .unwrap();
    assert_eq!(
        crate::runtime::RuntimeCore::managed(Default::default(), store, false, false)
            .err()
            .unwrap(),
        ErrorCode::StoreCorrupt
    );
    assert_eq!(
        fs::read_dir(fixture.profile.join("private-jobs"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn published_plan_survives_actual_process_exit_without_destructors() {
    let fixture = Fixture::new();
    let id = uuid::Uuid::new_v4().to_string();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "file_workflows::store::tests::plan_process_exit_child",
        ])
        .env("FLOWTOOLS_FILE_PLAN_EXIT_ROOT", &fixture.root)
        .env("FLOWTOOLS_FILE_PLAN_EXIT_ID", &id)
        .spawn()
        .unwrap();
    let mut child = child;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("file-plan exit fixture exceeded its bound");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(status.code(), Some(71));
    let store = fixture.open();
    let records = FilePlanStore::list(&store).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, id);
    assert_eq!(
        FilePlanStore::recheck(&store, &id, 1).unwrap().files.len(),
        1
    );
    assert!(!fixture
        .directory
        .join("整理_private-name-canary.txt")
        .exists());
}

#[test]
fn plan_process_exit_child() {
    let Some(root) = std::env::var_os("FLOWTOOLS_FILE_PLAN_EXIT_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    assert_eq!(root.parent().unwrap(), std::env::temp_dir());
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("flowtools-file-store-"));
    let id = std::env::var("FLOWTOOLS_FILE_PLAN_EXIT_ID").unwrap();
    let profile = root.join("profile");
    let mut store = DataStore::open(&profile.join("runtime.sqlite")).unwrap();
    let draft = prepare_rename(&root.join("selected"), "整理_", &profile).unwrap();
    FilePlanStore::persist(&mut store, &id, draft).unwrap();
    std::process::exit(71);
}
