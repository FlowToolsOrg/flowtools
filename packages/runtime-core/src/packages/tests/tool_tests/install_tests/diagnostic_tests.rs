use super::*;
use crate::{protocol::ErrorCode, tool_inventory::ToolInventoryQuery};

fn query() -> ToolInventoryQuery {
    ToolInventoryQuery {
        offset: 0,
        snapshot: None,
    }
}

#[test]
fn tool_inventory_observation_keeps_empty_storage_absent_and_installed_bytes_unchanged() {
    let mut world = World::new();
    let observe = |world: &mut World, query: &ToolInventoryQuery| {
        crate::packages::tools::install::diagnostics::observe(
            &world.trust,
            world.store.as_mut().unwrap(),
            query,
        )
    };
    let empty = observe(&mut world, &query()).unwrap();
    assert!(empty.items.is_empty());
    assert_eq!(empty.storage.as_ref().unwrap().bytes, 0);
    assert!(!world.root().exists());
    world.install().unwrap();
    let before = world.metadata(KEY);
    let report = observe(&mut world, &query()).unwrap();
    assert_eq!(report.total, 1);
    assert_eq!(
        report.items[0].archive_digest,
        world.fixture.base.package.archive.sha256
    );
    assert!(!report.items[0].materialized);
    assert_eq!(report.items[0].retained_jobs, 0);
    assert!(report.items[0].dependents.is_empty());
    assert!(report.storage.as_ref().unwrap().bytes > report.items[0].archive_bytes);
    assert_eq!(before, world.metadata(KEY));
    assert_eq!(world.pins().unwrap().len(), 1);
    let stale = ToolInventoryQuery {
        offset: 0,
        snapshot: empty.snapshot,
    };
    assert_eq!(
        observe(&mut world, &stale).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    let mut bad = query();
    bad.offset = 1;
    assert_eq!(
        observe(&mut world, &bad).unwrap_err(),
        ErrorCode::InputInvalid
    );
}

#[test]
fn tool_inventory_refuses_pending_corruption_and_actual_artifact_changes_without_empty_fallback() {
    let mut world = World::new();
    world.install().unwrap();
    let foreign_pending = "tool-image-materialization-pending-v1";
    world
        .store
        .as_mut()
        .unwrap()
        .connection
        .execute(
            "INSERT INTO core_metadata(key,value) VALUES(?1,?2)",
            rusqlite::params![foreign_pending, "private-image-pending"],
        )
        .unwrap();
    assert_eq!(
        crate::packages::tools::install::diagnostics::observe(
            &world.trust,
            world.store.as_mut().unwrap(),
            &query()
        )
        .unwrap_err(),
        ErrorCode::RecoveryPending
    );
    assert_eq!(
        world.metadata(foreign_pending).as_deref(),
        Some("private-image-pending")
    );
    world
        .store
        .as_mut()
        .unwrap()
        .connection
        .execute("DELETE FROM core_metadata WHERE key=?1", [foreign_pending])
        .unwrap();
    let store = world.store.as_mut().unwrap();
    store
        .connection
        .execute(
            "INSERT INTO core_metadata(key,value) VALUES(?1,?2)",
            rusqlite::params![PENDING, "private-invalid-pending"],
        )
        .unwrap();
    assert_eq!(
        crate::packages::tools::install::diagnostics::observe(
            &world.trust,
            world.store.as_mut().unwrap(),
            &query()
        )
        .unwrap_err(),
        ErrorCode::DependencyArtifactMismatch
    );
    assert_eq!(
        world.metadata(PENDING).as_deref(),
        Some("private-invalid-pending")
    );
    world
        .store
        .as_mut()
        .unwrap()
        .connection
        .execute("DELETE FROM core_metadata WHERE key=?1", [PENDING])
        .unwrap();
    writable(&world.blob().join("package.zip"));
    fs::write(world.blob().join("package.zip"), b"changed").unwrap();
    assert!(crate::packages::tools::install::diagnostics::observe(
        &world.trust,
        world.store.as_mut().unwrap(),
        &query()
    )
    .is_err());
    assert_eq!(
        fs::read(world.blob().join("package.zip")).unwrap(),
        b"changed"
    );
}
