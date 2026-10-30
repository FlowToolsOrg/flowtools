//! Actual Windows inert ZIP transactions using the existing signed ToolFixture.
mod image_tests;
mod reconciliation_tests;
use super::*;
use crate::packages::{
    install::{inspect_inert_inventory, InstallError},
    tools::install::{
        abort_inert_tool_install, inspect_inert_tools, install_fixture_with_hook,
        install_inert_tool, InstalledInertTool, ToolInstallError, ToolInstallPhase,
    },
};
use rusqlite::OptionalExtension;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
const KEY: &str = "tool-archive-inventory-v1";
const PENDING: &str = "tool-archive-install-pending-v1";
struct World {
    fixture: ToolFixture,
    trust: PackageTrustStore,
    store: Option<DataStore>,
    path: PathBuf,
    cleanup: bool,
}
impl World {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("flowtools-tool-install-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self::open(&path, true, true)
    }
    fn open(path: &Path, bootstrap: bool, cleanup: bool) -> Self {
        let fixture = ToolFixture::new(CompressionMethod::Stored);
        let trust = fixture.trust();
        let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
        if bootstrap {
            fixture.bootstrap(&mut store, &trust);
        }
        Self {
            fixture,
            trust,
            store: Some(store),
            path: path.into(),
            cleanup,
        }
    }
    fn install(&mut self) -> Result<InstalledInertTool, ToolInstallError> {
        self.with_hook(|_, _| Ok(()))
    }
    fn with_hook(
        &mut self,
        hook: impl FnMut(ToolInstallPhase, &mut DataStore) -> Result<(), ToolInstallError>,
    ) -> Result<InstalledInertTool, ToolInstallError> {
        let bytes = signed(PACKAGE_PAYLOAD_TYPE, &self.fixture.base.package, &[3]);
        install_fixture_with_hook(
            &self.trust,
            self.store.as_mut().unwrap(),
            self.fixture.input(&bytes),
            || self.fixture.now(),
            hook,
        )
    }
    fn pins(
        &mut self,
    ) -> Result<Vec<crate::packages::tools::install::ToolArchivePin>, ToolInstallError> {
        inspect_inert_tools(&self.trust, self.store.as_mut().unwrap())
    }
    fn abort(&mut self) -> Result<bool, ToolInstallError> {
        abort_inert_tool_install(&self.trust, self.store.as_mut().unwrap())
    }
    fn root(&self) -> PathBuf {
        self.path.join("tool-package-store-v1")
    }
    fn blob(&self) -> PathBuf {
        self.root()
            .join("artifacts")
            .join(&self.fixture.base.package.archive.sha256)
    }
    fn metadata(&self, key: &str) -> Option<String> {
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
}
impl Drop for World {
    fn drop(&mut self) {
        drop(self.store.take());
        if !self.cleanup {
            return;
        }
        assert_eq!(self.path.parent().unwrap(), std::env::temp_dir());
        assert!(self
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("flowtools-tool-install-")
            .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok_and(|v| v.to_string() == s)));
        fs::remove_dir_all(&self.path).unwrap();
    }
}
#[test]
fn production_profile_refusal_never_creates_a_tool_pool_journal_or_inventory() {
    let mut world = World::new();
    let bytes = signed(PACKAGE_PAYLOAD_TYPE, &world.fixture.base.package, &[3]);
    let result = install_inert_tool(
        &world.trust,
        world.store.as_mut().unwrap(),
        world.fixture.input(&bytes),
        || world.fixture.now(),
    );
    assert!(matches!(
        result,
        Err(ToolInstallError::Admission(
            ToolAdmissionError::PolicyDenied
        ))
    ));
    assert!(!world.root().exists());
    assert!(world.metadata(KEY).is_none());
    assert!(world.metadata(PENDING).is_none());
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn signed_archive_installation_is_durable_shared_and_does_not_extract_programs() {
    let mut world = World::new();
    let first = world.install().unwrap();
    let pin = first.pin();
    assert_eq!(
        first.programs(),
        &[ToolProgram::Ffmpeg, ToolProgram::Ffprobe]
    );
    assert_eq!(
        pin.archive_digest,
        world.fixture.base.package.archive.sha256
    );
    assert_eq!(
        fs::read(world.blob().join("package.zip")).unwrap(),
        world.fixture.base.archive
    );
    let mut names = fs::read_dir(world.blob())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["manifest.json", "package.zip", "seal.json"]);
    assert!(!world.root().join("bin").exists());
    let original_seal = fs::read(world.blob().join("seal.json")).unwrap();
    let original_metadata = world.metadata(KEY).unwrap();
    let bytes = signed(PACKAGE_PAYLOAD_TYPE, &world.fixture.base.package, &[3]);
    let second = install_fixture_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        world.fixture.input(&bytes),
        || world.fixture.now() + 1,
        |_, _| Ok(()),
    )
    .unwrap();
    assert_eq!(second.pin(), pin);
    assert_eq!(second.installed_at(), first.installed_at());
    assert_eq!(
        world.metadata(KEY).as_deref(),
        Some(original_metadata.as_str())
    );
    assert_eq!(
        fs::read_dir(world.root().join("artifacts"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        fs::read(world.blob().join("seal.json")).unwrap(),
        original_seal
    );
    world.reopen();
    assert_eq!(world.pins().unwrap(), [pin]);
    assert!(world.metadata(PENDING).is_none());
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn pending_tool_transaction_blocks_both_inventories_and_explicit_abort_preserves_every_byte() {
    let mut world = World::new();
    let result = world.with_hook(|phase, store| {
        if phase == ToolInstallPhase::ArtifactPublished {
            assert_eq!(
                inspect_inert_inventory(&world_trust(), store).err(),
                Some(InstallError::RecoveryPending)
            );
            return Err(InstallError::Storage.into());
        }
        Ok(())
    });
    assert!(result.is_err());
    assert!(world.blob().exists());
    assert_eq!(
        world.pins().unwrap_err(),
        ToolInstallError::Storage(InstallError::RecoveryPending)
    );
    assert!(matches!(
        world.install(),
        Err(ToolInstallError::Storage(InstallError::RecoveryPending))
    ));
    let original = fs::read(world.blob().join("package.zip")).unwrap();
    assert!(world.abort().unwrap());
    assert!(!world.abort().unwrap());
    assert!(!world.blob().exists());
    assert!(world.pins().unwrap().is_empty());
    let mut blobs = Vec::new();
    for group in fs::read_dir(world.root().join("quarantine")).unwrap() {
        let path = group.unwrap().path().join("package.zip");
        if path.is_file() {
            blobs.push(fs::read(path).unwrap());
        }
    }
    assert_eq!(blobs, [original]);
    assert_no_effects(world.store.as_ref().unwrap());
}
fn world_trust() -> PackageTrustStore {
    ToolFixture::new(CompressionMethod::Stored).trust()
}
fn writable(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_READONLY, INVALID_FILE_ATTRIBUTES,
    };
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let attributes = unsafe { GetFileAttributesW(path.as_ptr()) };
    assert_ne!(attributes, INVALID_FILE_ATTRIBUTES);
    assert_ne!(
        unsafe { SetFileAttributesW(path.as_ptr(), attributes & !FILE_ATTRIBUTE_READONLY) },
        0
    );
}
#[test]
fn existing_plugin_or_reconciliation_pending_bytes_refuse_tools_without_resetting_them() {
    for key in [
        "package-install-pending-v1",
        "package-reconciliation-pending-v1",
    ] {
        let mut world = World::new();
        let original = "CORRUPT_PENDING_BYTES_MUST_SURVIVE";
        world
            .store
            .as_mut()
            .unwrap()
            .connection
            .execute(
                "INSERT INTO core_metadata(key,value) VALUES(?1,?2)",
                rusqlite::params![key, original],
            )
            .unwrap();
        assert!(matches!(
            world.install(),
            Err(ToolInstallError::Storage(InstallError::RecoveryPending))
        ));
        assert_eq!(world.metadata(key).as_deref(), Some(original));
        assert!(!world.root().exists());
    }
}
#[test]
fn corrupt_inventory_and_unknown_fields_cannot_bootstrap_or_discard_bytes() {
    for text in [
        "{BROKEN",
        r#"{"formatVersion":1,"formatVersion":1}"#,
        r#"{"formatVersion":0,"profileDigest":"x","records":[]}"#,
    ] {
        let mut world = World::new();
        world
            .store
            .as_mut()
            .unwrap()
            .connection
            .execute(
                "INSERT INTO core_metadata(key,value) VALUES(?1,?2)",
                rusqlite::params![KEY, text],
            )
            .unwrap();
        assert!(world.install().is_err());
        assert!(world.pins().is_err());
        assert!(world.abort().is_err());
        assert_eq!(world.metadata(KEY).as_deref(), Some(text));
        assert!(!world.root().exists());
    }
}
#[test]
fn sql_commit_failure_keeps_pending_archive_until_explicit_abort_and_never_publishes_inventory() {
    let mut world = World::new();
    let result=world.with_hook(|phase,store|{if phase==ToolInstallPhase::BeforeCommit {
        store.connection.execute_batch("CREATE TRIGGER refuse_tool_inventory BEFORE INSERT ON core_metadata WHEN NEW.key='tool-archive-inventory-v1' BEGIN SELECT RAISE(ABORT,'fixture commit failure'); END;").unwrap();}Ok(())});
    assert!(result.is_err());
    assert!(world.metadata(KEY).is_none());
    assert!(world.metadata(PENDING).is_some());
    assert_eq!(
        fs::read(world.blob().join("package.zip")).unwrap(),
        world.fixture.base.archive
    );
    assert!(world.abort().unwrap());
    assert!(world.pins().unwrap().is_empty());
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn changed_inert_zip_or_new_signed_hash_for_the_same_version_cannot_be_adopted() {
    let mut world = World::new();
    world.install().unwrap();
    let metadata = world.metadata(KEY).unwrap();
    let path = world.blob().join("package.zip");
    writable(&path);
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    fs::write(&path, b"CHANGED_ARCHIVE").unwrap();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    assert!(world.pins().is_err());
    assert!(world.install().is_err());
    assert_eq!(world.metadata(KEY).as_deref(), Some(metadata.as_str()));
    writable(&path);
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    fs::write(&path, &world.fixture.base.archive).unwrap();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    world
        .fixture
        .contents
        .get_mut("bin/ffmpeg.exe")
        .unwrap()
        .push(1);
    world.fixture.rebuild();
    assert!(world.install().is_err());
    assert_eq!(world.metadata(KEY).as_deref(), Some(metadata.as_str()));
    assert!(world.metadata(PENDING).is_none());
}

#[test]
fn final_clock_expiry_and_rollback_preserve_pending_bytes_without_returning_a_receipt() {
    for expired in [false, true] {
        let mut world = World::new();
        let bytes = signed(PACKAGE_PAYLOAD_TYPE, &world.fixture.base.package, &[3]);
        let mut samples = 0;
        let result = install_fixture_with_hook(
            &world.trust,
            world.store.as_mut().unwrap(),
            world.fixture.input(&bytes),
            || {
                samples += 1;
                if samples < 5 {
                    world.fixture.now()
                } else if expired {
                    world.fixture.base.package.expires_at
                } else {
                    world.fixture.now() - 1
                }
            },
            |_, _| Ok(()),
        );
        assert_eq!(samples, 5);
        let expected = if expired {
            PackageError::TrustExpired
        } else {
            PackageError::ClockRollback
        };
        assert_eq!(
            result.err(),
            Some(ToolInstallError::Admission(ToolAdmissionError::Package(
                expected
            )))
        );
        assert!(world.metadata(KEY).is_none());
        assert!(world.metadata(PENDING).is_some());
        assert_eq!(
            fs::read(world.blob().join("package.zip")).unwrap(),
            world.fixture.base.archive
        );
        assert!(world.abort().unwrap());
        assert!(world.pins().unwrap().is_empty());
    }
}

#[test]
fn revocation_between_publication_and_commit_cannot_install_the_tool_or_clear_the_floor() {
    let mut world = World::new();
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
        world
            .trust
            .snapshot(world.store.as_mut().unwrap())
            .unwrap()
            .root_digest,
    );
    root.revoked_package_digests
        .push(world.fixture.base.package.archive.sha256.clone());
    let result = world.with_hook(|phase, store| {
        if phase == ToolInstallPhase::BeforeCommit {
            trust
                .rotate(store, &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]), now)
                .unwrap();
        }
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ToolInstallError::Admission(ToolAdmissionError::Package(
            PackageError::PackageRevoked
        )))
    ));
    assert!(world.metadata(KEY).is_none());
    assert!(world.metadata(PENDING).is_some());
    let before = world.trust.snapshot(world.store.as_mut().unwrap()).unwrap();
    assert!(world.abort().unwrap());
    let after = world.trust.snapshot(world.store.as_mut().unwrap()).unwrap();
    assert_eq!(after.root_digest, before.root_digest);
    assert_eq!(after.known_versions.len(), before.known_versions.len());
    assert!(world.pins().unwrap().is_empty());
}

#[test]
fn last_trust_observation_cannot_substitute_the_pending_journal_before_sql_commit() {
    let mut world = World::new();
    let bytes = signed(PACKAGE_PAYLOAD_TYPE, &world.fixture.base.package, &[3]);
    let target_time = world.fixture.now() + 5;
    let mut samples = 0;
    let result = install_fixture_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        world.fixture.input(&bytes),
        || {
            samples += 1;
            world.fixture.now() + samples
        },
        |phase, store| {
            if phase == ToolInstallPhase::BeforeCommit {
                store.connection.execute_batch(&format!("CREATE TRIGGER replace_last_tool_journal AFTER UPDATE ON core_metadata WHEN NEW.key='package-trust-state-v1' AND json_extract(NEW.value,'$.observedTime')={target_time} BEGIN UPDATE core_metadata SET value='CORRUPTED_AT_FINAL_TRUST_OBSERVATION' WHERE key='tool-archive-install-pending-v1'; END;")).unwrap();
            }
            Ok(())
        },
    );
    assert_eq!(samples, 5);
    assert!(matches!(
        result,
        Err(ToolInstallError::Storage(InstallError::StateInvalid))
    ));
    assert!(world.metadata(KEY).is_none());
    assert_eq!(
        world.metadata(PENDING).as_deref(),
        Some("CORRUPTED_AT_FINAL_TRUST_OBSERVATION")
    );
    assert!(world.abort().is_err());
    assert_eq!(
        fs::read(world.blob().join("package.zip")).unwrap(),
        world.fixture.base.archive
    );
}

#[test]
fn retained_plugin_quarantine_and_new_tool_archives_share_the_existing_disk_budget() {
    let mut world = World::new();
    let retained = world.path.join("package-store-v1/quarantine");
    fs::create_dir_all(&retained).unwrap();
    for _ in 0..4 {
        let group = retained.join(format!("{}-stage", uuid::Uuid::new_v4()));
        fs::create_dir(&group).unwrap();
        fs::File::create(group.join("package.zip"))
            .unwrap()
            .set_len(67_108_864)
            .unwrap();
    }
    assert!(matches!(
        world.install(),
        Err(ToolInstallError::Storage(InstallError::BudgetExceeded))
    ));
    assert!(world.metadata(KEY).is_none());
    assert!(world.metadata(PENDING).is_none());
    assert_eq!(fs::read_dir(retained).unwrap().count(), 4);
}

#[test]
fn copied_and_supported_backup_profiles_keep_tool_metadata_but_cannot_reuse_it() {
    let mut world = World::new();
    world.install().unwrap();
    let before = world.metadata(KEY).unwrap();
    let original = fs::read(world.blob().join("package.zip")).unwrap();
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
    assert_eq!(
        copy.pins().unwrap_err(),
        ToolInstallError::Storage(InstallError::StateInvalid)
    );
    assert_eq!(
        copy.install().err(),
        Some(ToolInstallError::Storage(InstallError::StateInvalid))
    );
    assert_eq!(
        copy.abort().err(),
        Some(ToolInstallError::Storage(InstallError::StateInvalid))
    );
    assert_eq!(copy.metadata(KEY).as_deref(), Some(before.as_str()));
    assert!(!copy.root().exists());
    DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
    world.reopen();
    assert_eq!(
        world.pins().unwrap_err(),
        ToolInstallError::Admission(ToolAdmissionError::Package(
            PackageError::TrustRecoveryRequired
        ))
    );
    assert!(world.install().is_err());
    assert!(world.abort().is_err());
    assert_eq!(world.metadata(KEY).as_deref(), Some(before.as_str()));
    assert_eq!(
        fs::read(world.blob().join("package.zip")).unwrap(),
        original
    );
}

#[test]
fn empty_retained_plugin_groups_and_new_tool_artifacts_share_the_group_limit() {
    let mut world = World::new();
    let retained = world.path.join("package-store-v1/quarantine");
    fs::create_dir_all(&retained).unwrap();
    for _ in 0..384 {
        fs::create_dir(retained.join(format!("{}-stage", uuid::Uuid::new_v4()))).unwrap();
    }
    assert_eq!(
        world.install().err(),
        Some(ToolInstallError::Storage(InstallError::BudgetExceeded))
    );
    assert!(world.metadata(KEY).is_none());
    assert!(world.metadata(PENDING).is_none());
    assert_eq!(fs::read_dir(retained).unwrap().count(), 384);
}

const PHASES: [ToolInstallPhase; 5] = [
    ToolInstallPhase::JournalCommitted,
    ToolInstallPhase::StagePrepared,
    ToolInstallPhase::ArtifactPublished,
    ToolInstallPhase::BeforeCommit,
    ToolInstallPhase::Committed,
];
#[test]
fn process_crash_child() {
    let Some(path) = std::env::var_os("FLOWTOOLS_TOOL_INSTALL_CHILD_ROOT") else {
        return;
    };
    let phase: usize = std::env::var("FLOWTOOLS_TOOL_INSTALL_BARRIER")
        .unwrap()
        .parse()
        .unwrap();
    assert!(phase < PHASES.len());
    let mut world = World::open(Path::new(&path), false, false);
    world
        .with_hook(|observed, _| {
            if observed == PHASES[phase] {
                std::process::exit(70 + phase as i32);
            }
            Ok(())
        })
        .unwrap();
    panic!("child did not exit at the actual barrier");
}
#[test]
fn actual_process_exits_at_every_barrier_keep_old_inventory_or_atomic_inert_commit() {
    for (index, _) in PHASES.iter().enumerate() {
        let mut world = World::new();
        drop(world.store.take());
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "packages::tests::tool_tests::install_tests::process_crash_child",
                "--nocapture",
            ])
            .env("FLOWTOOLS_TOOL_INSTALL_CHILD_ROOT", &world.path)
            .env("FLOWTOOLS_TOOL_INSTALL_BARRIER", index.to_string())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(70 + index as i32));
        world.store = Some(DataStore::open(&world.path.join("runtime.sqlite")).unwrap());
        if index == 4 {
            assert_eq!(world.pins().unwrap().len(), 1);
            assert!(world.metadata(PENDING).is_none());
        } else {
            assert_eq!(
                world.pins().unwrap_err(),
                ToolInstallError::Storage(InstallError::RecoveryPending)
            );
            assert!(world.abort().unwrap());
            assert!(world.pins().unwrap().is_empty());
        }
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
