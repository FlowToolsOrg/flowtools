//! Raw signed ZIP transaction trigger regressions, without source execution.
use super::*;

#[test]
fn metadata_commit_trigger_cannot_replace_prepared_journal() {
    let mut world = World::new();
    let bytes = world.fixture.base.archive.clone();
    let mut original = None;
    let result = world.with_hook(|phase, store| {
        if phase == ToolInstallPhase::BeforeCommit {
            original = Some(store.connection.query_row(
                "SELECT value FROM core_metadata WHERE key=?1", [PENDING],
                |r| r.get::<_, String>(0),
            ).unwrap());
            store.connection.execute_batch(&format!(
                "CREATE TRIGGER replace_prepared AFTER INSERT ON core_metadata WHEN NEW.key='{KEY}' BEGIN UPDATE core_metadata SET value='REPLACED' WHERE key='{PENDING}'; END;"
            )).unwrap();
        }
        Ok(())
    });
    assert!(
        result.is_err(),
        "a replaced journal must not be erased by commit"
    );
    assert_eq!(world.metadata(PENDING), original);
    assert!(world.metadata(KEY).is_none());
    assert_eq!(fs::read(world.blob().join("package.zip")).unwrap(), bytes);
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute_batch("DROP TRIGGER replace_prepared")
        .unwrap();
    assert!(world.abort().unwrap());
    assert!(world.pins().unwrap().is_empty());
}

#[test]
fn metadata_clear_trigger_cannot_replace_then_erase_journal() {
    for aborting in [false, true] {
        for timing in ["BEFORE", "AFTER"] {
            let mut world = World::new();
            let bytes = world.fixture.base.archive.clone();
            let mut original = None;
            let result = world.with_hook(|phase, store| {
                if phase == ToolInstallPhase::BeforeCommit {
                    original = Some(
                        store
                            .connection
                            .query_row(
                                "SELECT value FROM core_metadata WHERE key=?1",
                                [PENDING],
                                |r| r.get::<_, String>(0),
                            )
                            .unwrap(),
                    );
                    if aborting {
                        return Err(InstallError::Storage.into());
                    }
                    install_delete_trigger(store, timing);
                }
                Ok(())
            });
            assert!(result.is_err(), "{timing}, abort={aborting}");
            if aborting {
                install_delete_trigger(world.store.as_ref().unwrap(), timing);
                assert!(
                    world.abort().is_err(),
                    "{timing} abort must retain the journal"
                );
            }
            assert_eq!(
                world.metadata(PENDING),
                original,
                "{timing}, abort={aborting}"
            );
            assert!(world.metadata(KEY).is_none());
            assert_ne!(
                world.metadata("package-trust-state-v1").as_deref(),
                Some("REPLACED")
            );
            world
                .store
                .as_ref()
                .unwrap()
                .connection
                .execute_batch("DROP TRIGGER replace_on_delete")
                .unwrap();
            assert!(world.abort().unwrap());
            assert!(world.metadata(PENDING).is_none());
            assert!(world.pins().unwrap().is_empty());
            let retained = fs::read_dir(world.root().join("quarantine"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            assert_eq!(fs::read(retained.join("package.zip")).unwrap(), bytes);
        }
    }
}
fn install_delete_trigger(store: &DataStore, timing: &str) {
    let change = if timing == "BEFORE" {
        format!("UPDATE core_metadata SET value='REPLACED' WHERE key='{PENDING}'")
    } else {
        "UPDATE core_metadata SET value='REPLACED' WHERE key='package-trust-state-v1'".into()
    };
    store.connection.execute_batch(&format!(
        "CREATE TRIGGER replace_on_delete {timing} DELETE ON core_metadata WHEN OLD.key='{PENDING}' BEGIN {change}; END;"
    )).unwrap();
}
