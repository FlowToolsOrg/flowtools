//! Actual signed consumer/tool ZIP transactions; no code import or execution.
use super::super::super::dependency_tests::{bytes, manifest};
use super::*;
use crate::packages::{
    install::{
        self,
        tool_consumers::{self as consumers, ConsumerInstallPhase as Phase, InstalledToolConsumer},
        InertPackageBytes,
    },
    manifest::ManifestCompatibility,
    tools::install::{
        images::{
            inspect_inert_tool_images, materialize_fixture_with_hook,
            reconciliation::plan_image_reconciliation,
        },
        reconciliation::plan_tool_archive_reconciliation,
    },
};
const CONSUMERS: &str = "tool-consumer-inventory-v1";
const CONSUMER_PENDING: &str = "tool-consumer-install-pending-v1";
mod reconciliation_tests;
mod recovery_tests;
fn open(path: &Path, bootstrap: bool, cleanup: bool) -> World {
    let mut fixture = ToolFixture::new(CompressionMethod::Stored);
    let mut root: RootPayload = serde_json::from_slice(
        &decode_envelope(
            &serde_json::to_vec(&fixture.base.golden["rootEnvelope"]).unwrap(),
            ROOT_PAYLOAD_TYPE,
        )
        .unwrap()
        .payload,
    )
    .unwrap();
    root.publishers[0]
        .packages
        .extend(["consumer", "middle", "provider"].map(|id| PublisherScope {
            kind: PackageKind::Plugin,
            id: id.into(),
            targets: vec![fixture.base.package.target.clone()],
        }));
    fixture.base.golden["rootEnvelope"] =
        serde_json::from_slice(&signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2])).unwrap();
    let trust = fixture.trust();
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    if bootstrap {
        fixture.bootstrap(&mut store, &trust);
    }
    World {
        fixture,
        trust,
        store: Some(store),
        path: path.into(),
        cleanup,
    }
}
fn world() -> World {
    let path =
        std::env::temp_dir().join(format!("flowtools-tool-install-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&path).unwrap();
    open(&path, true, true)
}
fn compatibility(world: &World) -> ManifestCompatibility {
    ManifestCompatibility {
        host_version: "0.1.0".into(),
        sdk_version: "0.0.0".into(),
        target: world.fixture.base.package.target.clone(),
    }
}
struct Input {
    envelope: Vec<u8>,
    manifest: Vec<u8>,
    archive: Vec<u8>,
    payload: PackagePayload,
}
impl Input {
    fn borrow(&self) -> InertPackageBytes<'_> {
        InertPackageBytes {
            envelope: &self.envelope,
            manifest: &self.manifest,
            archive: &self.archive,
        }
    }
}
fn input(world: &World, id: &str, services: Vec<Value>, tool: bool, sequence: u64) -> Input {
    let mut value = manifest(id, services);
    if tool {
        value["dependencies"]["tools"] = json!([{
            "publisher":world.fixture.base.package.publisher, "id":"ffmpeg", "version":"*",
            "target":world.fixture.base.package.target, "buildFlavor":"fixture-static",
            "digest":world.fixture.base.package.archive.sha256,
        }]);
    }
    input_value(world, value, sequence)
}
fn input_value(world: &World, value: Value, sequence: u64) -> Input {
    let (mut payload, manifest, archive) = bytes(&world.fixture.base, value, sequence);
    payload.kind = PackageKind::Plugin;
    Input {
        envelope: signed(PACKAGE_PAYLOAD_TYPE, &payload, &[3]),
        payload,
        manifest,
        archive,
    }
}
fn service(id: &str) -> Value {
    json!({"publisher":"fixture-publisher","id":id,"version":"^1.0.0","service":"text-transform","interfaceVersion":"^1.0.0"})
}
fn install(world: &mut World, input: &Input) -> Result<InstalledToolConsumer, ToolInstallError> {
    with_hook(world, input, |_, _| Ok(()))
}
fn with_hook(
    world: &mut World,
    input: &Input,
    hook: impl FnMut(Phase, &mut DataStore) -> Result<(), ToolInstallError>,
) -> Result<InstalledToolConsumer, ToolInstallError> {
    let compatibility = compatibility(world);
    let now = world.fixture.now();
    consumers::install_fixture_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        input.borrow(),
        &compatibility,
        || now,
        hook,
    )
}
fn inspect(world: &mut World) -> Result<Vec<InstalledToolConsumer>, ToolInstallError> {
    consumers::inspect_inert_tool_consumers(&world.trust, world.store.as_mut().unwrap())
}
fn abort(world: &mut World) -> Result<bool, ToolInstallError> {
    consumers::abort_inert_tool_consumer_install(&world.trust, world.store.as_mut().unwrap())
}
fn set(world: &World, key: &str, text: &str) {
    world.store.as_ref().unwrap().connection.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [key, text]).unwrap();
}
fn remove(world: &World, key: &str) {
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute("DELETE FROM core_metadata WHERE key=?1", [key])
        .unwrap();
}
fn artifact(world: &World, input: &Input) -> PathBuf {
    world
        .path
        .join("tool-bound-plugin-store-v1/artifacts")
        .join(&input.payload.archive.sha256)
}

#[test]
fn production_review_refuses_before_consumer_namespace_or_journal() {
    let mut world = world();
    world.install().unwrap();
    let input = input(&world, "consumer", vec![], true, 1);
    let raw = world.metadata(KEY);
    let compatibility = compatibility(&world);
    let now = world.fixture.now();
    assert_eq!(
        consumers::install_inert_tool_consumer(
            &world.trust,
            world.store.as_mut().unwrap(),
            input.borrow(),
            &compatibility,
            || now
        )
        .err(),
        Some(ToolInstallError::Admission(
            ToolAdmissionError::PolicyDenied
        ))
    );
    assert!(!world.path.join("tool-bound-plugin-store-v1").exists());
    assert!(world.metadata(CONSUMERS).is_none());
    assert!(world.metadata(CONSUMER_PENDING).is_none());
    assert_eq!(world.metadata(KEY), raw);
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn two_installed_consumers_share_one_original_tool_zip_and_preserve_legacy_v2_bytes() {
    let mut world = world();
    world.install().unwrap();
    let provider = input(&world, "provider", vec![], false, 1);
    let compatibility = compatibility(&world);
    let now = world.fixture.now();
    install::install_inert_plugin(
        &world.trust,
        world.store.as_mut().unwrap(),
        provider.borrow(),
        &compatibility,
        || now,
    )
    .unwrap();
    let legacy = world.metadata("package-install-inventory-v1");
    let raw = world.metadata(KEY);
    let middle = input(&world, "middle", vec![service("provider")], true, 1);
    let first = install(&mut world, &middle).unwrap();
    let consumer = input(&world, "consumer", vec![service("middle")], true, 1);
    let second = install(&mut world, &consumer).unwrap();
    assert_eq!(first.dependency_lock().tools.len(), 1);
    assert_eq!(second.dependency_lock().tools.len(), 2);
    assert_eq!(second.dependency_lock().packages.len(), 3);
    assert!(second
        .dependency_lock()
        .tools
        .iter()
        .all(|t| t.package_digest == world.fixture.base.package.archive.sha256));
    assert_ne!(
        second.dependency_lock().tools[0].consumer,
        second.dependency_lock().tools[1].consumer
    );
    assert_eq!(
        second.dependency_lock().verified_digest().unwrap(),
        second.dependency_lock().digest
    );
    assert_eq!(world.metadata("package-install-inventory-v1"), legacy);
    assert_eq!(world.metadata(KEY), raw);
    assert_eq!(
        fs::read_dir(world.root().join("artifacts"))
            .unwrap()
            .count(),
        1
    );
    let before = world.metadata(CONSUMERS);
    let repeat = install(&mut world, &consumer).unwrap();
    assert_eq!(repeat.installed_at(), second.installed_at());
    assert_eq!(world.metadata(CONSUMERS), before);
    world.reopen();
    assert_eq!(inspect(&mut world).unwrap().len(), 2);
    // This case intentionally installed one legacy provider. Keep the original
    // tool-only helper strict; assert execution effects separately here.
    for table in ["grants", "jobs", "plugin_data", "imports"] {
        let count: i64 = world
            .store
            .as_ref()
            .unwrap()
            .connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "{table}");
    }
    assert_eq!(world.metadata("package-install-inventory-v1"), legacy);
}
#[test]
fn missing_or_tampered_tool_bytes_never_become_a_consumer_lock() {
    for attack in ["missing", "bytes", "seal", "inventory"] {
        let mut world = world();
        if attack != "missing" {
            world.install().unwrap();
        }
        let input = input(&world, "consumer", vec![], true, 1);
        match attack {
            "bytes" | "seal" => {
                let path = world.blob().join(if attack == "bytes" {
                    "package.zip"
                } else {
                    "seal.json"
                });
                writable(&path);
                fs::write(&path, b"SUBSTITUTED").unwrap();
                let mut permissions = fs::metadata(&path).unwrap().permissions();
                permissions.set_readonly(true);
                fs::set_permissions(&path, permissions).unwrap();
            }
            "inventory" => set(&world, KEY, "CORRUPT"),
            _ => (),
        }
        assert!(install(&mut world, &input).is_err(), "{attack}");
        assert!(world.metadata(CONSUMERS).is_none());
        assert!(world.metadata(CONSUMER_PENDING).is_none());
        assert!(!world.path.join("tool-bound-plugin-store-v1").exists());
    }
}
#[test]
fn interrupted_consumer_installation_blocks_all_other_surfaces_until_explicit_abort() {
    for phase in [
        Phase::JournalCommitted,
        Phase::StagePrepared,
        Phase::ArtifactPublished,
        Phase::BeforeCommit,
        Phase::Committed,
    ] {
        let mut world = world();
        let source = world.install().unwrap();
        let input = input(&world, "consumer", vec![], true, 1);
        let raw = world.metadata(KEY);
        assert_eq!(
            with_hook(&mut world, &input, |observed, _| if observed == phase {
                Err(InstallError::Storage.into())
            } else {
                Ok(())
            })
            .err(),
            Some(ToolInstallError::Storage(InstallError::Storage))
        );
        world.reopen();
        if phase == Phase::Committed {
            assert!(!abort(&mut world).unwrap());
            assert_eq!(inspect(&mut world).unwrap().len(), 1);
        } else {
            let pending = world.metadata(CONSUMER_PENDING).unwrap();
            for text in [pending.as_str(), "CORRUPT"] {
                set(&world, CONSUMER_PENDING, text);
                let expected = Some(ToolInstallError::Storage(InstallError::RecoveryPending));
                assert_eq!(world.install().err(), expected);
                assert_eq!(world.pins().err(), expected);
                assert_eq!(world.abort().err(), expected);
                assert_eq!(inspect(&mut world).err(), expected);
                assert_eq!(install(&mut world, &input).err(), expected);
                let now = world.fixture.now();
                assert_eq!(
                    materialize_fixture_with_hook(
                        &world.trust,
                        world.store.as_mut().unwrap(),
                        &source,
                        || now,
                        |_, _| Ok(())
                    )
                    .err(),
                    expected
                );
                assert_eq!(
                    inspect_inert_tool_images(&world.trust, world.store.as_mut().unwrap()).err(),
                    expected
                );
                assert_eq!(
                    plan_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap())
                        .err(),
                    expected
                );
                assert_eq!(
                    plan_image_reconciliation(&world.trust, world.store.as_mut().unwrap()).err(),
                    expected
                );
                assert_eq!(
                    install::inspect_inert_inventory(&world.trust, world.store.as_mut().unwrap())
                        .err(),
                    Some(InstallError::RecoveryPending)
                );
                assert_eq!(world.metadata(CONSUMER_PENDING).as_deref(), Some(text));
            }
            set(&world, CONSUMER_PENDING, &pending);
            assert!(abort(&mut world).unwrap());
            assert!(!abort(&mut world).unwrap());
            assert!(world.metadata(CONSUMER_PENDING).is_none());
            assert!(inspect(&mut world).unwrap().is_empty());
            if phase != Phase::JournalCommitted {
                let id = serde_json::from_str::<Value>(&pending).unwrap()["id"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                let suffix = if phase == Phase::StagePrepared {
                    "stage"
                } else {
                    "artifact"
                };
                assert_eq!(
                    fs::read(
                        world
                            .path
                            .join("tool-bound-plugin-store-v1/quarantine")
                            .join(format!("{id}-{suffix}/package.zip"))
                    )
                    .unwrap(),
                    input.archive
                );
            }
        }
        assert_eq!(world.metadata(KEY), raw);
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
#[test]
fn missing_referenced_tool_inventory_or_consumer_lock_corruption_refuses_other_reconciliation() {
    for attack in ["raw", "consumer"] {
        let mut world = world();
        world.install().unwrap();
        let input = input(&world, "consumer", vec![], true, 1);
        install(&mut world, &input).unwrap();
        if attack == "raw" {
            remove(&world, KEY);
        } else {
            set(&world, CONSUMERS, "CORRUPT");
        }
        assert!(world.pins().is_err());
        assert!(inspect(&mut world).is_err());
        assert!(
            plan_tool_archive_reconciliation(&world.trust, world.store.as_mut().unwrap()).is_err()
        );
        assert!(plan_image_reconciliation(&world.trust, world.store.as_mut().unwrap()).is_err());
        assert!(
            install::inspect_inert_inventory(&world.trust, world.store.as_mut().unwrap()).is_err()
        );
        assert!(artifact(&world, &input).exists());
        assert!(world.blob().exists());
    }
}
#[test]
fn journal_insert_substitution_rolls_back_before_any_file_effect() {
    for key in [CONSUMERS, KEY, "package-trust-state-v1", CONSUMER_PENDING] {
        let mut world = world();
        world.install().unwrap();
        let input = input(&world, "consumer", vec![], true, 1);
        let original = world.metadata(key);
        world.store.as_ref().unwrap().connection.execute_batch(&format!("CREATE TRIGGER substitute AFTER INSERT ON core_metadata WHEN NEW.key='{CONSUMER_PENDING}' BEGIN INSERT INTO core_metadata(key,value) VALUES('{key}','CORRUPT') ON CONFLICT(key) DO UPDATE SET value=excluded.value; END;")).unwrap();
        assert!(install(&mut world, &input).is_err(), "{key}");
        // Descriptor admission may advance the existing trust observation, but
        // the trigger's replacement bytes must never commit.
        if key == "package-trust-state-v1" {
            assert_ne!(world.metadata(key).as_deref(), Some("CORRUPT"));
        } else {
            assert_eq!(world.metadata(key), original);
        }
        assert!(world.metadata(CONSUMER_PENDING).is_none());
        assert!(!artifact(&world, &input).exists());
        assert_eq!(
            fs::read_dir(world.path.join("tool-bound-plugin-store-v1/staging"))
                .unwrap()
                .count(),
            0
        );
    }
}
