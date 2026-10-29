//! Actual Windows materialized-file transactions, still using inert signed data.
#[path = "../../../../../../../apps/runtime/tests/isolation/filesystem.rs"]
mod filesystem;
#[allow(dead_code)]
#[path = "../../../../../../../apps/runtime/tests/isolation/harness.rs"]
mod harness;
use super::*;
use crate::packages::tools::install::images::{
    abort_inert_tool_materialization, inspect_inert_tool_images, materialize_fixture_with_hook,
    materialize_inert_tool, MaterializePhase, MaterializedInertTool,
};
const IMAGE_KEY: &str = "tool-image-inventory-v1";
const IMAGE_PENDING: &str = "tool-image-materialization-pending-v1";
fn with_hook(
    world: &mut World,
    source: &InstalledInertTool,
    hook: impl FnMut(MaterializePhase, &mut DataStore) -> Result<(), ToolInstallError>,
) -> Result<MaterializedInertTool, ToolInstallError> {
    materialize_fixture_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        source,
        || world.fixture.now(),
        hook,
    )
}
fn abort(world: &mut World) -> Result<bool, ToolInstallError> {
    abort_inert_tool_materialization(&world.trust, world.store.as_mut().unwrap())
}
fn inspect(
    world: &mut World,
) -> Result<Vec<crate::packages::tools::install::ToolArchivePin>, ToolInstallError> {
    inspect_inert_tool_images(&world.trust, world.store.as_mut().unwrap())
}
fn materialize(
    world: &mut World,
    source: &InstalledInertTool,
) -> Result<MaterializedInertTool, ToolInstallError> {
    materialize_fixture_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        source,
        || world.fixture.now(),
        |_, _| Ok(()),
    )
}
fn image(world: &World) -> PathBuf {
    world
        .path
        .join("tool-image-store-v1/artifacts")
        .join(&world.fixture.base.package.archive.sha256)
}
#[test]
fn production_profile_refuses_materialization_before_creating_image_namespace_or_journal() {
    let mut world = World::new();
    let source = world.install().unwrap();
    let outcome =
        materialize_inert_tool(&world.trust, world.store.as_mut().unwrap(), &source, || {
            world.fixture.now()
        });
    assert!(matches!(
        outcome,
        Err(ToolInstallError::Admission(
            ToolAdmissionError::PolicyDenied
        ))
    ));
    assert!(!world.path.join("tool-image-store-v1").exists());
    assert!(world.metadata(IMAGE_KEY).is_none());
    assert!(world.metadata(IMAGE_PENDING).is_none());
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn real_sealed_tree_reuses_identical_native_files_across_multiple_readers() {
    let mut world = World::new();
    let source = world.install().unwrap();
    let first = materialize(&mut world, &source).unwrap();
    let original = world.metadata(IMAGE_KEY).unwrap();
    assert!(world.metadata(IMAGE_PENDING).is_none());
    for (name, bytes) in &world.fixture.contents {
        assert_eq!(fs::read(image(&world).join(name)).unwrap(), *bytes);
    }
    assert_eq!(
        fs::read(image(&world).join("manifest.json")).unwrap(),
        world.fixture.base.manifest
    );
    let second = materialize(&mut world, &source).unwrap();
    assert_eq!(first.pin(), second.pin());
    assert_eq!(first.materialized_at(), second.materialized_at());
    assert_eq!(world.metadata(IMAGE_KEY).unwrap(), original);
    assert_eq!(
        fs::read_dir(world.path.join("tool-image-store-v1/artifacts"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        inspect_inert_tool_images(&world.trust, world.store.as_mut().unwrap()).unwrap(),
        vec![source.pin()]
    );
    assert!(
        !abort_inert_tool_materialization(&world.trust, world.store.as_mut().unwrap()).unwrap()
    );
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn held_readers_prevent_writes_replacement_and_republication_even_after_readonly_is_cleared() {
    let mut world = World::new();
    let source = world.install().unwrap();
    let held = materialize(&mut world, &source).unwrap();
    let path = image(&world).join("bin/ffmpeg.exe");
    writable(&path);
    assert!(fs::write(&path, b"REPLACE").is_err());
    assert!(fs::remove_file(&path).is_err());
    assert!(fs::rename(image(&world), image(&world).with_extension("moved")).is_err());
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    assert_eq!(
        fs::read(&path).unwrap(),
        world.fixture.contents["bin/ffmpeg.exe"]
    );
    assert_eq!(inspect(&mut world).unwrap(), vec![held.pin()]);
}
#[test]
fn interrupted_images_block_all_installation_until_explicit_abort_preserves_exact_bytes() {
    for phase in [
        MaterializePhase::JournalCommitted,
        MaterializePhase::StagePrepared,
        MaterializePhase::ArtifactPublished,
        MaterializePhase::BeforeCommit,
    ] {
        let mut world = World::new();
        let source = world.install().unwrap();
        let result = with_hook(&mut world, &source, |observed, _| {
            if observed == phase {
                Err(ToolInstallError::Storage(InstallError::Storage))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        assert!(world.metadata(IMAGE_KEY).is_none());
        let pending: Value = serde_json::from_str(&world.metadata(IMAGE_PENDING).unwrap()).unwrap();
        let pending_id = pending["id"].as_str().unwrap();
        for error in [
            world.install().err(),
            world.pins().err(),
            world.abort().err(),
            inspect(&mut world).err(),
        ] {
            assert_eq!(
                error,
                Some(ToolInstallError::Storage(InstallError::RecoveryPending))
            );
        }
        assert!(matches!(
            inspect_inert_inventory(&world.trust, world.store.as_mut().unwrap()),
            Err(InstallError::RecoveryPending)
        ));
        assert!(abort(&mut world).unwrap());
        assert!(!abort(&mut world).unwrap());
        assert!(world.metadata(IMAGE_PENDING).is_none());
        assert!(inspect(&mut world).unwrap().is_empty());
        let suffix = if matches!(phase, MaterializePhase::StagePrepared) {
            "stage"
        } else {
            "artifact"
        };
        if phase != MaterializePhase::JournalCommitted {
            let quarantined = world
                .path
                .join("tool-image-store-v1/quarantine")
                .join(format!("{pending_id}-{suffix}"));
            for (name, bytes) in &world.fixture.contents {
                assert_eq!(fs::read(quarantined.join(name)).unwrap(), *bytes);
            }
            assert_eq!(
                fs::read(quarantined.join("manifest.json")).unwrap(),
                world.fixture.base.manifest
            );
        }
        assert_eq!(world.pins().unwrap(), vec![source.pin()]);
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
#[test]
fn corrupt_duplicate_and_old_image_inventory_or_journal_are_preserved_without_adoption() {
    for key in [IMAGE_KEY, IMAGE_PENDING] {
        for text in [
            "CORRUPT",
            "{}",
            "{\"formatVersion\":0}",
            "{\"formatVersion\":1,\"formatVersion\":1}",
        ] {
            let mut world = World::new();
            let source = world.install().unwrap();
            world
                .store
                .as_ref()
                .unwrap()
                .connection
                .execute(
                    "INSERT INTO core_metadata(key,value) VALUES(?1,?2)",
                    rusqlite::params![key, text],
                )
                .unwrap();
            assert!(materialize(&mut world, &source).is_err());
            assert!(inspect(&mut world).is_err());
            assert!(abort(&mut world).is_err());
            assert_eq!(world.metadata(key).as_deref(), Some(text));
            assert!(!world.path.join("tool-image-store-v1").exists());
        }
    }
}
#[test]
fn tampered_bytes_seal_extra_files_and_hardlinks_cannot_be_reused() {
    for attack in 0..4 {
        let mut world = World::new();
        let source = world.install().unwrap();
        drop(materialize(&mut world, &source).unwrap());
        let before = world.metadata(IMAGE_KEY).unwrap();
        let root = image(&world);
        if attack < 2 {
            let path = root.join(if attack == 0 {
                "bin/ffmpeg.exe"
            } else {
                "seal.json"
            });
            writable(&path);
            fs::write(&path, b"CHANGED_BYTES").unwrap();
            let mut p = fs::metadata(&path).unwrap().permissions();
            p.set_readonly(true);
            fs::set_permissions(&path, p).unwrap();
        } else if attack == 2 {
            fs::write(root.join("unexpected.txt"), b"EXTRA").unwrap();
        } else {
            fs::hard_link(
                root.join("bin/ffmpeg.exe"),
                world.path.join("extra-link.exe"),
            )
            .unwrap();
        }
        assert!(materialize(&mut world, &source).is_err());
        assert!(inspect(&mut world).is_err());
        assert_eq!(world.metadata(IMAGE_KEY).as_deref(), Some(before.as_str()));
        assert!(world.metadata(IMAGE_PENDING).is_none());
    }
}
#[test]
fn sql_trigger_mutations_roll_back_inventory_and_leave_abortable_files() {
    for sql in [
        "CREATE TRIGGER fail_image BEFORE INSERT ON core_metadata WHEN NEW.key='tool-image-inventory-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END;",
        "CREATE TRIGGER change_image AFTER INSERT ON core_metadata WHEN NEW.key='tool-image-inventory-v1' BEGIN UPDATE core_metadata SET value='CORRUPT' WHERE key='tool-image-inventory-v1'; END;",
        "CREATE TRIGGER change_source AFTER INSERT ON core_metadata WHEN NEW.key='tool-image-inventory-v1' BEGIN UPDATE core_metadata SET value='CORRUPT' WHERE key='tool-archive-inventory-v1'; END;",
        "CREATE TRIGGER change_trust AFTER INSERT ON core_metadata WHEN NEW.key='tool-image-inventory-v1' BEGIN UPDATE core_metadata SET value='CORRUPT' WHERE key='package-trust-state-v1'; END;",
    ] {
        let mut world=World::new();let source=world.install().unwrap();
        let raw=world.metadata(KEY).unwrap();
        let result=with_hook(&mut world,&source,|phase,store|{if phase==MaterializePhase::BeforeCommit {store.connection.execute_batch(sql).unwrap();}Ok(())});
        assert!(result.is_err());assert!(world.metadata(IMAGE_KEY).is_none());assert!(world.metadata(IMAGE_PENDING).is_some());
        assert_eq!(world.metadata(KEY).as_deref(),Some(raw.as_str()));
        assert_eq!(fs::read(image(&world).join("bin/ffmpeg.exe")).unwrap(),world.fixture.contents["bin/ffmpeg.exe"]);
        assert!(abort(&mut world).unwrap());assert!(inspect(&mut world).unwrap().is_empty());
    }
}
#[test]
fn fresh_commit_clock_expiry_rollback_and_package_revocation_refuse_a_materialized_proof() {
    for expired in [false, true] {
        let mut world = World::new();
        let source = world.install().unwrap();
        let mut calls = 0;
        let result = materialize_fixture_with_hook(
            &world.trust,
            world.store.as_mut().unwrap(),
            &source,
            || {
                calls += 1;
                if calls < 5 {
                    world.fixture.now()
                } else if expired {
                    world.fixture.base.package.expires_at
                } else {
                    world.fixture.now() - 1
                }
            },
            |_, _| Ok(()),
        );
        assert_eq!(calls, 5);
        assert_eq!(
            result.err(),
            Some(ToolInstallError::Admission(ToolAdmissionError::Package(
                if expired {
                    PackageError::TrustExpired
                } else {
                    PackageError::ClockRollback
                }
            )))
        );
        assert!(world.metadata(IMAGE_KEY).is_none());
        assert!(world.metadata(IMAGE_PENDING).is_some());
        assert!(abort(&mut world).unwrap());
    }
    let mut world = World::new();
    let source = world.install().unwrap();
    let trust = world.fixture.trust();
    let now = world.fixture.now();
    let envelope = serde_json::to_vec(&world.fixture.base.golden["rootEnvelope"]).unwrap();
    let mut root: RootPayload = serde_json::from_slice(
        &decode_envelope(&envelope, ROOT_PAYLOAD_TYPE)
            .unwrap()
            .payload,
    )
    .unwrap();
    root.version += 1;
    root.previous_root_sha256 = Some(
        trust
            .snapshot(world.store.as_mut().unwrap())
            .unwrap()
            .root_digest,
    );
    root.revoked_package_digests
        .push(world.fixture.base.package.archive.sha256.clone());
    let result = with_hook(&mut world, &source, |phase, store| {
        if phase == MaterializePhase::BeforeCommit {
            trust
                .rotate(store, &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]), now)
                .unwrap();
        }
        Ok(())
    });
    assert_eq!(
        result.err(),
        Some(ToolInstallError::Admission(ToolAdmissionError::Package(
            PackageError::PackageRevoked
        )))
    );
    let before = trust
        .snapshot(world.store.as_mut().unwrap())
        .unwrap()
        .root_digest;
    assert!(abort(&mut world).unwrap());
    assert_eq!(
        trust
            .snapshot(world.store.as_mut().unwrap())
            .unwrap()
            .root_digest,
        before
    );
}
#[test]
fn image_quarantine_plugin_blobs_and_raw_tool_blobs_share_one_disk_and_group_budget() {
    for byte_limit in [false, true] {
        let mut world = World::new();
        let source = world.install().unwrap();
        assert!(with_hook(&mut world, &source, |phase, _| {
            if phase == MaterializePhase::JournalCommitted {
                Err(ToolInstallError::Storage(InstallError::Storage))
            } else {
                Ok(())
            }
        })
        .is_err());
        assert!(abort(&mut world).unwrap());
        let quarantine = world.path.join("tool-image-store-v1/quarantine");
        let count = if byte_limit { 4 } else { 383 };
        for _ in 0..count {
            let group = quarantine.join(format!("{}-stage", uuid::Uuid::new_v4()));
            fs::create_dir(&group).unwrap();
            if byte_limit {
                fs::File::create(group.join("retained.bin"))
                    .unwrap()
                    .set_len(67_108_864)
                    .unwrap();
            }
        }
        assert_eq!(
            materialize(&mut world, &source)
                .err()
                .map(|e| format!("{e:?}")),
            Some(format!(
                "{:?}",
                ToolInstallError::Storage(InstallError::BudgetExceeded)
            ))
        );
        world.fixture.base.package.version = "1.0.1".into();
        world.fixture.base.package.release_sequence += 1;
        world.fixture.manifest["version"] = json!("1.0.1");
        world.fixture.rebuild();
        assert_eq!(
            world.install().err(),
            Some(ToolInstallError::Storage(InstallError::BudgetExceeded))
        );
        assert!(world.metadata(IMAGE_KEY).is_none());
        assert!(world.metadata(IMAGE_PENDING).is_none());
        assert_eq!(fs::read_dir(quarantine).unwrap().count(), count);
    }
}
#[test]
fn copied_and_restored_profiles_preserve_image_metadata_without_reusing_paths_or_grants() {
    let mut world = World::new();
    let source = world.install().unwrap();
    drop(materialize(&mut world, &source).unwrap());
    let before = world.metadata(IMAGE_KEY).unwrap();
    let original = fs::read(image(&world).join("bin/ffmpeg.exe")).unwrap();
    let backup = world.store.as_ref().unwrap().backup().unwrap();
    drop(world.store.take());
    let copied =
        std::env::temp_dir().join(format!("flowtools-tool-install-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&copied).unwrap();
    fs::copy(
        world.path.join("runtime.sqlite"),
        copied.join("runtime.sqlite"),
    )
    .unwrap();
    let mut copy = World::open(&copied, false, true);
    assert!(materialize(&mut copy, &source).is_err());
    assert!(inspect(&mut copy).is_err());
    assert!(abort(&mut copy).is_err());
    assert_eq!(copy.metadata(IMAGE_KEY).as_deref(), Some(before.as_str()));
    assert!(!copy.path.join("tool-image-store-v1").exists());
    DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
    world.reopen();
    assert!(materialize(&mut world, &source).is_err());
    assert!(inspect(&mut world).is_err());
    assert!(abort(&mut world).is_err());
    assert_eq!(world.metadata(IMAGE_KEY).as_deref(), Some(before.as_str()));
    assert_eq!(
        fs::read(image(&world).join("bin/ffmpeg.exe")).unwrap(),
        original
    );
    assert_no_effects(world.store.as_ref().unwrap());
}
const IMAGE_PHASES: [MaterializePhase; 5] = [
    MaterializePhase::JournalCommitted,
    MaterializePhase::StagePrepared,
    MaterializePhase::ArtifactPublished,
    MaterializePhase::BeforeCommit,
    MaterializePhase::Committed,
];
#[test]
fn image_process_crash_child() {
    let Some(path) = std::env::var_os("FLOWTOOLS_TOOL_IMAGE_CHILD_ROOT") else {
        return;
    };
    let phase: usize = std::env::var("FLOWTOOLS_TOOL_IMAGE_BARRIER")
        .unwrap()
        .parse()
        .unwrap();
    assert!(phase < IMAGE_PHASES.len());
    let mut world = World::open(Path::new(&path), false, false);
    let source = world.install().unwrap();
    with_hook(&mut world, &source, |observed, _| {
        if observed == IMAGE_PHASES[phase] {
            std::process::exit(80 + phase as i32);
        }
        Ok(())
    })
    .unwrap();
    panic!("child missed barrier");
}
#[test]
fn actual_process_exits_at_all_five_barriers_never_adopt_uncommitted_image_files() {
    for index in 0..IMAGE_PHASES.len() {
        let mut world = World::new();
        world.install().unwrap();
        drop(world.store.take());
        let status=Command::new(std::env::current_exe().unwrap()).args(["--exact","packages::tests::tool_tests::install_tests::image_tests::image_process_crash_child","--nocapture"])
            .env("FLOWTOOLS_TOOL_IMAGE_CHILD_ROOT",&world.path).env("FLOWTOOLS_TOOL_IMAGE_BARRIER",index.to_string()).status().unwrap();
        assert_eq!(status.code(), Some(80 + index as i32));
        world.reopen();
        if index == 4 {
            assert_eq!(inspect(&mut world).unwrap().len(), 1);
            assert!(world.metadata(IMAGE_PENDING).is_none());
        } else {
            assert_eq!(
                inspect(&mut world).err(),
                Some(ToolInstallError::Storage(InstallError::RecoveryPending))
            );
            assert!(abort(&mut world).unwrap());
            assert!(inspect(&mut world).unwrap().is_empty());
        }
        assert_eq!(world.pins().unwrap().len(), 1);
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
fn lpac_report(world: &World, other: &World, root: &Path, capability: bool) -> Value {
    use std::time::Duration;
    let fixture = harness::Fixture::new().unwrap();
    let inventory: Value = serde_json::from_str(
        &world
            .metadata(IMAGE_KEY)
            .unwrap_or_else(|| world.metadata(IMAGE_PENDING).unwrap()),
    )
    .unwrap();
    let profile = inventory["profileDigest"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            // The pending-only test obtains the same canonical DB binding from the
            // already validated raw inventory, never a caller-selected profile.
            let raw: Value = serde_json::from_str(&world.metadata(KEY).unwrap()).unwrap();
            raw["profileDigest"].as_str().unwrap().to_owned()
        });
    let child = fixture
        .launch_tool_files(
            &profile,
            &world.fixture.base.package.archive.sha256,
            root,
            &image(other),
            capability,
        )
        .unwrap();
    let (exit, _, stderr) = child.output(Duration::from_secs(5), 65536).unwrap();
    assert_eq!(exit, 0, "{}", String::from_utf8_lossy(&stderr));
    child.wait_empty(Duration::from_millis(500)).unwrap();
    assert_eq!(child.active().unwrap(), 0);
    assert!(!child.private_inherited);
    let bytes = fs::read(fixture.work.join("report.json")).unwrap();
    assert!(bytes.len() < 16384);
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["pid"], child.pub_pid);
    assert_eq!(report["sid"], fixture.container.text);
    assert_eq!(report["lpac"], true);
    assert_eq!(report["capabilities"], if capability { 2 } else { 1 });
    assert_eq!(report["write"], false);
    assert_eq!(report["relabel"], false);
    assert_eq!(report["otherRead"], false);
    report
}
#[test]
fn two_real_lpac_identities_read_one_sealed_artifact_but_cannot_mutate_or_read_another_profile() {
    let mut world = World::new();
    let source = world.install().unwrap();
    drop(materialize(&mut world, &source).unwrap());
    let mut other = World::new();
    let source = other.install().unwrap();
    drop(materialize(&mut other, &source).unwrap());
    let path = image(&world).join("bin/ffmpeg.exe");
    writable(&path);
    let first = lpac_report(&world, &other, &image(&world), true);
    let second = lpac_report(&world, &other, &image(&world), true);
    let denied = lpac_report(&world, &other, &image(&world), false);
    let mut p = fs::metadata(&path).unwrap().permissions();
    p.set_readonly(true);
    fs::set_permissions(&path, p).unwrap();
    assert_eq!(
        first["bytes"],
        json!(world.fixture.contents["bin/ffmpeg.exe"])
    );
    assert_eq!(second["bytes"], first["bytes"]);
    assert_ne!(first["sid"], second["sid"]);
    assert_eq!(first["identity"], second["identity"]);
    assert!(first["identity"].is_string());
    assert!(denied["bytes"].is_null());
    assert!(denied["identity"].is_null());
    assert_eq!(inspect(&mut world).unwrap().len(), 1);
}
#[test]
fn explicit_abort_revokes_artifact_capability_even_if_clearing_journal_fails() {
    let mut world = World::new();
    let source = world.install().unwrap();
    assert!(with_hook(&mut world, &source, |phase, _| {
        if phase == MaterializePhase::ArtifactPublished {
            Err(ToolInstallError::Storage(InstallError::Storage))
        } else {
            Ok(())
        }
    })
    .is_err());
    let mut other = World::new();
    let other_source = other.install().unwrap();
    drop(materialize(&mut other, &other_source).unwrap());
    let before = lpac_report(&world, &other, &image(&world), true);
    assert!(before["bytes"].is_array());
    let pending = world.metadata(IMAGE_PENDING).unwrap();
    let record: Value = serde_json::from_str(&pending).unwrap();
    let root = world
        .path
        .join("tool-image-store-v1/quarantine")
        .join(format!("{}-artifact", record["id"].as_str().unwrap()));
    world.store.as_ref().unwrap().connection.execute_batch("CREATE TRIGGER fail_image_abort BEFORE DELETE ON core_metadata WHEN OLD.key='tool-image-materialization-pending-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(abort(&mut world).is_err());
    assert_eq!(
        world.metadata(IMAGE_PENDING).as_deref(),
        Some(pending.as_str())
    );
    assert_eq!(
        fs::read(root.join("bin/ffmpeg.exe")).unwrap(),
        world.fixture.contents["bin/ffmpeg.exe"]
    );
    let denied = lpac_report(&world, &other, &root, true);
    assert!(denied["bytes"].is_null());
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute_batch("DROP TRIGGER fail_image_abort;")
        .unwrap();
    assert!(abort(&mut world).unwrap());
    assert!(!abort(&mut world).unwrap());
    assert!(inspect(&mut world).unwrap().is_empty());
}
#[test]
fn modified_acl_low_integrity_and_native_junctions_are_refused_without_repairing_them() {
    for low in [false, true] {
        let mut world = World::new();
        let source = world.install().unwrap();
        drop(materialize(&mut world, &source).unwrap());
        let before = world.metadata(IMAGE_KEY).unwrap();
        let user = harness::user_sid().unwrap();
        let sddl = if low {
            format!("D:P(A;;FA;;;{user})S:(ML;;NW;;;LW)")
        } else {
            format!("D:P(A;;FA;;;{user})(A;;FRFX;;;WD)")
        };
        harness::protect(&image(&world).join("bin/ffmpeg.exe"), &sddl, low).unwrap();
        assert!(inspect(&mut world).is_err());
        assert!(materialize(&mut world, &source).is_err());
        assert_eq!(world.metadata(IMAGE_KEY).as_deref(), Some(before.as_str()));
    }
    let mut world = World::new();
    let source = world.install().unwrap();
    let other = World::new();
    fs::write(other.path.join("external.txt"), b"EXTERNAL_CANARY").unwrap();
    let path = world.path.join("tool-image-store-v1");
    filesystem::junction(&path, &other.path).unwrap();
    let refused = materialize(&mut world, &source).is_err();
    let preserved = fs::read(other.path.join("external.txt")).unwrap();
    fs::remove_dir(path).unwrap();
    assert!(refused);
    assert_eq!(preserved, b"EXTERNAL_CANARY");
    assert!(world.metadata(IMAGE_PENDING).is_none());
}
#[test]
fn target_collision_and_corrupt_abort_batch_preserve_all_source_groups() {
    let mut world = World::new();
    let source = world.install().unwrap();
    let target = image(&world);
    let result = with_hook(&mut world, &source, |phase, _| {
        if phase == MaterializePhase::StagePrepared {
            fs::create_dir(&target).unwrap();
            fs::write(target.join("collision.txt"), b"DO_NOT_OVERWRITE").unwrap();
        }
        Ok(())
    });
    assert!(result.is_err());
    let pending: Value = serde_json::from_str(&world.metadata(IMAGE_PENDING).unwrap()).unwrap();
    let stage = world
        .path
        .join("tool-image-store-v1/staging")
        .join(pending["id"].as_str().unwrap());
    assert!(abort(&mut world).is_err());
    assert!(stage.is_dir());
    assert_eq!(
        fs::read(target.join("collision.txt")).unwrap(),
        b"DO_NOT_OVERWRITE"
    );
    assert_eq!(
        fs::read(stage.join("bin/ffmpeg.exe")).unwrap(),
        world.fixture.contents["bin/ffmpeg.exe"]
    );
    assert_eq!(
        fs::read_dir(world.path.join("tool-image-store-v1/quarantine"))
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn changed_source_zip_is_refused_before_image_io_and_private_partial_stage_can_be_aborted() {
    let mut world = World::new();
    let source = world.install().unwrap();
    let raw = world.blob().join("package.zip");
    writable(&raw);
    fs::write(&raw, b"CHANGED_SOURCE").unwrap();
    assert!(materialize(&mut world, &source).is_err());
    assert!(!world.path.join("tool-image-store-v1").exists());
    assert!(world.metadata(IMAGE_PENDING).is_none());
    fs::write(&raw, &world.fixture.base.archive).unwrap();
    let mut p = fs::metadata(&raw).unwrap().permissions();
    p.set_readonly(true);
    fs::set_permissions(&raw, p).unwrap();
    let root = world.path.join("tool-image-store-v1");
    assert!(with_hook(&mut world, &source, |phase, store| {
        if phase == MaterializePhase::JournalCommitted {
            let text: String = store
                .connection
                .query_row(
                    "SELECT value FROM core_metadata WHERE key=?1",
                    [IMAGE_PENDING],
                    |r| r.get(0),
                )
                .unwrap();
            let value: Value = serde_json::from_str(&text).unwrap();
            let stage = root.join("staging").join(value["id"].as_str().unwrap());
            fs::create_dir(&stage).unwrap();
            fs::create_dir(stage.join("bin")).unwrap();
            fs::write(stage.join("bin/ffmpeg.exe"), b"PARTIAL").unwrap();
            return Err(ToolInstallError::Storage(InstallError::Storage));
        }
        Ok(())
    })
    .is_err());
    let pending: Value = serde_json::from_str(&world.metadata(IMAGE_PENDING).unwrap()).unwrap();
    assert!(abort(&mut world).unwrap());
    assert_eq!(
        fs::read(
            root.join("quarantine")
                .join(format!("{}-stage", pending["id"].as_str().unwrap()))
                .join("bin/ffmpeg.exe")
        )
        .unwrap(),
        b"PARTIAL"
    );
    assert!(inspect(&mut world).unwrap().is_empty());
}
#[test]
fn abort_sql_trigger_cannot_mutate_trust_or_source_metadata_while_clearing_pending() {
    for key in [KEY, "package-trust-state-v1", IMAGE_KEY] {
        let mut world = World::new();
        let source = world.install().unwrap();
        assert!(with_hook(&mut world, &source, |phase, _| {
            if phase == MaterializePhase::ArtifactPublished {
                Err(ToolInstallError::Storage(InstallError::Storage))
            } else {
                Ok(())
            }
        })
        .is_err());
        let pending = world.metadata(IMAGE_PENDING).unwrap();
        let old = world.metadata(key);
        world.store.as_ref().unwrap().connection.execute_batch(&format!("CREATE TRIGGER change_during_abort AFTER DELETE ON core_metadata WHEN OLD.key='tool-image-materialization-pending-v1' BEGIN INSERT INTO core_metadata(key,value) VALUES('{key}','CORRUPT') ON CONFLICT(key) DO UPDATE SET value='CORRUPT'; END;")).unwrap();
        assert!(abort(&mut world).is_err());
        assert_eq!(world.metadata(key), old);
        assert_eq!(
            world.metadata(IMAGE_PENDING).as_deref(),
            Some(pending.as_str())
        );
        world
            .store
            .as_ref()
            .unwrap()
            .connection
            .execute_batch("DROP TRIGGER change_during_abort;")
            .unwrap();
        assert!(abort(&mut world).unwrap());
    }
}
