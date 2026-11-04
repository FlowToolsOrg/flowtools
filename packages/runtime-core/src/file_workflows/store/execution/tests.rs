use super::*;
use crate::file_workflows::store::tests::Fixture;
use std::fs;

#[test]
fn maximum_batch_can_commit_and_undo_within_original_private_payload_quota() {
    let f = Fixture::new();
    let mut s = f.open();
    for n in 1..128 {
        fs::write(f.directory.join(format!("file-{n}.txt")), b"data").unwrap();
    }
    let p = f.create(&mut s);
    assert_eq!(p.files, 128);
    assert_eq!(
        execute(&mut s, &p.id, 1, &p.digest, false, || Ok(()))
            .unwrap()
            .completed,
        128
    );
    assert_eq!(
        execute(&mut s, &p.id, 1, &p.digest, true, || Ok(()))
            .unwrap()
            .completed,
        0
    );
    assert_eq!(fs::read_dir(f.directory.as_path()).unwrap().count(), 128);
    assert_eq!(
        fs::read_dir(f.profile.join("private-jobs"))
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn commit_restart_retry_and_undo_preserve_identity_and_bytes() {
    let f = Fixture::new();
    let mut s = f.open();
    let p = f.create(&mut s);
    let done = execute(&mut s, &p.id, 1, &p.digest, false, || Ok(())).unwrap();
    assert_eq!(done.phase, FileExecutionPhase::Complete);
    assert!(!f.directory.join("private-name-canary.txt").exists());
    assert_eq!(
        fs::read(f.directory.join("整理_private-name-canary.txt")).unwrap(),
        b"private-content-canary"
    );
    drop(s);
    let mut s = f.open();
    assert_eq!(
        execute(&mut s, &p.id, 1, &p.digest, false, || Ok(()))
            .unwrap()
            .completed,
        1
    );
    let undone = execute(&mut s, &p.id, 1, &p.digest, true, || Ok(())).unwrap();
    assert_eq!(undone.phase, FileExecutionPhase::Undone);
    assert_eq!(
        execute(&mut s, &p.id, 1, &p.digest, true, || Ok(()))
            .unwrap()
            .completed,
        0
    );
    assert_eq!(
        fs::read(f.directory.join("private-name-canary.txt")).unwrap(),
        b"private-content-canary"
    );
    assert!(!f.directory.join("整理_private-name-canary.txt").exists());
}

#[test]
fn exact_consent_collision_and_modified_undo_are_refused() {
    let f = Fixture::new();
    let mut s = f.open();
    let p = f.create(&mut s);
    assert_eq!(
        execute(&mut s, &p.id, 1, &"0".repeat(64), false, || Ok(())).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    assert_eq!(
        execute(&mut s, &p.id, 1, &p.digest, false, || Err(
            ErrorCode::ApprovalRequired
        ))
        .unwrap_err(),
        ErrorCode::ApprovalRequired
    );
    fs::write(f.directory.join("整理_private-name-canary.txt"), b"foreign").unwrap();
    assert!(execute(&mut s, &p.id, 1, &p.digest, false, || Ok(())).is_err());
    assert_eq!(
        fs::read(f.directory.join("整理_private-name-canary.txt")).unwrap(),
        b"foreign"
    );
    fs::remove_file(f.directory.join("整理_private-name-canary.txt")).unwrap();
    execute(&mut s, &p.id, 1, &p.digest, false, || Ok(())).unwrap();
    fs::write(f.directory.join("整理_private-name-canary.txt"), b"edited").unwrap();
    assert_eq!(
        execute(&mut s, &p.id, 1, &p.digest, true, || Ok(())).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    assert!(!f.directory.join("private-name-canary.txt").exists());
}

#[test]
fn unknown_post_effect_write_is_reconciled_without_repeating_rename() {
    let f = Fixture::new();
    let mut s = f.open();
    let p = f.create(&mut s);
    s.connection.execute_batch("CREATE TRIGGER refuse_receipt BEFORE UPDATE ON core_metadata WHEN NEW.key='file-executions-v1' AND json_extract(NEW.value,'$.entries[0].record.completed')=1 BEGIN SELECT RAISE(ABORT,'receipt'); END;").unwrap();
    assert!(execute(&mut s, &p.id, 1, &p.digest, false, || Ok(())).is_err());
    assert!(f.directory.join("整理_private-name-canary.txt").exists());
    assert!(records(&s).unwrap()[0].pending);
    s.connection
        .execute_batch("DROP TRIGGER refuse_receipt")
        .unwrap();
    drop(s);
    let mut s = f.open();
    assert_eq!(
        execute(&mut s, &p.id, 1, &p.digest, false, || Ok(()))
            .unwrap()
            .phase,
        FileExecutionPhase::Complete
    );
    assert_eq!(
        FilePlanStore::cancel(&mut s, &p.id, 1).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    let retained = FilePlanStore::retained_input_ids(&s).unwrap();
    private(&s).unwrap().remove_orphans(&retained).unwrap();
    execute(&mut s, &p.id, 1, &p.digest, true, || Ok(())).unwrap();
}

#[test]
fn journal_trigger_and_copied_profile_cannot_authorize_effects() {
    let f = Fixture::new();
    let mut s = f.open();
    let p = f.create(&mut s);
    s.connection.execute_batch("CREATE TRIGGER substitute_journal AFTER INSERT ON core_metadata WHEN NEW.key='file-executions-v1' BEGIN UPDATE core_metadata SET value='{}' WHERE key=NEW.key; END;").unwrap();
    assert_eq!(
        execute(&mut s, &p.id, 1, &p.digest, false, || Ok(())).unwrap_err(),
        ErrorCode::StoreCorrupt
    );
    assert!(f.directory.join("private-name-canary.txt").exists());
    s.connection
        .execute_batch("DROP TRIGGER substitute_journal")
        .unwrap();
    execute(&mut s, &p.id, 1, &p.digest, false, || Ok(())).unwrap();
    s.connection
        .execute(
            "INSERT INTO core_metadata(key,value) VALUES('recovery-generation','new-generation')",
            [],
        )
        .unwrap();
    assert_eq!(
        execute(&mut s, &p.id, 1, &p.digest, true, || Ok(())).unwrap_err(),
        ErrorCode::RecoveryPending
    );
    assert_eq!(FilePlanStore::retained_input_ids(&s).unwrap().len(), 2);
    let challenge = FilePlanStore::recovery(&s).unwrap().unwrap().challenge;
    FilePlanStore::discard_restored(&mut s, &challenge).unwrap();
    assert!(f.directory.join("整理_private-name-canary.txt").exists());
    assert!(execute(&mut s, &p.id, 2, &p.digest, true, || Ok(())).is_err());
    let next =
        FilePlanStore::persist(&mut s, &uuid::Uuid::new_v4().to_string(), f.draft("新_")).unwrap();
    execute(&mut s, &next.id, 1, &next.digest, false, || Ok(())).unwrap();
    assert!(f.directory.join("新_整理_private-name-canary.txt").exists());
}
