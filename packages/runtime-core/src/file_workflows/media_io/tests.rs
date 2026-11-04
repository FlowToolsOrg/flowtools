use super::*;
use std::fs;
struct Fixture {
    root: std::path::PathBuf,
    profile: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("flowtools-media-io-{}", uuid::Uuid::new_v4()));
        let profile = root.join("profile");
        fs::create_dir_all(&profile).unwrap();
        fs::write(root.join("source.wav"), b"original").unwrap();
        Self { root, profile }
    }
    fn store(&self) -> DataStore {
        DataStore::open(&self.profile.join("runtime.sqlite")).unwrap()
    }
    fn plan(&self, s: &mut DataStore) -> MediaPlanPreview {
        create(
            s,
            CreateMediaPlan {
                request_key: uuid::Uuid::new_v4().to_string(),
                input_path: self.root.join("source.wav").to_str().unwrap().into(),
                output_path: Some(self.root.join("output.wav").to_str().unwrap().into()),
                action: UserMediaAction::Audio,
            },
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
            .starts_with("flowtools-media-io-"));
        fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn disk_full_write_failure_keeps_pending_intent_and_never_publishes_or_replays() {
    let f = Fixture::new();
    let mut s = f.store();
    let p = f.plan(&mut s);
    let run = uuid::Uuid::new_v4().to_string();
    bind(&mut s, &p.id, &run).unwrap();
    windows::fail_next_publication_write(112); // ERROR_DISK_FULL at the actual staging writer.
    assert_eq!(
        publish(&mut s, &p.id, &run, b"converted", |_| Ok(())).unwrap_err(),
        ErrorCode::StorageFailed
    );
    assert!(!f.root.join("output.wav").exists());
    let raw = store::bounded_text(&s.connection, KEY, LIMIT)
        .unwrap()
        .unwrap();
    assert!(!raw.contains("source.wav"));
    assert!(!raw.contains("output.wav"));
    let state = load(&s, false).unwrap();
    assert!(!state.entries[0].publication.as_ref().unwrap().complete);
    assert_eq!(
        publish(&mut s, &p.id, &run, b"converted", |_| Ok(())).unwrap_err(),
        ErrorCode::AcceptanceUnknown
    );
    let retained = retained_input_ids(&s).unwrap();
    private(&s).unwrap().remove_orphans(&retained).unwrap();
    drop(s);
    let s = f.store();
    assert_eq!(get(&s, &p.id).unwrap().digest, p.digest);
    assert!(!f.root.join("output.wav").exists());
}
#[test]
fn revocation_at_publication_and_sql_substitution_preserve_bytes_without_output() {
    let f = Fixture::new();
    let mut s = f.store();
    let p = f.plan(&mut s);
    let run = uuid::Uuid::new_v4().to_string();
    bind(&mut s, &p.id, &run).unwrap();
    assert_eq!(
        publish(&mut s, &p.id, &run, b"converted", |_| Err(
            ErrorCode::GrantRevoked
        ))
        .unwrap_err(),
        ErrorCode::GrantRevoked
    );
    assert!(!f.root.join("output.wav").exists());
    assert_eq!(fs::read(f.root.join("source.wav")).unwrap(), b"original");
    let p2 = create(
        &mut s,
        CreateMediaPlan {
            request_key: uuid::Uuid::new_v4().to_string(),
            input_path: f.root.join("source.wav").to_str().unwrap().into(),
            output_path: None,
            action: UserMediaAction::Inspect,
        },
    )
    .unwrap();
    s.connection.execute_batch("CREATE TRIGGER substitute_media AFTER UPDATE ON core_metadata WHEN NEW.key='user-media-plans-v1' BEGIN UPDATE core_metadata SET value='{}' WHERE key=NEW.key; END;").unwrap();
    assert_eq!(
        bind(&mut s, &p2.id, &uuid::Uuid::new_v4().to_string()).unwrap_err(),
        ErrorCode::StoreCorrupt
    );
    assert!(bound_run(&s, &p2.id).unwrap().is_none());
    s.connection
        .execute_batch("DROP TRIGGER substitute_media")
        .unwrap();
    s.connection
        .execute(
            "INSERT INTO core_metadata(key,value) VALUES('recovery-generation','restored')",
            [],
        )
        .unwrap();
    assert_eq!(get(&s, &p.id).unwrap_err(), ErrorCode::RecoveryPending);
    assert_eq!(retained_input_ids(&s).unwrap().len(), 2);
}
