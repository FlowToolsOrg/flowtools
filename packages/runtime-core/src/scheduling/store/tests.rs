use super::*;
use crate::{catalog::BuiltinCatalog, data::DataStore};

fn binding() -> ScheduleBinding {
    let catalog = BuiltinCatalog::embedded();
    let (manifest, command) = catalog.command("plugin-base64-encoder", "run").unwrap();
    ScheduleBinding {
        identity: CommandIdentity::from_manifest("local-cli", manifest, command),
        grant_epoch: 1,
        input_digest: digest(&serde_json::json!({"value":"private"})),
        timeout_ms: 1_000,
    }
}
fn definition() -> ScheduleDefinition {
    ScheduleDefinition {
        first_due_ms: 100_000,
        interval_ms: Some(1_000),
        occurrences: 10,
        grace_ms: 500,
        missed: MissedRunPolicy::CoalesceLatest,
    }
}
fn create(store: &mut DataStore) -> ScheduleRecord {
    ScheduleStore::create(
        store,
        uuid::Uuid::new_v4().to_string(),
        definition(),
        binding(),
        99_000,
    )
    .unwrap()
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
fn replacement_is_one_commit_and_stale_edits_leave_both_positions_unchanged() {
    let mut store = DataStore::memory().unwrap();
    let old = create(&mut store);
    let key = super::super::commands::ScheduleKey {
        id: old.id.clone(),
        revision: old.revision,
    };
    let replacement = uuid::Uuid::new_v4().to_string();
    store
        .connection
        .execute_batch("PRAGMA query_only=ON")
        .unwrap();
    assert!(ScheduleStore::replace(
        &mut store,
        &key,
        replacement.clone(),
        definition(),
        binding(),
        99_000
    )
    .is_err());
    store
        .connection
        .execute_batch("PRAGMA query_only=OFF")
        .unwrap();
    assert!(!ScheduleStore::list(&store).unwrap()[0].cancelled);
    let new = ScheduleStore::replace(
        &mut store,
        &key,
        replacement,
        definition(),
        binding(),
        99_000,
    )
    .unwrap();
    let records = ScheduleStore::list(&store).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records[0].cancelled);
    assert_eq!(records[0].revision, 2);
    assert_eq!(new.next_index, 0);
    assert_eq!(
        ScheduleStore::replace(
            &mut store,
            &key,
            uuid::Uuid::new_v4().to_string(),
            definition(),
            binding(),
            99_000
        )
        .unwrap_err(),
        ErrorCode::RevisionConflict
    );
    assert_eq!(
        ScheduleStore::claim_due(&mut store, 100_000, |_| true).unwrap()[0].id,
        new.id
    );
}

#[test]
fn explicit_restore_disposal_keeps_claims_positions_and_floors_and_is_not_rearming() {
    let mut store = DataStore::memory().unwrap();
    create(&mut store);
    ScheduleStore::claim_due(&mut store, 102_000, |_| true).unwrap();
    let before = value(&store);
    store
        .connection
        .execute(
            "INSERT INTO core_metadata(key,value) VALUES('recovery-generation',?1)",
            [uuid::Uuid::new_v4().to_string()],
        )
        .unwrap();
    assert_eq!(
        ScheduleStore::list(&store).unwrap_err(),
        ErrorCode::RecoveryPending
    );
    let recovery = ScheduleStore::recovery(&store).unwrap().unwrap();
    assert_eq!(recovery.count, 1);
    assert_eq!(
        ScheduleStore::discard_restored(&mut store, "wrong").unwrap_err(),
        ErrorCode::RevisionConflict
    );
    assert_eq!(value(&store), before);
    let records = ScheduleStore::discard_restored(&mut store, &recovery.challenge).unwrap();
    assert!(records[0].cancelled);
    assert_eq!(records[0].next_index, 3);
    assert_eq!(
        records[0].last_claim.as_ref().unwrap().result,
        ClaimResult::Interrupted
    );
    assert!(ScheduleStore::recovery(&store).unwrap().is_none());
    assert!(ScheduleStore::claim_due(&mut store, 109_000, |_| true)
        .unwrap()
        .is_empty());
    assert_eq!(
        ScheduleStore::create(
            &mut store,
            uuid::Uuid::new_v4().to_string(),
            definition(),
            binding(),
            101_999
        )
        .unwrap_err(),
        ErrorCode::InvalidRequest
    );
    let fresh = create(&mut DataStore::memory().unwrap());
    ScheduleStore::create(&mut store, fresh.id, definition(), binding(), 102_000).unwrap();
    let retry = ScheduleStore::discard_restored(&mut store, &recovery.challenge).unwrap();
    assert!(!retry[1].cancelled);
    store
        .connection
        .execute(
            "UPDATE core_metadata SET value=?1 WHERE key='recovery-generation'",
            [uuid::Uuid::new_v4().to_string()],
        )
        .unwrap();
    assert_eq!(
        ScheduleStore::discard_restored(&mut store, &recovery.challenge).unwrap_err(),
        ErrorCode::RevisionConflict
    );
}

#[test]
fn claimed_position_survives_reopen_and_unknown_attempt_is_not_replayed() {
    let root = std::env::temp_dir().join(format!("flowtools-schedule-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("runtime.sqlite");
    let mut store = DataStore::open(&path).unwrap();
    let record = create(&mut store);
    let claimed = ScheduleStore::claim_due(&mut store, 100_000, |_| true).unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].last_claim.as_ref().unwrap().index, 0);
    drop(store);
    let mut store = DataStore::open(&path).unwrap();
    assert!(ScheduleStore::claim_due(&mut store, 100_000, |_| true)
        .unwrap()
        .is_empty());
    ScheduleStore::recover(&mut store).unwrap();
    let restored = ScheduleStore::list(&store).unwrap();
    assert_eq!(restored[0].id, record.id);
    assert_eq!(
        restored[0].last_claim.as_ref().unwrap().result,
        ClaimResult::Interrupted
    );
    assert!(ScheduleStore::claim_due(&mut store, 100_000, |_| true)
        .unwrap()
        .is_empty());
    assert_eq!(
        ScheduleStore::claim_due(&mut store, 101_000, |_| true).unwrap()[0].next_index,
        2
    );
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn completion_requires_the_exact_claim_and_is_idempotent() {
    let mut store = DataStore::memory().unwrap();
    let record = create(&mut store);
    let claims = ScheduleStore::claim_due(&mut store, 105_000, |_| true).unwrap();
    let token = &claims[0].last_claim.as_ref().unwrap().token;
    assert_eq!(claims[0].skipped, 5);
    let before = value(&store);
    assert_eq!(
        ScheduleStore::finish(&mut store, &record.id, "wrong", ClaimResult::Interrupted),
        Err(ErrorCode::RevisionConflict)
    );
    assert_eq!(value(&store), before);
    let result = ClaimResult::Submitted {
        run_id: uuid::Uuid::new_v4().to_string(),
    };
    ScheduleStore::finish(&mut store, &record.id, token, result.clone()).unwrap();
    ScheduleStore::finish(&mut store, &record.id, token, result).unwrap();
    assert_eq!(
        ScheduleStore::finish(&mut store, &record.id, token, ClaimResult::Interrupted),
        Err(ErrorCode::RevisionConflict)
    );
}

#[test]
fn cancellation_and_revision_conflicts_cannot_rearm_a_schedule() {
    let mut store = DataStore::memory().unwrap();
    let record = create(&mut store);
    let before = value(&store);
    assert_eq!(
        ScheduleStore::cancel(&mut store, &record.id, 2).err(),
        Some(ErrorCode::RevisionConflict)
    );
    assert_eq!(value(&store), before);
    let cancelled = ScheduleStore::cancel(&mut store, &record.id, 1).unwrap();
    assert_eq!(cancelled.revision, 2);
    ScheduleStore::cancel(&mut store, &record.id, 2).unwrap();
    ScheduleStore::recover(&mut store).unwrap();
    assert!(ScheduleStore::claim_due(&mut store, 110_000, |_| true)
        .unwrap()
        .is_empty());
}

#[test]
fn rollback_clock_and_rejected_payloads_preserve_exact_metadata() {
    let mut store = DataStore::memory().unwrap();
    create(&mut store);
    ScheduleStore::claim_due(&mut store, 100_000, |_| true).unwrap();
    let before = value(&store);
    assert_eq!(
        ScheduleStore::claim_due(&mut store, 99_999, |_| true).err(),
        Some(ErrorCode::InvalidRequest)
    );
    assert_eq!(value(&store), before);
    let mut invalid = definition();
    invalid.occurrences = 0;
    assert!(ScheduleStore::create(
        &mut store,
        uuid::Uuid::new_v4().to_string(),
        invalid,
        binding(),
        100_000
    )
    .is_err());
    assert_eq!(value(&store), before);
    assert!(!before.contains("private"));
}

#[test]
fn restore_generation_and_copied_database_cannot_replay_claims() {
    let root =
        std::env::temp_dir().join(format!("flowtools-schedule-copy-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let original = root.join("original.sqlite");
    let mut store = DataStore::open(&original).unwrap();
    create(&mut store);
    drop(store);
    let copy = root.join("copied.sqlite");
    std::fs::copy(&original, &copy).unwrap();
    let mut copied = DataStore::open(&copy).unwrap();
    assert_eq!(
        ScheduleStore::claim_due(&mut copied, 100_000, |_| true).err(),
        Some(ErrorCode::RecoveryPending)
    );
    drop(copied);
    let mut store = DataStore::open(&original).unwrap();
    store
        .connection
        .execute(
            "INSERT INTO core_metadata(key,value) VALUES('recovery-generation','new-restore')",
            [],
        )
        .unwrap();
    let before = value(&store);
    assert_eq!(
        ScheduleStore::claim_due(&mut store, 100_000, |_| true).err(),
        Some(ErrorCode::RecoveryPending)
    );
    assert_eq!(value(&store), before);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_sql_write_returns_no_claim_and_does_not_consume_a_position() {
    let mut store = DataStore::memory().unwrap();
    create(&mut store);
    let before = value(&store);
    store
        .connection
        .execute_batch("PRAGMA query_only=ON")
        .unwrap();
    assert_eq!(
        ScheduleStore::claim_due(&mut store, 100_000, |_| true).err(),
        Some(ErrorCode::StorageFailed)
    );
    assert_eq!(value(&store), before);
    store
        .connection
        .execute_batch("PRAGMA query_only=OFF")
        .unwrap();
    assert_eq!(
        ScheduleStore::claim_due(&mut store, 100_000, |_| true).unwrap()[0].next_index,
        1
    );
}

#[test]
fn corrupt_state_is_preserved_and_never_silently_reinitialized() {
    let mut store = DataStore::memory().unwrap();
    create(&mut store);
    for text in ["{}", "{\"formatVersion\":2}", "not-json"] {
        store
            .connection
            .execute(
                "UPDATE core_metadata SET value=?1 WHERE key=?2",
                rusqlite::params![text, KEY],
            )
            .unwrap();
        assert_eq!(
            ScheduleStore::recover(&mut store),
            Err(ErrorCode::StoreCorrupt)
        );
        assert_eq!(value(&store), text);
    }
}

#[test]
fn host_overlap_filter_leaves_ineligible_positions_unconsumed() {
    let mut store = DataStore::memory().unwrap();
    create(&mut store);
    assert!(ScheduleStore::claim_due(&mut store, 105_000, |_| false)
        .unwrap()
        .is_empty());
    assert_eq!(ScheduleStore::list(&store).unwrap()[0].next_index, 0);
    let claims = ScheduleStore::claim_due(&mut store, 105_000, |_| true).unwrap();
    assert_eq!(claims[0].next_index, 6);
    assert_eq!(claims[0].skipped, 5);
}
