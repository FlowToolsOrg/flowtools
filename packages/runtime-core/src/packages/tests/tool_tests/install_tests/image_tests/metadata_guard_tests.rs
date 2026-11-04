//! Native materialization must keep an exact abortable SQL journal.
use super::*;

#[test]
fn metadata_commit_trigger_cannot_replace_prepared_journal() {
    let mut world = World::new();
    let source = world.install().unwrap();
    let raw = world.metadata(KEY);
    let mut original = None;
    let result = with_hook(&mut world, &source, |phase, store| {
        if phase == MaterializePhase::BeforeCommit {
            original = Some(
                store
                    .connection
                    .query_row(
                        "SELECT value FROM core_metadata WHERE key=?1",
                        [IMAGE_PENDING],
                        |r| r.get::<_, String>(0),
                    )
                    .unwrap(),
            );
            store.connection.execute_batch(&format!(
                "CREATE TRIGGER replace_prepared AFTER INSERT ON core_metadata WHEN NEW.key='{IMAGE_KEY}' BEGIN UPDATE core_metadata SET value='REPLACED' WHERE key='{IMAGE_PENDING}'; END;"
            )).unwrap();
        }
        Ok(())
    });
    assert!(
        result.is_err(),
        "a replaced journal must not be erased by commit"
    );
    assert_eq!(world.metadata(IMAGE_PENDING), original);
    assert!(world.metadata(IMAGE_KEY).is_none());
    assert_eq!(world.metadata(KEY), raw);
    assert_eq!(
        fs::read(image(&world).join("bin/ffmpeg.exe")).unwrap(),
        world.fixture.contents["bin/ffmpeg.exe"]
    );
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute_batch("DROP TRIGGER replace_prepared")
        .unwrap();
    assert!(abort(&mut world).unwrap());
    assert!(inspect(&mut world).unwrap().is_empty());
}

#[test]
fn metadata_clear_trigger_cannot_replace_then_erase_journal() {
    for aborting in [false, true] {
        for timing in ["BEFORE", "AFTER"] {
            let mut world = World::new();
            let source = world.install().unwrap();
            let raw = world.metadata(KEY);
            let mut original = None;
            let result = with_hook(&mut world, &source, |phase, store| {
                if phase == MaterializePhase::BeforeCommit {
                    original = Some(
                        store
                            .connection
                            .query_row(
                                "SELECT value FROM core_metadata WHERE key=?1",
                                [IMAGE_PENDING],
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
                    abort(&mut world).is_err(),
                    "{timing} abort must retain the journal"
                );
            }
            assert_eq!(
                world.metadata(IMAGE_PENDING),
                original,
                "{timing}, abort={aborting}"
            );
            assert!(world.metadata(IMAGE_KEY).is_none());
            assert_eq!(world.metadata(KEY), raw);
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
            assert!(abort(&mut world).unwrap());
            assert!(world.metadata(IMAGE_PENDING).is_none());
            assert!(inspect(&mut world).unwrap().is_empty());
            let retained = fs::read_dir(world.path.join("tool-image-store-v1/quarantine"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            assert_eq!(
                fs::read(retained.join("bin/ffmpeg.exe")).unwrap(),
                world.fixture.contents["bin/ffmpeg.exe"]
            );
        }
    }
}
fn install_delete_trigger(store: &DataStore, timing: &str) {
    let change = if timing == "BEFORE" {
        format!("UPDATE core_metadata SET value='REPLACED' WHERE key='{IMAGE_PENDING}'")
    } else {
        "UPDATE core_metadata SET value='REPLACED' WHERE key='package-trust-state-v1'".into()
    };
    store.connection.execute_batch(&format!(
        "CREATE TRIGGER replace_on_delete {timing} DELETE ON core_metadata WHEN OLD.key='{IMAGE_PENDING}' BEGIN {change}; END;"
    )).unwrap();
}
