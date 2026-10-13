use super::*;
use crate::{
    data::DataStore,
    packages::trust::{DescriptorVerification, PackageTrustStore},
    protocol::StorageAction,
};
use std::{
    io::{Cursor, Write},
    path::PathBuf,
    process::Command,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

mod recovery_tests;

fn manager(fixture: &Fixture) -> PackageTrustStore {
    PackageTrustStore::new(BootstrapPins {
        keys: serde_json::from_value(fixture.golden["pins"].clone()).unwrap(),
        threshold: 2,
    })
    .unwrap()
}
fn bootstrap(store: &mut DataStore, trust: &PackageTrustStore, fixture: &Fixture) {
    trust
        .bootstrap(
            store,
            &serde_json::to_vec(&fixture.golden["rootEnvelope"]).unwrap(),
            fixture.policy.clock.now,
        )
        .unwrap();
}
fn input<'a>(fixture: &'a Fixture, envelope: &'a [u8], now: u64) -> DescriptorVerification<'a> {
    DescriptorVerification {
        envelope,
        manifest: &fixture.manifest,
        archive: &fixture.archive,
        target: fixture.package.target.clone(),
        now,
    }
}
fn verify(
    store: &mut DataStore,
    trust: &PackageTrustStore,
    fixture: &Fixture,
    now: u64,
) -> Result<VerifiedPackageDescriptor, PackageError> {
    trust.verify_descriptor(
        store,
        input(
            fixture,
            &signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[3]),
            now,
        ),
    )
}
fn revision(version: &str, sequence: u64) -> Fixture {
    let mut fixture = Fixture::new();
    let inventory = archive::validate_archive(
        &fixture.verify().unwrap(),
        &fixture.manifest,
        &fixture.archive,
    )
    .unwrap();
    fixture.package.version = version.into();
    fixture.package.release_sequence = sequence;
    let mut manifest: Value = serde_json::from_slice(&fixture.manifest).unwrap();
    manifest["version"] = json!(version);
    fixture.manifest = serde_json::to_vec(&manifest).unwrap();
    fixture.package.manifest = BlobDescriptor {
        sha256: hash(&fixture.manifest),
        size: fixture.manifest.len() as u64,
    };
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in inventory.files() {
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(if name == "manifest.json" {
            &fixture.manifest
        } else {
            bytes
        })
        .unwrap();
    }
    fixture.archive = zip.finish().unwrap().into_inner();
    fixture.package.archive.sha256 = hash(&fixture.archive);
    fixture.package.archive.size = fixture.archive.len() as u64;
    archive::validate_archive(
        &fixture.verify().unwrap(),
        &fixture.manifest,
        &fixture.archive,
    )
    .unwrap();
    fixture
}
fn profile() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "flowtools-validation-package-trust-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
fn raw(store: &DataStore) -> String {
    store
        .connection
        .query_row(
            "SELECT value FROM core_metadata WHERE key='package-trust-state-v1'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}
fn replace_state(store: &DataStore, value: &Value) {
    store
        .connection
        .execute(
            "UPDATE core_metadata SET value=?1 WHERE key='package-trust-state-v1'",
            [serde_json::to_string(value).unwrap()],
        )
        .unwrap();
}

#[test]
fn durable_identity_release_and_time_floors_survive_reopen_without_new_store() {
    let path = profile();
    let fixture = Fixture::new();
    let now = fixture.policy.clock.now;
    let trust = manager(&fixture);
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    assert_eq!(
        trust.snapshot(&mut store).err(),
        Some(PackageError::TrustStateRequired)
    );
    bootstrap(&mut store, &trust, &fixture);
    let first = verify(&mut store, &trust, &fixture, now).unwrap();
    assert_eq!(first.package_digest(), fixture.package.archive.sha256);
    assert_eq!(
        DataStore::open(&path.join("runtime.sqlite")).err(),
        Some(crate::protocol::ErrorCode::StoreBusy)
    );
    let newer = revision("2.0.0", 2);
    verify(&mut store, &trust, &newer, now + 10).unwrap();
    drop(store);
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    let trust = manager(&fixture);
    let snapshot = trust.snapshot(&mut store).unwrap();
    assert_eq!(snapshot.known_versions.len(), 2);
    assert_eq!(snapshot.observed_time, now + 10);
    assert_eq!(
        verify(&mut store, &trust, &fixture, now + 10).err(),
        Some(PackageError::ReleaseRollback)
    );
    assert_eq!(
        verify(&mut store, &trust, &newer, now + 9).err(),
        Some(PackageError::ClockRollback)
    );
    assert_eq!(
        trust
            .bootstrap(
                &mut store,
                &serde_json::to_vec(&fixture.golden["rootEnvelope"]).unwrap(),
                now + 10
            )
            .err(),
        Some(PackageError::RootRollback)
    );
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn immutable_versions_sequence_conflicts_and_descriptor_renewal_use_database_floors() {
    let fixture = Fixture::new();
    let now = fixture.policy.clock.now;
    let trust = manager(&fixture);
    let mut store = DataStore::memory().unwrap();
    bootstrap(&mut store, &trust, &fixture);
    verify(&mut store, &trust, &fixture, now).unwrap();
    let mut renewed = Fixture::new();
    renewed.package.expires_at += 1;
    let receipt = verify(&mut store, &trust, &renewed, now + 1).unwrap();
    assert_eq!(receipt.package_digest(), fixture.package.archive.sha256);
    assert_eq!(trust.snapshot(&mut store).unwrap().known_versions.len(), 1);
    let changed = revision("1.0.0", 1);
    assert_ne!(
        changed.package.archive.sha256,
        fixture.package.archive.sha256
    );
    assert_eq!(
        verify(&mut store, &trust, &changed, now + 1).err(),
        Some(PackageError::VersionConflict)
    );
    let sequence_changed = revision("1.0.0", 2);
    assert_eq!(
        verify(&mut store, &trust, &sequence_changed, now + 1).err(),
        Some(PackageError::VersionConflict)
    );
    let sequence_reused = revision("2.0.0", 1);
    assert_eq!(
        verify(&mut store, &trust, &sequence_reused, now + 1).err(),
        Some(PackageError::VersionConflict)
    );
}
#[test]
fn current_root_is_reloaded_and_revocation_survives_rotation_and_reopen() {
    let path = profile();
    let fixture = Fixture::new();
    let now = fixture.policy.clock.now;
    let first = manager(&fixture);
    let stale = manager(&fixture);
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    bootstrap(&mut store, &first, &fixture);
    verify(&mut store, &first, &fixture, now).unwrap();
    let mut next = next_root(&fixture);
    next.revoked_package_digests
        .push(fixture.package.archive.sha256.clone());
    first
        .rotate(
            &mut store,
            &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2, 4, 5]),
            now + 1,
        )
        .unwrap();
    assert_eq!(
        verify(&mut store, &stale, &fixture, now + 1).err(),
        Some(PackageError::PackageRevoked)
    );
    drop(store);
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    assert_eq!(first.snapshot(&mut store).unwrap().root_version, 2);
    assert_eq!(
        verify(&mut store, &stale, &fixture, now + 2).err(),
        Some(PackageError::PackageRevoked)
    );
    let mut third = next.clone();
    third.version = 3;
    third.previous_root_sha256 = Some(hash(&serde_json::to_vec(&next).unwrap()));
    third.revoked_package_digests.clear();
    assert_eq!(
        first
            .rotate(
                &mut store,
                &signed(ROOT_PAYLOAD_TYPE, &third, &[4, 5]),
                now + 2
            )
            .err(),
        Some(PackageError::RootRollback)
    );
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn expired_history_can_authenticate_a_fresh_successor_without_admitting_an_expired_root() {
    let fixture = Fixture::new();
    let trust = manager(&fixture);
    let mut store = DataStore::memory().unwrap();
    bootstrap(&mut store, &trust, &fixture);
    let now = fixture.root.payload().expires_at;
    assert_eq!(
        verify(&mut store, &trust, &fixture, now).err(),
        Some(PackageError::TrustExpired)
    );
    assert_eq!(
        verify(&mut store, &trust, &fixture, now - 1).err(),
        Some(PackageError::ClockRollback)
    );
    let mut next = next_root(&fixture);
    next.issued_at = now;
    next.expires_at = now + 86_400;
    trust
        .rotate(
            &mut store,
            &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2, 4, 5]),
            now,
        )
        .unwrap();
    let mut current = Fixture::new();
    current.package.issued_at = now;
    current.package.expires_at = now + 86_400;
    verify(&mut store, &trust, &current, now).unwrap();
}
#[test]
fn invalid_package_observes_t0_time_but_never_records_an_identity_or_grants() {
    let fixture = Fixture::new();
    let now = fixture.policy.clock.now;
    let trust = manager(&fixture);
    let mut store = DataStore::memory().unwrap();
    bootstrap(&mut store, &trust, &fixture);
    let forged = signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[6]);
    assert_eq!(
        trust
            .verify_descriptor(&mut store, input(&fixture, &forged, now + 10))
            .err(),
        Some(PackageError::SignatureInvalid)
    );
    let snapshot = trust.snapshot(&mut store).unwrap();
    assert_eq!(snapshot.observed_time, now + 10);
    assert!(snapshot.known_versions.is_empty());
    assert_eq!(
        verify(&mut store, &trust, &fixture, now + 9).err(),
        Some(PackageError::ClockRollback)
    );
    let count: u32 = store.connection.query_row("SELECT (SELECT count(*) FROM grants)+(SELECT count(*) FROM jobs)+(SELECT count(*) FROM plugin_data)",[],|row|row.get(0)).unwrap();
    assert_eq!(count, 0);
}
#[test]
fn t0_pins_are_bound_and_key_order_does_not_change_the_binding() {
    let fixture = Fixture::new();
    let trust = manager(&fixture);
    let mut store = DataStore::memory().unwrap();
    bootstrap(&mut store, &trust, &fixture);
    let changed = PackageTrustStore::new(BootstrapPins {
        keys: vec![public(1), public(4)],
        threshold: 2,
    })
    .unwrap();
    assert_eq!(
        changed.snapshot(&mut store).err(),
        Some(PackageError::TrustStateInvalid)
    );
    let mut pins: Vec<PublicKey> = serde_json::from_value(fixture.golden["pins"].clone()).unwrap();
    pins.reverse();
    let reordered = PackageTrustStore::new(BootstrapPins {
        keys: pins,
        threshold: 2,
    })
    .unwrap();
    assert_eq!(reordered.snapshot(&mut store).unwrap().root_version, 1);
    assert_eq!(
        PackageTrustStore::new(BootstrapPins {
            keys: vec![public(1)],
            threshold: 1
        })
        .err(),
        Some(PackageError::TrustInvalid)
    );
}
#[test]
fn corrupt_unknown_duplicate_or_oversized_state_cannot_bootstrap_itself_again() {
    let fixture = Fixture::new();
    let now = fixture.policy.clock.now;
    let trust = manager(&fixture);
    let mut store = DataStore::memory().unwrap();
    bootstrap(&mut store, &trust, &fixture);
    verify(&mut store, &trust, &fixture, now).unwrap();
    let good: Value = serde_json::from_str(&raw(&store)).unwrap();
    for (pointer, value) in [
        ("/formatVersion", json!(2)),
        ("/pinsDigest", json!("0".repeat(64))),
        ("/observedTime", json!(0)),
        ("/roots", json!([])),
        ("/roots/0/envelope", json!("corrupt")),
        ("/roots/0/observedAt", json!(0)),
        ("/knownVersions/0/releaseSequence", json!(0)),
        ("/knownVersions/0/packageDigest", json!("corrupt")),
        (
            "/knownVersions",
            json!([good["knownVersions"][0], good["knownVersions"][0]]),
        ),
    ] {
        let mut corrupt = good.clone();
        *corrupt.pointer_mut(pointer).unwrap() = value;
        replace_state(&store, &corrupt);
        assert_eq!(
            trust.snapshot(&mut store).err(),
            Some(PackageError::TrustStateInvalid),
            "{pointer}"
        );
        assert_eq!(
            trust
                .bootstrap(
                    &mut store,
                    &serde_json::to_vec(&fixture.golden["rootEnvelope"]).unwrap(),
                    now
                )
                .err(),
            Some(PackageError::RootRollback)
        );
        assert_eq!(
            serde_json::from_str::<Value>(&raw(&store)).unwrap(),
            corrupt
        );
    }
    let mut corrupt = good.clone();
    corrupt["newTrustFlag"] = json!(true);
    replace_state(&store, &corrupt);
    assert_eq!(
        trust.snapshot(&mut store).err(),
        Some(PackageError::TrustStateInvalid)
    );
    let duplicate = format!(
        "{{\"formatVersion\":1,{}",
        serde_json::to_string(&good)
            .unwrap()
            .trim_start_matches('{')
    );
    store
        .connection
        .execute(
            "UPDATE core_metadata SET value=?1 WHERE key='package-trust-state-v1'",
            [duplicate],
        )
        .unwrap();
    assert_eq!(
        trust.snapshot(&mut store).err(),
        Some(PackageError::TrustStateInvalid)
    );
    store
        .connection
        .execute(
            "UPDATE core_metadata SET value=?1 WHERE key='package-trust-state-v1'",
            [" ".repeat(16_777_217)],
        )
        .unwrap();
    assert_eq!(
        trust.snapshot(&mut store).err(),
        Some(PackageError::TrustStateInvalid)
    );
    replace_state(&store, &good);
    assert_eq!(trust.snapshot(&mut store).unwrap().known_versions.len(), 1);
}
#[test]
fn failed_state_commit_never_returns_a_receipt_or_changes_the_package_floor() {
    let fixture = Fixture::new();
    let now = fixture.policy.clock.now;
    let trust = manager(&fixture);
    let mut store = DataStore::memory().unwrap();
    bootstrap(&mut store, &trust, &fixture);
    verify(&mut store, &trust, &fixture, now).unwrap();
    let newer = revision("2.0.0", 2);
    store.connection.execute_batch("CREATE TRIGGER reject_observation BEFORE UPDATE ON core_metadata WHEN NEW.key='package-trust-state-v1' BEGIN SELECT RAISE(ABORT,'controlled clock write fault'); END;").unwrap();
    assert_eq!(
        verify(&mut store, &trust, &newer, now + 1).err(),
        Some(PackageError::TrustStorageFailed)
    );
    assert_eq!(trust.snapshot(&mut store).unwrap().observed_time, now);
    assert_eq!(trust.snapshot(&mut store).unwrap().known_versions.len(), 1);
    store
        .connection
        .execute_batch("DROP TRIGGER reject_observation")
        .unwrap();
    store.connection.execute_batch("CREATE TRIGGER reject_new_package BEFORE UPDATE ON core_metadata WHEN NEW.key='package-trust-state-v1' AND json_array_length(json_extract(NEW.value,'$.knownVersions'))>1 BEGIN SELECT RAISE(ABORT,'controlled trust write fault'); END;").unwrap();
    assert_eq!(
        verify(&mut store, &trust, &newer, now + 1).err(),
        Some(PackageError::TrustStorageFailed)
    );
    assert_eq!(trust.snapshot(&mut store).unwrap().known_versions.len(), 1);
    assert_eq!(trust.snapshot(&mut store).unwrap().observed_time, now + 1);
    verify(&mut store, &trust, &fixture, now + 1).unwrap();
    store
        .connection
        .execute_batch("DROP TRIGGER reject_new_package")
        .unwrap();
    verify(&mut store, &trust, &newer, now + 1).unwrap();
    assert_eq!(
        verify(&mut store, &trust, &fixture, now + 1).err(),
        Some(PackageError::ReleaseRollback)
    );
}
#[test]
fn bounded_history_refuses_without_silently_pruning_a_floor_or_root_chain() {
    let fixture = Fixture::new();
    let now = fixture.policy.clock.now;
    let trust = manager(&fixture);
    let mut store = DataStore::memory().unwrap();
    bootstrap(&mut store, &trust, &fixture);
    let mut root = fixture.root.clone();
    for version in 2..=32 {
        let mut next = root.payload().clone();
        next.version = version;
        next.previous_root_sha256 = Some(root.payload_digest().into());
        root = trust
            .rotate(
                &mut store,
                &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2]),
                now + version,
            )
            .unwrap();
    }
    let mut next = root.payload().clone();
    next.version = 33;
    next.previous_root_sha256 = Some(root.payload_digest().into());
    assert_eq!(
        trust
            .rotate(
                &mut store,
                &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2]),
                now + 33
            )
            .err(),
        Some(PackageError::TrustBudgetExceeded)
    );
    assert_eq!(trust.snapshot(&mut store).unwrap().root_version, 32);
    let mut state: Value = serde_json::from_str(&raw(&store)).unwrap();
    let base = fixture.known(&fixture.package.archive.sha256);
    state["knownVersions"] = json!((1..=1024)
        .map(|sequence| {
            let mut known = base.clone();
            known.version = format!("{sequence}.0.0");
            known.release_sequence = sequence;
            known.package_digest = hash(sequence.to_string().as_bytes());
            known
        })
        .collect::<Vec<_>>());
    replace_state(&store, &state);
    let newer = revision("1025.0.0", 1025);
    assert_eq!(
        verify(&mut store, &trust, &newer, now + 33).err(),
        Some(PackageError::TrustBudgetExceeded)
    );
    assert_eq!(
        trust.snapshot(&mut store).unwrap().known_versions.len(),
        1024
    );
}

#[test]
fn trust_crash_child() {
    let Ok(path) = std::env::var("FLOWTOOLS_TRUST_TEST_PROFILE") else {
        return;
    };
    let mode = std::env::var("FLOWTOOLS_TRUST_TEST_MODE").unwrap();
    let fixture = revision("2.0.0", 2);
    let trust = manager(&fixture);
    let mut store = DataStore::open(&PathBuf::from(path).join("runtime.sqlite")).unwrap();
    let now = fixture.policy.clock.now + 1;
    if mode == "before-commit" {
        trust
            .verify_with_hook(
                &mut store,
                input(
                    &fixture,
                    &signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[3]),
                    now,
                ),
                || std::process::exit(17),
            )
            .unwrap();
    } else {
        verify(&mut store, &trust, &fixture, now).unwrap();
        std::process::exit(17);
    }
}
#[test]
fn real_process_exit_on_both_sides_of_commit_preserves_the_correct_floor() {
    for mode in ["before-commit", "after-commit"] {
        let path = profile();
        let fixture = Fixture::new();
        let trust = manager(&fixture);
        let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
        bootstrap(&mut store, &trust, &fixture);
        verify(&mut store, &trust, &fixture, fixture.policy.clock.now).unwrap();
        drop(store);
        let result = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "packages::tests::trust_tests::trust_crash_child"])
            .env("FLOWTOOLS_TRUST_TEST_PROFILE", &path)
            .env("FLOWTOOLS_TRUST_TEST_MODE", mode)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(result.code(), Some(17));
        let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
        let snapshot = trust.snapshot(&mut store).unwrap();
        assert_eq!(snapshot.observed_time, fixture.policy.clock.now + 1);
        assert_eq!(
            snapshot.known_versions.len(),
            if mode == "before-commit" { 1 } else { 2 }
        );
        assert_eq!(
            verify(&mut store, &trust, &fixture, fixture.policy.clock.now + 1).is_ok(),
            mode == "before-commit"
        );
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }
}
#[test]
fn managed_restore_that_predates_trust_initialization_cannot_reset_the_trust_floor() {
    let path = profile();
    let fixture = Fixture::new();
    let trust = manager(&fixture);
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    let backup = store.backup().unwrap();
    bootstrap(&mut store, &trust, &fixture);
    verify(
        &mut store,
        &trust,
        &revision("2.0.0", 2),
        fixture.policy.clock.now,
    )
    .unwrap();
    drop(store);
    let backup_id = backup
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("runtime-backup-")
        .unwrap()
        .strip_suffix(".sqlite")
        .unwrap()
        .to_owned();
    crate::recovery::manage(&path, StorageAction::Restore { backup_id }).unwrap();
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    assert_eq!(
        trust.snapshot(&mut store).err(),
        Some(PackageError::TrustRecoveryRequired)
    );
    assert_eq!(
        trust
            .bootstrap(
                &mut store,
                &serde_json::to_vec(&fixture.golden["rootEnvelope"]).unwrap(),
                fixture.policy.clock.now
            )
            .err(),
        Some(PackageError::TrustRecoveryRequired)
    );
    assert_eq!(
        verify(&mut store, &trust, &fixture, fixture.policy.clock.now).err(),
        Some(PackageError::TrustRecoveryRequired)
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM core_metadata WHERE key='package-trust-state-v1'",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn low_level_backup_restore_also_blocks_stale_trust_even_when_old_state_exists() {
    let path = profile();
    let fixture = Fixture::new();
    let trust = manager(&fixture);
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    bootstrap(&mut store, &trust, &fixture);
    verify(&mut store, &trust, &fixture, fixture.policy.clock.now).unwrap();
    let backup = store.backup().unwrap();
    verify(
        &mut store,
        &trust,
        &revision("2.0.0", 2),
        fixture.policy.clock.now,
    )
    .unwrap();
    drop(store);
    let preserved = DataStore::restore_backup(&path.join("runtime.sqlite"), &backup).unwrap();
    assert!(preserved.exists());
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    assert_eq!(
        trust.snapshot(&mut store).err(),
        Some(PackageError::TrustRecoveryRequired)
    );
    assert_eq!(
        verify(&mut store, &trust, &fixture, fixture.policy.clock.now).err(),
        Some(PackageError::TrustRecoveryRequired)
    );
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}
