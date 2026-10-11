use super::dependency_tests::{bytes, manifest};
use super::*;
use crate::{
    data::DataStore,
    dependencies::DependencyError,
    packages::{
        install::{
            abort_inert_install, inspect_inert_inventory, install_inert_plugin, install_with_hook,
            InertPackageBytes, InstallError, InstallPhase,
        },
        manifest::ManifestCompatibility,
        trust::PackageTrustStore,
    },
};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

const INVENTORY: &str = "package-install-inventory-v1";
const PENDING: &str = "package-install-pending-v1";
struct World {
    fixture: Fixture,
    trust: PackageTrustStore,
    store: Option<DataStore>,
    compatibility: ManifestCompatibility,
    path: PathBuf,
    cleanup: bool,
}
impl World {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "flowtools-validation-inert-install-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&path).unwrap();
        Self::open(&path, true)
    }
    fn open(path: &Path, initialize: bool) -> Self {
        let mut fixture = Fixture::new();
        let pins = BootstrapPins {
            keys: serde_json::from_value(fixture.golden["pins"].clone()).unwrap(),
            threshold: 2,
        };
        let trust = PackageTrustStore::new(pins.clone()).unwrap();
        let mut root = fixture.root.payload().clone();
        root.publishers[0].packages = ["consumer", "middle", "provider"]
            .iter()
            .map(|id| PublisherScope {
                kind: PackageKind::Plugin,
                id: (*id).into(),
                targets: vec![fixture.package.target.clone()],
            })
            .collect();
        let envelope = signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]);
        fixture.root = bootstrap_root(&envelope, &pins, fixture.policy.clock).unwrap();
        let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
        if initialize {
            trust
                .bootstrap(&mut store, &envelope, fixture.policy.clock.now)
                .unwrap();
        }
        let compatibility = ManifestCompatibility {
            host_version: "0.1.0".into(),
            sdk_version: "0.0.0".into(),
            target: fixture.package.target.clone(),
        };
        Self {
            fixture,
            trust,
            store: Some(store),
            compatibility,
            path: path.into(),
            cleanup: initialize,
        }
    }
    fn input(&self, value: Value, sequence: u64) -> Input {
        let (payload, manifest, archive) = bytes(&self.fixture, value, sequence);
        Input {
            envelope: signed(PACKAGE_PAYLOAD_TYPE, &payload, &[3]),
            payload,
            manifest,
            archive,
        }
    }
    fn install(
        &mut self,
        input: &Input,
    ) -> Result<crate::packages::install::InstalledInertPackage, InstallError> {
        let now = self.fixture.policy.clock.now;
        install_inert_plugin(
            &self.trust,
            self.store.as_mut().unwrap(),
            input.borrow(),
            &self.compatibility,
            || now,
        )
    }
    fn inspect(
        &mut self,
    ) -> Result<crate::packages::install::InertInventorySnapshot, InstallError> {
        inspect_inert_inventory(&self.trust, self.store.as_mut().unwrap())
    }
    fn abort(&mut self) -> Result<bool, InstallError> {
        abort_inert_install(&self.trust, self.store.as_mut().unwrap())
    }
    fn raw(&self, key: &str) -> Option<String> {
        self.store
            .as_ref()
            .unwrap()
            .connection
            .query_row("SELECT value FROM core_metadata WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()
            .unwrap()
    }
    fn reopen(&mut self) {
        drop(self.store.take());
        self.store = Some(DataStore::open(&self.path.join("runtime.sqlite")).unwrap());
    }
    fn artifact(&self, input: &Input) -> PathBuf {
        self.path
            .join("package-store-v1/artifacts")
            .join(&input.payload.archive.sha256)
    }
}
use rusqlite::OptionalExtension;
impl Drop for World {
    fn drop(&mut self) {
        drop(self.store.take());
        if self.cleanup {
            // Only this test's UUID profile; never recurse through a junction.
            fn writable(path: &Path) {
                let Ok(metadata) = std::fs::symlink_metadata(path) else {
                    return;
                };
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return;
                }
                if metadata.is_dir() {
                    for entry in std::fs::read_dir(path).unwrap() {
                        writable(&entry.unwrap().path());
                    }
                } else {
                    writable_file(path);
                }
            }
            assert!(self.path.parent().unwrap() == std::env::temp_dir());
            assert!(self
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("flowtools-validation-inert-install-"));
            writable(&self.path);
            std::fs::remove_dir_all(&self.path).unwrap();
        }
    }
}
struct Input {
    payload: PackagePayload,
    envelope: Vec<u8>,
    manifest: Vec<u8>,
    archive: Vec<u8>,
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
fn dependency(id: &str) -> Value {
    json!({"publisher":"fixture-publisher","id":id,"version":"^1.0.0","service":"text-transform","interfaceVersion":"^1.0.0"})
}
fn revision() -> Value {
    let mut value = manifest("consumer", vec![]);
    value["version"] = json!("1.1.0");
    value
}
fn change(path: &Path, bytes: &[u8]) {
    writable_file(path);
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    std::fs::write(path, bytes).unwrap();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).unwrap();
}
fn writable_file(path: &Path) {
    use std::os::windows::{ffi::OsStrExt, fs::MetadataExt};
    use windows_sys::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_READONLY};
    let attributes = std::fs::metadata(path).unwrap().file_attributes() & !FILE_ATTRIBUTE_READONLY;
    let name: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    assert_ne!(unsafe { SetFileAttributesW(name.as_ptr(), attributes) }, 0);
}

#[test]
fn real_signed_packages_install_inert_blobs_and_locks_in_the_existing_writer() {
    let mut world = World::new();
    let provider = world.input(manifest("provider", vec![]), 1);
    let middle = world.input(manifest("middle", vec![dependency("provider")]), 1);
    let consumer = world.input(manifest("consumer", vec![dependency("middle")]), 1);
    world.install(&provider).unwrap();
    world.install(&middle).unwrap();
    let receipt = world.install(&consumer).unwrap();
    assert_eq!(receipt.pin().digest, consumer.payload.archive.sha256);
    assert_eq!(receipt.dependency_lock().packages.len(), 3);
    assert_eq!(receipt.dependency_lock().services.len(), 2);
    assert_eq!(
        receipt
            .dependency_lock()
            .topology
            .iter()
            .map(|id| id.id.as_str())
            .collect::<Vec<_>>(),
        vec!["provider", "middle", "consumer"]
    );
    let original = world.raw(INVENTORY).unwrap();
    assert!(world.raw(PENDING).is_none());
    world.reopen();
    assert_eq!(world.inspect().unwrap().packages().len(), 3);
    let repeated = world.install(&consumer).unwrap();
    assert_eq!(repeated.installed_at(), receipt.installed_at());
    assert_eq!(world.raw(INVENTORY).unwrap(), original);
    for input in [&provider, &middle, &consumer] {
        let path = world.artifact(input);
        assert_eq!(
            std::fs::read(path.join("manifest.json")).unwrap(),
            input.manifest
        );
        assert_eq!(
            std::fs::read(path.join("package.zip")).unwrap(),
            input.archive
        );
        assert!(std::fs::metadata(path.join("package.zip"))
            .unwrap()
            .permissions()
            .readonly());
        assert_eq!(std::fs::read_dir(path).unwrap().count(), 3);
    }
    for table in ["grants", "jobs", "plugin_data", "imports"] {
        let count: i64 = world
            .store
            .as_ref()
            .unwrap()
            .connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
    assert_eq!(
        DataStore::open(&world.path.join("runtime.sqlite")).err(),
        Some(crate::protocol::ErrorCode::StoreBusy)
    );
}

#[test]
fn rejected_signature_archive_manifest_dependency_and_target_do_not_stage_or_install() {
    for mode in [
        "signature",
        "zip",
        "manifest",
        "missing",
        "tools",
        "platform",
    ] {
        let mut world = World::new();
        let mut value = manifest(
            "consumer",
            if mode == "missing" {
                vec![dependency("provider")]
            } else {
                vec![]
            },
        );
        if mode == "manifest" {
            value["commands"][0]["headless"] = json!(false);
            value["commands"][0]["supportsColdStart"] = json!(true);
        }
        if mode == "tools" {
            value["dependencies"]["tools"] =
                json!([{"publisher":"fixture-publisher","id":"fixture-tool","version":"*"}]);
        }
        let mut input = world.input(value, 1);
        if mode == "signature" {
            input.envelope = signed(PACKAGE_PAYLOAD_TYPE, &input.payload, &[4]);
        }
        if mode == "zip" {
            input.archive[0] ^= 1;
        }
        if mode == "platform" {
            world.compatibility.target.arch = PackageArch::Arm64;
        }
        assert!(world.install(&input).is_err(), "{mode}");
        assert!(world.raw(INVENTORY).is_none());
        assert!(world.raw(PENDING).is_none());
        assert!(!world.path.join("package-store-v1").exists());
    }
}

#[test]
fn process_crash_child() {
    let Ok(profile) = std::env::var("FLOWTOOLS_TEST_INERT_INSTALL_PROFILE") else {
        return;
    };
    let path = PathBuf::from(profile);
    assert_eq!(path.parent().unwrap(), std::env::temp_dir());
    assert!(path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("flowtools-validation-inert-install-"));
    let mut world = World::open(&path, false);
    let input = world.input(revision(), 2);
    let requested = std::env::var("FLOWTOOLS_TEST_INERT_INSTALL_PHASE").unwrap();
    let now = world.fixture.policy.clock.now;
    install_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        input.borrow(),
        &world.compatibility,
        || now,
        |phase, _| {
            if format!("{phase:?}") == requested {
                std::process::exit(21);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("requested crash barrier was not reached");
}

#[test]
fn real_process_exit_at_all_install_barriers_preserves_old_inventory_or_atomic_commit() {
    for phase in [
        InstallPhase::JournalCommitted,
        InstallPhase::StagePrepared,
        InstallPhase::ArtifactPublished,
        InstallPhase::BeforeCommit,
        InstallPhase::Committed,
    ] {
        let mut world = World::new();
        let first = world.input(manifest("consumer", vec![]), 1);
        world.install(&first).unwrap();
        let before = world.raw(INVENTORY).unwrap();
        let original_bytes = std::fs::read(world.artifact(&first).join("package.zip")).unwrap();
        drop(world.store.take());
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "packages::tests::install_tests::process_crash_child",
                "--nocapture",
            ])
            .env("FLOWTOOLS_TEST_INERT_INSTALL_PROFILE", &world.path)
            .env("FLOWTOOLS_TEST_INERT_INSTALL_PHASE", format!("{phase:?}"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(21),
            "{phase:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        world.reopen();
        let candidate = world.input(revision(), 2);
        if phase == InstallPhase::Committed {
            assert!(world.raw(PENDING).is_none());
            assert_eq!(world.inspect().unwrap().packages().len(), 2);
            assert!(!world.abort().unwrap());
        } else {
            assert_eq!(world.raw(INVENTORY).unwrap(), before);
            assert_eq!(world.inspect().err(), Some(InstallError::RecoveryPending));
            assert_eq!(
                world.install(&candidate).err(),
                Some(InstallError::RecoveryPending)
            );
            assert!(world.abort().unwrap());
            assert!(!world.abort().unwrap());
            assert_eq!(world.raw(INVENTORY).unwrap(), before);
            assert!(!world.artifact(&candidate).exists());
            assert_eq!(
                world.install(&first).err(),
                Some(InstallError::Package(PackageError::ReleaseRollback))
            );
            assert_eq!(world.inspect().unwrap().packages().len(), 1);
            world.install(&candidate).unwrap();
            assert_eq!(world.inspect().unwrap().packages().len(), 2);
        }
        assert_eq!(
            std::fs::read(world.artifact(&first).join("package.zip")).unwrap(),
            original_bytes
        );
    }
}

#[test]
fn write_failures_keep_inventory_and_journal_recovery_is_idempotent_after_quarantine() {
    for gate in ["journal", "prepared", "inventory", "abort"] {
        let mut world = World::new();
        let input = world.input(manifest("consumer", vec![]), 1);
        let sql = match gate {
            "journal" => "CREATE TRIGGER write_fault BEFORE INSERT ON core_metadata WHEN NEW.key='package-install-pending-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END",
            "prepared" => "CREATE TRIGGER write_fault BEFORE UPDATE ON core_metadata WHEN NEW.key='package-install-pending-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END",
            "inventory" => "CREATE TRIGGER write_fault BEFORE INSERT ON core_metadata WHEN NEW.key='package-install-inventory-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END",
            _ => "CREATE TRIGGER write_fault BEFORE DELETE ON core_metadata WHEN OLD.key='package-install-pending-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END",
        };
        world
            .store
            .as_ref()
            .unwrap()
            .connection
            .execute_batch(sql)
            .unwrap();
        assert_eq!(
            world.install(&input).err(),
            Some(InstallError::Storage),
            "{gate}"
        );
        assert!(world.raw(INVENTORY).is_none());
        if gate == "abort" {
            assert_eq!(world.abort().err(), Some(InstallError::Storage));
            assert!(!world.artifact(&input).exists());
            assert_eq!(
                std::fs::read_dir(world.path.join("package-store-v1/quarantine"))
                    .unwrap()
                    .count(),
                1
            );
        }
        world
            .store
            .as_ref()
            .unwrap()
            .connection
            .execute_batch("DROP TRIGGER write_fault")
            .unwrap();
        if gate == "journal" {
            assert!(world.raw(PENDING).is_none());
        } else {
            assert!(world.abort().unwrap());
            assert!(world.raw(PENDING).is_none());
        }
        world.install(&input).unwrap();
        assert_eq!(world.inspect().unwrap().packages().len(), 1);
    }
}

#[test]
fn commit_resamples_time_and_rechecks_revocation_and_exact_installed_provider_bytes() {
    for mode in ["expiry", "revoked", "provider"] {
        let mut world = World::new();
        let provider = world.input(manifest("provider", vec![]), 1);
        if mode == "provider" {
            world.install(&provider).unwrap();
        }
        let input = world.input(
            manifest(
                "consumer",
                if mode == "provider" {
                    vec![dependency("provider")]
                } else {
                    vec![]
                },
            ),
            1,
        );
        let before = world.raw(INVENTORY);
        let now = world.fixture.policy.clock.now;
        let mut calls = 0;
        let clock = || {
            calls += 1;
            if mode == "expiry" && calls == 2 {
                input.payload.expires_at
            } else {
                now
            }
        };
        let trust = &world.trust;
        let mut root = world.fixture.root.payload().clone();
        root.version += 1;
        root.previous_root_sha256 = Some(world.fixture.root.payload_digest().into());
        root.revoked_package_digests
            .push(input.payload.archive.sha256.clone());
        let provider_file = world.artifact(&provider).join("package.zip");
        let error = install_with_hook(
            trust,
            world.store.as_mut().unwrap(),
            input.borrow(),
            &world.compatibility,
            clock,
            |phase, store| {
                if phase == InstallPhase::BeforeCommit {
                    if mode == "revoked" {
                        trust
                            .rotate(store, &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]), now)
                            .unwrap();
                    }
                    if mode == "provider" {
                        change(&provider_file, b"changed fixture");
                    }
                }
                Ok(())
            },
        )
        .err();
        assert_eq!(
            error,
            Some(if mode == "expiry" {
                InstallError::Package(PackageError::TrustExpired)
            } else if mode == "revoked" {
                InstallError::Package(PackageError::PackageRevoked)
            } else {
                InstallError::ArtifactInvalid
            })
        );
        assert_eq!(world.raw(INVENTORY), before);
        assert!(world.raw(PENDING).is_some());
        if mode == "provider" {
            change(&provider_file, &provider.archive);
        }
        assert!(world.abort().unwrap());
    }
}

#[test]
fn provider_updates_retain_old_artifacts_and_do_not_silently_rebind_existing_transitive_locks() {
    let mut world = World::new();
    let first = world.input(manifest("provider", vec![]), 1);
    let middle = world.input(manifest("middle", vec![dependency("provider")]), 1);
    world.install(&first).unwrap();
    world.install(&middle).unwrap();
    let mut value = manifest("provider", vec![]);
    value["version"] = json!("1.1.0");
    let second = world.input(value, 2);
    world.install(&second).unwrap();
    assert!(world.artifact(&first).exists() && world.artifact(&second).exists());
    let consumer = world.input(manifest("consumer", vec![dependency("middle")]), 1);
    assert_eq!(
        world.install(&consumer).err(),
        Some(InstallError::Dependency(DependencyError::VersionConflict))
    );
    assert!(world.raw(PENDING).is_none());
    let relocked = world.install(&middle).unwrap();
    assert!(relocked
        .dependency_lock()
        .packages
        .iter()
        .any(|pin| pin.digest == second.payload.archive.sha256));
    let receipt = world.install(&consumer).unwrap();
    assert!(receipt
        .dependency_lock()
        .packages
        .iter()
        .any(|pin| pin.digest == second.payload.archive.sha256));
    assert!(!receipt
        .dependency_lock()
        .packages
        .iter()
        .any(|pin| pin.digest == first.payload.archive.sha256));
}

#[test]
fn hardlinks_tampered_blobs_and_foreign_staging_are_refused_without_adoption() {
    for mode in ["hardlink", "tamper", "foreign"] {
        let mut world = World::new();
        let input = world.input(manifest("consumer", vec![]), 1);
        world.install(&input).unwrap();
        let before = world.raw(INVENTORY);
        let path = world.artifact(&input).join("package.zip");
        match mode {
            "hardlink" => std::fs::hard_link(&path, world.path.join("fixture-alias.zip")).unwrap(),
            "tamper" => change(&path, b"hostile fixture"),
            _ => std::fs::create_dir(world.path.join("package-store-v1/staging/foreign")).unwrap(),
        }
        assert_eq!(
            world.inspect().err(),
            Some(match mode {
                "hardlink" => InstallError::PathRejected,
                "tamper" => InstallError::ArtifactInvalid,
                _ => InstallError::StateInvalid,
            })
        );
        assert_eq!(world.raw(INVENTORY), before);
        assert!(world.raw(PENDING).is_none());
    }
}

#[test]
fn directory_handles_block_replacement_and_junctions_cannot_redirect_installation() {
    let mut world = World::new();
    let input = world.input(manifest("consumer", vec![]), 1);
    let root = world.path.join("package-store-v1");
    let renamed = world.path.join("fixture-renamed");
    let now = world.fixture.policy.clock.now;
    install_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        input.borrow(),
        &world.compatibility,
        || now,
        |phase, _| {
            if [InstallPhase::StagePrepared, InstallPhase::BeforeCommit].contains(&phase) {
                for path in [
                    &root,
                    &root.join("staging"),
                    &root.join("artifacts"),
                    &root.join("quarantine"),
                ] {
                    assert!(std::fs::rename(path, &renamed).is_err());
                }
            }
            Ok(())
        },
    )
    .unwrap();
    std::fs::rename(&root, &renamed).unwrap();
    let outside = world.path.join("fixture-outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("canary.txt"), b"UNCHANGED").unwrap();
    let result = Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&root)
        .arg(&outside)
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(world.inspect().err(), Some(InstallError::PathRejected));
    assert_eq!(
        std::fs::read(outside.join("canary.txt")).unwrap(),
        b"UNCHANGED"
    );
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1);
    std::fs::remove_dir(&root).unwrap();
    std::fs::rename(&renamed, &root).unwrap();
    assert_eq!(world.inspect().unwrap().packages().len(), 1);
}

#[test]
fn corrupt_or_changed_journals_and_inventory_refuse_to_commit_or_recover() {
    for mode in ["journal", "inventory"] {
        let mut world = World::new();
        let input = world.input(manifest("consumer", vec![]), 1);
        let now = world.fixture.policy.clock.now;
        let error = install_with_hook(
            &world.trust,
            world.store.as_mut().unwrap(),
            input.borrow(),
            &world.compatibility,
            || now,
            |phase, store| {
                if phase == InstallPhase::BeforeCommit {
                    if mode == "journal" {
                        let text: String = store
                            .connection
                            .query_row(
                                "SELECT value FROM core_metadata WHERE key=?1",
                                [PENDING],
                                |r| r.get(0),
                            )
                            .unwrap();
                        let mut value: Value = serde_json::from_str(&text).unwrap();
                        value["id"] = json!(uuid::Uuid::new_v4().to_string());
                        store
                            .connection
                            .execute(
                                "UPDATE core_metadata SET value=?1 WHERE key=?2",
                                [value.to_string(), PENDING.into()],
                            )
                            .unwrap();
                    } else {
                        store
                            .connection
                            .execute(
                                "INSERT INTO core_metadata(key,value) VALUES(?1,'{}')",
                                [INVENTORY],
                            )
                            .unwrap();
                    }
                }
                Ok(())
            },
        )
        .err();
        assert!(matches!(
            error,
            Some(InstallError::InventoryChanged | InstallError::StateInvalid)
        ));
        assert!(world.raw(PENDING).is_some());
        assert!(world.abort().is_err());
    }
}

#[test]
fn sqlite_backup_restore_keeps_installation_and_trust_recovery_required() {
    let mut world = World::new();
    let input = world.input(manifest("consumer", vec![]), 1);
    world.install(&input).unwrap();
    let backup = world.store.as_ref().unwrap().backup().unwrap();
    let old = world.raw(INVENTORY);
    let candidate = world.input(revision(), 2);
    world.install(&candidate).unwrap();
    drop(world.store.take());
    DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
    world.reopen();
    assert_eq!(
        world.inspect().err(),
        Some(InstallError::Package(PackageError::TrustRecoveryRequired))
    );
    assert_eq!(
        world.install(&input).err(),
        Some(InstallError::Package(PackageError::TrustRecoveryRequired))
    );
    assert_eq!(
        world.abort().err(),
        Some(InstallError::Package(PackageError::TrustRecoveryRequired))
    );
    assert_eq!(world.raw(INVENTORY), old);
    assert!(world.raw("package-trust-recovery-required").is_some());
}

#[test]
fn descriptor_renewal_preserves_immutable_blobs_and_same_version_changed_hash_is_refused() {
    let mut world = World::new();
    let input = world.input(manifest("consumer", vec![]), 1);
    world.install(&input).unwrap();
    let seal = std::fs::read(world.artifact(&input).join("seal.json")).unwrap();
    let mut renewed = world.input(manifest("consumer", vec![]), 1);
    renewed.payload.expires_at += 1;
    renewed.envelope = signed(PACKAGE_PAYLOAD_TYPE, &renewed.payload, &[3]);
    world.install(&renewed).unwrap();
    assert_eq!(world.inspect().unwrap().packages().len(), 1);
    assert_eq!(
        std::fs::read(world.artifact(&input).join("seal.json")).unwrap(),
        seal
    );
    assert_eq!(
        std::fs::read_dir(world.path.join("package-store-v1/staging"))
            .unwrap()
            .count(),
        0
    );
    let mut value = manifest("consumer", vec![]);
    value["name"] = json!("Changed immutable ZIP");
    let changed = world.input(value, 1);
    let old = world.raw(INVENTORY);
    assert_eq!(
        world.install(&changed).err(),
        Some(InstallError::Package(PackageError::VersionConflict))
    );
    assert_eq!(world.raw(INVENTORY), old);
    assert!(!world.artifact(&changed).exists());
    assert!(world.raw(PENDING).is_none());
}

#[test]
fn state_read_and_physical_retention_budgets_fail_without_staging_or_metadata_changes() {
    for mode in ["state", "groups", "reserved-group", "disk"] {
        let mut world = World::new();
        let input = world.input(manifest("consumer", vec![]), 1);
        world.install(&input).unwrap();
        let old = world.raw(INVENTORY);
        if mode == "state" {
            world
                .store
                .as_ref()
                .unwrap()
                .connection
                .execute(
                    "UPDATE core_metadata SET value=?1 WHERE key=?2",
                    [" ".repeat(16_777_217), INVENTORY.into()],
                )
                .unwrap();
        } else {
            let count = match mode {
                "groups" => 384,
                "reserved-group" => 383,
                _ => 4,
            };
            for _ in 0..count {
                let dir = world
                    .path
                    .join("package-store-v1/quarantine")
                    .join(format!("{}-stage", uuid::Uuid::new_v4()));
                std::fs::create_dir(&dir).unwrap();
                if mode == "disk" {
                    std::fs::File::create(dir.join("package.zip"))
                        .unwrap()
                        .set_len(MAX_ARCHIVE_BYTES)
                        .unwrap();
                }
            }
        }
        if mode == "reserved-group" {
            assert_eq!(world.inspect().unwrap().packages().len(), 1);
        } else {
            assert_eq!(
                world.inspect().err(),
                Some(if mode == "state" {
                    InstallError::StateInvalid
                } else {
                    InstallError::BudgetExceeded
                })
            );
        }
        let candidate = world.input(revision(), 2);
        assert!(world.install(&candidate).is_err());
        assert!(world.raw(PENDING).is_none());
        assert!(!world.artifact(&candidate).exists());
        if mode != "state" {
            assert_eq!(world.raw(INVENTORY), old);
        }
    }
}

#[test]
fn repeated_raw_keys_unknown_metadata_and_mismatched_seals_are_rejected() {
    for mode in ["duplicate", "unknown", "seal", "missing", "observation"] {
        let mut world = World::new();
        let input = world.input(manifest("consumer", vec![]), 1);
        world.install(&input).unwrap();
        if mode == "observation" {
            let original = world.raw("package-trust-state-v1").unwrap();
            let mut state: Value = serde_json::from_str(&original).unwrap();
            state["knownVersions"] = json!([]);
            world
                .store
                .as_ref()
                .unwrap()
                .connection
                .execute(
                    "UPDATE core_metadata SET value=?1 WHERE key='package-trust-state-v1'",
                    [state.to_string()],
                )
                .unwrap();
        } else if mode == "seal" {
            change(&world.artifact(&input).join("seal.json"), b"{}");
        } else if mode == "missing" {
            let path = world.artifact(&input).join("manifest.json");
            writable_file(&path);
            std::fs::remove_file(path).unwrap();
        } else {
            let original = world.raw(INVENTORY).unwrap();
            let text = if mode == "duplicate" {
                original.replacen('{', "{\"formatVersion\":1,", 1)
            } else {
                original.replacen('{', "{\"allowUnverified\":true,", 1)
            };
            world
                .store
                .as_ref()
                .unwrap()
                .connection
                .execute(
                    "UPDATE core_metadata SET value=?1 WHERE key=?2",
                    [text, INVENTORY.into()],
                )
                .unwrap();
        }
        assert!(world.inspect().is_err());
        assert!(world.raw(PENDING).is_none());
    }
}

#[test]
fn valid_compressed_provider_pool_cannot_exceed_the_aggregate_decoding_budget() {
    use std::io::{Cursor, Write};
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};
    let mut world = World::new();
    let mut seed = 17u32;
    let chunk: Vec<u8> = (0..4096)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as u8
        })
        .collect();
    let source = chunk.repeat(8192);
    for id in ["provider", "middle"] {
        let mut value = manifest(id, vec![]);
        let mut payload = world.fixture.package.clone();
        payload.id = id.into();
        payload.version = "1.0.0".into();
        payload.release_sequence = 1;
        payload.files = ["dist/commands.js", "dist/services.js"]
            .iter()
            .map(|path| PackageFile {
                path: (*path).into(),
                sha256: hash(&source),
                size: source.len() as u64,
            })
            .collect();
        value["files"] = serde_json::to_value(&payload.files).unwrap();
        let manifest = serde_json::to_vec(&value).unwrap();
        payload.manifest = BlobDescriptor {
            sha256: hash(&manifest),
            size: manifest.len() as u64,
        };
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in [
            ("manifest.json", manifest.as_slice()),
            ("dist/commands.js", source.as_slice()),
            ("dist/services.js", source.as_slice()),
        ] {
            zip.start_file(
                name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .unwrap();
            zip.write_all(bytes).unwrap();
        }
        let archive = zip.finish().unwrap().into_inner();
        assert!(archive.len() < 1_048_576);
        payload.archive.sha256 = hash(&archive);
        payload.archive.size = archive.len() as u64;
        let input = Input {
            envelope: signed(PACKAGE_PAYLOAD_TYPE, &payload, &[3]),
            payload,
            manifest,
            archive,
        };
        world.install(&input).unwrap();
    }
    let old = world.raw(INVENTORY);
    let input = world.input(
        manifest(
            "consumer",
            vec![dependency("provider"), dependency("middle")],
        ),
        1,
    );
    assert_eq!(
        world.install(&input).err(),
        Some(InstallError::BudgetExceeded)
    );
    assert_eq!(world.raw(INVENTORY), old);
    assert!(world.raw(PENDING).is_none());
    assert!(!world.artifact(&input).exists());
}
