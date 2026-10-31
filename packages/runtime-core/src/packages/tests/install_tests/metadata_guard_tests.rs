//! Real SQLite trigger effects at the legacy v2 transaction boundary.
use super::*;

#[test]
fn metadata_commit_trigger_cannot_replace_prepared_journal() {
    let mut world = World::new();
    let input = world.input(manifest("consumer", vec![]), 1);
    let at = world.fixture.policy.clock.now;
    let mut original = None;
    let result = install_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        input.borrow(),
        &world.compatibility,
        || at,
        |phase, store| {
            if phase == InstallPhase::BeforeCommit {
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
                store.connection.execute_batch(&format!(
                    "CREATE TRIGGER replace_prepared AFTER INSERT ON core_metadata WHEN NEW.key='{INVENTORY}' BEGIN UPDATE core_metadata SET value='REPLACED' WHERE key='{PENDING}'; END;"
                )).unwrap();
            }
            Ok(())
        },
    );
    assert!(
        result.is_err(),
        "a replaced journal must not be erased by commit"
    );
    assert_eq!(world.raw(PENDING), original);
    assert!(world.raw(INVENTORY).is_none());
    assert_eq!(
        std::fs::read(world.artifact(&input).join("package.zip")).unwrap(),
        input.archive
    );
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute_batch("DROP TRIGGER replace_prepared")
        .unwrap();
    assert!(world.abort().unwrap());
    assert!(world.inspect().unwrap().packages().is_empty());
}

#[test]
fn metadata_clear_trigger_cannot_replace_then_erase_journal() {
    for aborting in [false, true] {
        for timing in ["BEFORE", "AFTER"] {
            let mut world = World::new();
            let input = world.input(manifest("consumer", vec![]), 1);
            let at = world.fixture.policy.clock.now;
            let mut original = None;
            let result = install_with_hook(
                &world.trust,
                world.store.as_mut().unwrap(),
                input.borrow(),
                &world.compatibility,
                || at,
                |phase, store| {
                    if phase == InstallPhase::BeforeCommit {
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
                            return Err(InstallError::Storage);
                        }
                        install_delete_trigger(store, timing);
                    }
                    Ok(())
                },
            );
            assert!(result.is_err(), "{timing}, abort={aborting}");
            if aborting {
                install_delete_trigger(world.store.as_ref().unwrap(), timing);
                assert!(
                    world.abort().is_err(),
                    "{timing} abort must retain the journal"
                );
            }
            assert_eq!(world.raw(PENDING), original, "{timing}, abort={aborting}");
            assert!(world.raw(INVENTORY).is_none());
            assert_ne!(
                world.raw("package-trust-state-v1").as_deref(),
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
            assert!(world.raw(PENDING).is_none());
            assert!(world.inspect().unwrap().packages().is_empty());
            let retained = std::fs::read_dir(world.path.join("package-store-v1/quarantine"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            assert_eq!(
                std::fs::read(retained.join("package.zip")).unwrap(),
                input.archive
            );
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
