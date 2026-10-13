use super::*;
use crate::packages::trust::{
    TrustRecoveryApproval, TrustRecoveryChallenge, RECOVERY_PAYLOAD_TYPE,
};
use rusqlite::OptionalExtension;

const REQUEST: &str = "package-trust-recovery-request-v1";
const RECEIPT: &str = "package-trust-recovery-receipt-v1";
const MARKER: &str = "package-trust-recovery-required";

fn recovery_manager(fixture: &Fixture) -> PackageTrustStore {
    PackageTrustStore::with_recovery_authority(
        BootstrapPins {
            keys: serde_json::from_value(fixture.golden["pins"].clone()).unwrap(),
            threshold: 2,
        },
        BootstrapPins {
            keys: vec![public(6), public(7)],
            threshold: 2,
        },
    )
    .unwrap()
}

struct World {
    path: PathBuf,
    trust: PackageTrustStore,
    store: Option<DataStore>,
    checkpoint: Vec<u8>,
    now: u64,
    rotated: bool,
}
impl World {
    fn new(pretrust: bool, rotated: bool) -> Self {
        let path = profile();
        let fixture = Fixture::new();
        let trust = recovery_manager(&fixture);
        let now = fixture.policy.clock.now;
        let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
        let early = pretrust.then(|| store.backup().unwrap());
        bootstrap(&mut store, &trust, &fixture);
        verify(&mut store, &trust, &fixture, now).unwrap();
        let backup = early.unwrap_or_else(|| store.backup().unwrap());
        verify(&mut store, &trust, &revision("2.0.0", 2), now + 1).unwrap();
        if rotated {
            let mut root = next_root(&fixture);
            root.revoked_package_digests
                .push(fixture.package.archive.sha256);
            trust
                .rotate(
                    &mut store,
                    &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2, 4, 5]),
                    now + 2,
                )
                .unwrap();
        }
        let checkpoint = trust
            .export_recovery_checkpoint(&mut store, now + 3)
            .unwrap();
        drop(store);
        DataStore::restore_backup(&path.join("runtime.sqlite"), &backup).unwrap();
        let store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
        Self {
            path,
            trust,
            store: Some(store),
            checkpoint,
            now: now + 4,
            rotated,
        }
    }
    fn challenge(&mut self) -> TrustRecoveryChallenge {
        self.trust
            .begin_trust_recovery(self.store.as_mut().unwrap(), self.now)
            .unwrap()
    }
    fn envelope(&self, challenge: &TrustRecoveryChallenge, bytes: &[u8], seeds: &[u8]) -> Vec<u8> {
        approval(challenge, bytes, self.now, seeds)
    }
    fn signed(&self, challenge: &TrustRecoveryChallenge) -> Vec<u8> {
        self.envelope(
            challenge,
            &self.checkpoint,
            if self.rotated {
                &[4, 5, 6, 7]
            } else {
                &[1, 2, 6, 7]
            },
        )
    }
    fn recover(
        &mut self,
        envelope: &[u8],
    ) -> Result<crate::packages::trust::TrustSnapshot, PackageError> {
        self.trust.recover_trust(
            self.store.as_mut().unwrap(),
            &self.checkpoint,
            envelope,
            || self.now,
        )
    }
    fn value(&self, key: &str) -> Option<String> {
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
    fn replace(&self, key: &str, value: &str) {
        self.store
            .as_ref()
            .unwrap()
            .connection
            .execute(
                "INSERT OR REPLACE INTO core_metadata(key,value) VALUES(?1,?2)",
                [key, value],
            )
            .unwrap();
    }
    fn blocked(&mut self) {
        assert_eq!(
            self.trust.snapshot(self.store.as_mut().unwrap()).err(),
            Some(PackageError::TrustRecoveryRequired)
        );
    }
}
fn approval(challenge: &TrustRecoveryChallenge, bytes: &[u8], now: u64, seeds: &[u8]) -> Vec<u8> {
    let state: Value = serde_json::from_slice(bytes).unwrap();
    let roots = state["roots"].as_array().unwrap();
    let envelope = decode_base64(roots.last().unwrap()["envelope"].as_str().unwrap()).unwrap();
    let root = decode_envelope(&envelope, ROOT_PAYLOAD_TYPE).unwrap();
    signed(
        RECOVERY_PAYLOAD_TYPE,
        &TrustRecoveryApproval {
            format_version: 1,
            request_digest: challenge.request_digest().into(),
            checkpoint_sha256: hash(bytes),
            root_sha256: hash(&root.payload),
            authority_sha256: challenge.authority_digest().into(),
            issued_at: now,
            expires_at: now + 60,
        },
        seeds,
    )
}
impl Drop for World {
    fn drop(&mut self) {
        self.store.take();
        assert_eq!(self.path.parent().unwrap(), std::env::temp_dir());
        let id = self
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("flowtools-validation-package-trust-")
            .unwrap();
        assert_eq!(uuid::Uuid::parse_str(id).unwrap().to_string(), id);
        std::fs::remove_dir_all(&self.path).unwrap();
    }
}

#[test]
fn copied_database_cannot_replay_another_profiles_request_or_receipt() {
    let mut a = World::new(false, false);
    let mut b = World::new(false, false);
    let request = a.challenge();
    let approval = a.signed(&request);
    b.store.take();
    let clone = b.path.join("cloned.sqlite");
    a.store
        .as_ref()
        .unwrap()
        .connection
        .backup(rusqlite::MAIN_DB, &clone, None)
        .unwrap();
    b.store = Some(DataStore::open(&clone).unwrap());
    assert_eq!(a.value(REQUEST), b.value(REQUEST));
    assert_eq!(a.value(MARKER), b.value(MARKER));
    assert_eq!(b.recover(&approval).err(), Some(PackageError::TrustInvalid));
    assert!(b.value(RECEIPT).is_none());
    let own = b.challenge();
    assert_ne!(request.request_digest(), own.request_digest());
    assert_eq!(b.recover(&approval).err(), Some(PackageError::TrustInvalid));
    a.recover(&approval).unwrap();
    b.store.take();
    // Copy an already committed acknowledgement as well as the matching marker.
    a.store
        .as_ref()
        .unwrap()
        .connection
        .backup(rusqlite::MAIN_DB, &clone, None)
        .unwrap();
    b.store = Some(DataStore::open(&clone).unwrap());
    assert_eq!(a.value(RECEIPT), b.value(RECEIPT));
    b.blocked();
    let own = b.challenge();
    // A new approval must also preserve the prior receipt's commit-time floor.
    b.checkpoint = a
        .trust
        .export_recovery_checkpoint(a.store.as_mut().unwrap(), a.now)
        .unwrap();
    let own_approval = b.signed(&own);
    b.recover(&own_approval).unwrap();
    assert_eq!(
        b.trust
            .snapshot(b.store.as_mut().unwrap())
            .unwrap()
            .known_versions
            .len(),
        2
    );
}

#[test]
fn actual_backup_recovery_preserves_highest_floors_and_keeps_restore_markers() {
    for pretrust in [false, true] {
        let mut world = World::new(pretrust, false);
        world.blocked();
        let marker = world.value(MARKER).unwrap();
        assert!(uuid::Uuid::parse_str(&marker).is_ok());
        let request = world.challenge();
        let same = world.challenge();
        assert_eq!(request.id(), same.id());
        assert_eq!(request.request_digest(), same.request_digest());
        world.blocked();
        let envelope = world.signed(&request);
        let snapshot = world.recover(&envelope).unwrap();
        assert_eq!(snapshot.known_versions.len(), 2);
        assert_eq!(snapshot.observed_time, world.now);
        assert_eq!(world.value(MARKER), Some(marker));
        assert!(world.value(REQUEST).is_none());
        assert!(world.value(RECEIPT).is_some());
        assert_eq!(
            verify(
                world.store.as_mut().unwrap(),
                &world.trust,
                &Fixture::new(),
                world.now
            )
            .err(),
            Some(PackageError::ReleaseRollback)
        );
        verify(
            world.store.as_mut().unwrap(),
            &world.trust,
            &revision("2.0.0", 2),
            world.now,
        )
        .unwrap();
        let receipt = world.value(RECEIPT);
        assert_eq!(
            world.recover(&envelope).err(),
            Some(PackageError::TrustInvalid)
        );
        assert_eq!(world.value(RECEIPT), receipt);
        // A committed receipt survives its approval window; no replay is needed.
        verify(
            world.store.as_mut().unwrap(),
            &world.trust,
            &revision("2.0.0", 2),
            world.now + 120,
        )
        .unwrap();
        world.store.take();
        world.store = Some(DataStore::open(&world.path.join("runtime.sqlite")).unwrap());
        assert_eq!(
            world
                .trust
                .snapshot(world.store.as_mut().unwrap())
                .unwrap()
                .known_versions
                .len(),
            2
        );
    }
}

#[test]
fn rotated_root_quorum_recovers_append_only_revocations_without_restoring_old_keys() {
    let mut world = World::new(false, true);
    let request = world.challenge();
    let old = world.envelope(&request, &world.checkpoint, &[1, 2]);
    assert_eq!(
        world.recover(&old).err(),
        Some(PackageError::SignatureInvalid)
    );
    world.blocked();
    let envelope = world.signed(&request);
    assert_eq!(world.recover(&envelope).unwrap().root_version, 2);
    assert_eq!(
        verify(
            world.store.as_mut().unwrap(),
            &world.trust,
            &Fixture::new(),
            world.now
        )
        .err(),
        Some(PackageError::PackageRevoked)
    );
    verify(
        world.store.as_mut().unwrap(),
        &world.trust,
        &revision("2.0.0", 2),
        world.now,
    )
    .unwrap();
}

#[test]
fn retired_root_cannot_self_approve_stale_checkpoint_without_independent_authority() {
    let mut world = World::new(false, true);
    let request = world.challenge();
    // The backup lost root v2 and release v2. Retired root v1 can still sign a
    // fresh v1 recovery token; only the independent release quorum stops it.
    let mut stale: Value = serde_json::from_str(&raw(world.store.as_ref().unwrap())).unwrap();
    stale["observedTime"] = json!(world.now);
    world.checkpoint = serde_json::to_vec(&stale).unwrap();
    let forged = world.envelope(&request, &world.checkpoint, &[1, 2]);
    assert_eq!(
        world.recover(&forged).err(),
        Some(PackageError::SignatureInvalid)
    );
    world.blocked();
    assert!(world.value(RECEIPT).is_none());
}

#[test]
fn absent_authority_and_overlapping_roles_refuse_without_enabling_recovery() {
    let fixture = Fixture::new();
    let mut world = World::new(false, false);
    let ordinary = manager(&fixture);
    assert_eq!(
        ordinary
            .begin_trust_recovery(world.store.as_mut().unwrap(), world.now)
            .err()
            .map(|e| e.code()),
        Some("PACKAGE_TRUST_RECOVERY_AUTHORITY_REQUIRED")
    );
    assert!(world.value(REQUEST).is_none());
    let pins = || BootstrapPins {
        keys: vec![public(1), public(2)],
        threshold: 2,
    };
    assert_eq!(
        PackageTrustStore::with_recovery_authority(pins(), pins())
            .err()
            .map(|e| e.code()),
        Some("PACKAGE_TRUST_INVALID")
    );
    let shared = PackageTrustStore::with_recovery_authority(
        pins(),
        BootstrapPins {
            keys: vec![public(3), public(6)],
            threshold: 2,
        },
    )
    .unwrap();
    let mut store = DataStore::memory().unwrap();
    assert_eq!(
        shared
            .bootstrap(
                &mut store,
                &serde_json::to_vec(&fixture.golden["rootEnvelope"]).unwrap(),
                world.now
            )
            .err(),
        Some(PackageError::TrustInvalid)
    );
    let mut root = next_root(&fixture);
    root.root_keys = vec![public(6), public(7)];
    let mut store = DataStore::memory().unwrap();
    let trust = recovery_manager(&fixture);
    bootstrap(&mut store, &trust, &fixture);
    assert_eq!(
        trust
            .rotate(
                &mut store,
                &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2, 6, 7]),
                world.now
            )
            .err(),
        Some(PackageError::TrustInvalid)
    );
    assert_eq!(trust.snapshot(&mut store).unwrap().root_version, 1);
}

#[test]
fn publisher_one_root_key_duplicate_votes_and_wrong_dsse_domain_cannot_approve() {
    let mut world = World::new(false, false);
    let request = world.challenge();
    let before = raw(world.store.as_ref().unwrap());
    for seeds in [
        &[3][..],
        &[1][..],
        &[1, 1][..],
        &[3, 1][..],
        &[1, 2][..],
        &[6, 7][..],
        &[1, 2, 6, 6][..],
    ] {
        let envelope = world.envelope(&request, &world.checkpoint, seeds);
        assert_eq!(
            world.recover(&envelope).err(),
            Some(PackageError::SignatureInvalid)
        );
        assert_eq!(raw(world.store.as_ref().unwrap()), before);
        assert!(world.value(RECEIPT).is_none());
        world.blocked();
    }
    let mut envelope: Value = serde_json::from_slice(&world.signed(&request)).unwrap();
    envelope["payloadType"] = json!(PACKAGE_PAYLOAD_TYPE);
    assert_eq!(
        world.recover(&serde_json::to_vec(&envelope).unwrap()).err(),
        Some(PackageError::EnvelopeInvalid)
    );
    world.blocked();
}

#[test]
fn approval_binds_exact_checkpoint_nonce_restore_and_profile() {
    let mut a = World::new(false, false);
    let mut b = World::new(false, false);
    let request = a.challenge();
    let approval = a.signed(&request);
    b.challenge();
    assert_eq!(b.recover(&approval).err(), Some(PackageError::TrustInvalid));
    let original = a.checkpoint.clone();
    a.checkpoint.push(b' ');
    assert_eq!(a.recover(&approval).err(), Some(PackageError::TrustInvalid));
    a.checkpoint = original;
    a.replace(MARKER, &uuid::Uuid::new_v4().to_string());
    assert_eq!(a.recover(&approval).err(), Some(PackageError::TrustInvalid));
    let refreshed = a.challenge();
    assert_ne!(request.id(), refreshed.id());
    assert_eq!(a.recover(&approval).err(), Some(PackageError::TrustInvalid));
    a.blocked();
    b.blocked();
}

#[test]
fn dropping_restored_observations_or_root_history_is_refused_even_with_valid_quorum() {
    let mut world = World::new(false, false);
    let request = world.challenge();
    let original = world.checkpoint.clone();
    let mut state: Value = serde_json::from_slice(&original).unwrap();
    state["knownVersions"] = json!([]);
    world.checkpoint = serde_json::to_vec(&state).unwrap();
    let envelope = world.signed(&request);
    assert_eq!(
        world.recover(&envelope).err(),
        Some(PackageError::RootRollback)
    );
    state = serde_json::from_slice(&original).unwrap();
    state["roots"][0]["observedAt"] = json!(Fixture::new().policy.clock.now + 1);
    world.checkpoint = serde_json::to_vec(&state).unwrap();
    let envelope = world.signed(&request);
    assert_eq!(
        world.recover(&envelope).err(),
        Some(PackageError::RootRollback)
    );
    world.blocked();
}

#[test]
fn fresh_approval_refuses_expiry_clock_rollback_and_unbounded_validity() {
    let mut world = World::new(false, false);
    let request = world.challenge();
    let envelope = world.signed(&request);
    let clock = world.now;
    world.now = clock - 2;
    assert_eq!(
        world.recover(&envelope).err(),
        Some(PackageError::ClockRollback)
    );
    world.now = clock + 61;
    assert_eq!(
        world.recover(&envelope).err(),
        Some(PackageError::TrustExpired)
    );
    world.now = clock;
    let mut value: Value = serde_json::from_slice(&envelope).unwrap();
    let decoded = decode_base64(value["payload"].as_str().unwrap()).unwrap();
    let mut payload: Value = serde_json::from_slice(&decoded).unwrap();
    payload["expiresAt"] = json!(clock + 301);
    let extended = signed(RECOVERY_PAYLOAD_TYPE, &payload, &[1, 2, 6, 7]);
    assert_eq!(
        world.recover(&extended).err(),
        Some(PackageError::PayloadInvalid)
    );
    payload["expiresAt"] = json!(clock + 60);
    payload["approved"] = json!(true);
    value =
        serde_json::from_slice(&signed(RECOVERY_PAYLOAD_TYPE, &payload, &[1, 2, 6, 7])).unwrap();
    assert_eq!(
        world.recover(&serde_json::to_vec(&value).unwrap()).err(),
        Some(PackageError::PayloadInvalid)
    );
    world.blocked();
}

#[test]
fn modified_restored_state_and_corrupt_or_over_budget_checkpoints_fail_without_receipt() {
    let mut world = World::new(false, false);
    let request = world.challenge();
    let envelope = world.signed(&request);
    let before = raw(world.store.as_ref().unwrap());
    world.replace("package-trust-state-v1", &(before.clone() + " "));
    assert_eq!(
        world.recover(&envelope).err(),
        Some(PackageError::TrustInvalid)
    );
    world.replace("package-trust-state-v1", &before);
    let original = world.checkpoint.clone();
    world.checkpoint = b"{broken".to_vec();
    assert_eq!(
        world.recover(&envelope).err(),
        Some(PackageError::TrustStateInvalid)
    );
    world.checkpoint = vec![b' '; 8_388_609];
    assert_eq!(
        world.recover(&envelope).err(),
        Some(PackageError::TrustBudgetExceeded)
    );
    world.checkpoint = original;
    assert_eq!(raw(world.store.as_ref().unwrap()), before);
    assert!(world.value(RECEIPT).is_none());
    world.blocked();
}

#[test]
fn commit_resamples_t0_clock_and_refuses_approval_that_expires_after_staging() {
    for offset in [61, -1, 1] {
        let mut world = World::new(false, false);
        let request = world.challenge();
        let approval = world.signed(&request);
        let before = raw(world.store.as_ref().unwrap());
        let clock = std::cell::Cell::new(world.now);
        let result = world.trust.recover_with_hook(
            world.store.as_mut().unwrap(),
            &world.checkpoint,
            &approval,
            || clock.get(),
            || {
                clock.set(world.now.checked_add_signed(offset).unwrap());
                Ok(())
            },
        );
        if offset == 1 {
            assert_eq!(result.unwrap().observed_time, world.now + 1);
            assert_eq!(
                world
                    .trust
                    .snapshot(world.store.as_mut().unwrap())
                    .unwrap()
                    .observed_time,
                world.now + 1
            );
        } else {
            assert_eq!(
                result.err(),
                Some(if offset == -1 {
                    PackageError::ClockRollback
                } else {
                    PackageError::TrustExpired
                })
            );
            assert_eq!(raw(world.store.as_ref().unwrap()), before);
            assert!(world.value(RECEIPT).is_none());
            assert!(world.value(REQUEST).is_some());
            world.blocked();
        }
    }
}

#[test]
fn state_receipt_and_request_commit_atomically_on_real_sqlite_write_failures() {
    for trigger in [
        "CREATE TRIGGER reject_receipt BEFORE INSERT ON core_metadata WHEN NEW.key='package-trust-recovery-receipt-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END;",
        "CREATE TRIGGER reject_request_delete BEFORE DELETE ON core_metadata WHEN OLD.key='package-trust-recovery-request-v1' BEGIN SELECT RAISE(ABORT,'fixture'); END;",
    ] {
        let mut world = World::new(false, false);
        let request = world.challenge();
        let envelope = world.signed(&request);
        let before = raw(world.store.as_ref().unwrap());
        let pending = world.value(REQUEST);
        world.store.as_ref().unwrap().connection.execute_batch(trigger).unwrap();
        assert_eq!(world.recover(&envelope).err(), Some(PackageError::TrustStorageFailed));
        assert_eq!(raw(world.store.as_ref().unwrap()), before);
        assert_eq!(world.value(REQUEST), pending);
        assert!(world.value(RECEIPT).is_none());
        world.blocked();
    }
}

#[test]
fn tampered_receipt_or_reduced_mutable_floors_are_refused_on_every_load() {
    let mut world = World::new(false, false);
    let request = world.challenge();
    let approval = world.signed(&request);
    world.recover(&approval).unwrap();
    let receipt = world.value(RECEIPT).unwrap();
    world.replace(RECEIPT, "{}");
    assert_eq!(
        world.trust.snapshot(world.store.as_mut().unwrap()).err(),
        Some(PackageError::TrustStateInvalid)
    );
    world.replace(RECEIPT, &receipt);
    let before = raw(world.store.as_ref().unwrap());
    let mut state: Value = serde_json::from_str(&before).unwrap();
    state["knownVersions"].as_array_mut().unwrap().pop();
    world.replace(
        "package-trust-state-v1",
        &serde_json::to_string(&state).unwrap(),
    );
    assert_eq!(
        world.trust.snapshot(world.store.as_mut().unwrap()).err(),
        Some(PackageError::RootRollback)
    );
    world.replace("package-trust-state-v1", &before);
    world.trust.snapshot(world.store.as_mut().unwrap()).unwrap();
}

#[test]
fn restoring_again_invalidates_old_approval_and_preserves_signed_receipt_floor() {
    let mut world = World::new(false, false);
    let request = world.challenge();
    let approval = world.signed(&request);
    world.recover(&approval).unwrap();
    let backup = world.store.as_ref().unwrap().backup().unwrap();
    let old_marker = world.value(MARKER);
    world.store.take();
    DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup).unwrap();
    world.store = Some(DataStore::open(&world.path.join("runtime.sqlite")).unwrap());
    assert_ne!(world.value(MARKER), old_marker);
    world.blocked();
    let next = world.challenge();
    assert_ne!(next.id(), request.id());
    assert_eq!(
        world.recover(&approval).err(),
        Some(PackageError::RootRollback)
    );
    let candidate: Value = serde_json::from_slice(&world.checkpoint).unwrap();
    assert!(candidate["observedTime"].as_u64().unwrap() < world.now);
    let old = world.signed(&next);
    assert_eq!(world.recover(&old).err(), Some(PackageError::RootRollback));
    let mut refreshed = candidate;
    refreshed["observedTime"] = json!(world.now);
    world.checkpoint = serde_json::to_vec(&refreshed).unwrap();
    assert_eq!(
        world.recover(&approval).err(),
        Some(PackageError::TrustInvalid)
    );
    let current = world.signed(&next);
    world.recover(&current).unwrap();
}

#[test]
fn managed_restore_generation_is_retained_and_remains_bound_to_the_receipt() {
    let mut world = World::new(false, false);
    // Actual manager restore emits both markers; use the real owned backup.
    let backup = world.store.as_ref().unwrap().backup().unwrap();
    let id = backup
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("runtime-backup-")
        .unwrap()
        .strip_suffix(".sqlite")
        .unwrap()
        .to_owned();
    world.store.take();
    crate::recovery::manage(&world.path, StorageAction::Restore { backup_id: id }).unwrap();
    world.store = Some(DataStore::open(&world.path.join("runtime.sqlite")).unwrap());
    let generation = world.value("recovery-generation").unwrap();
    let request = world.challenge();
    let approval = world.signed(&request);
    world.recover(&approval).unwrap();
    assert_eq!(world.value("recovery-generation"), Some(generation));
    world.replace("recovery-generation", &uuid::Uuid::new_v4().to_string());
    world.blocked();
}

#[test]
fn recovery_crash_child() {
    let Ok(path) = std::env::var("FLOWTOOLS_TRUST_RECOVERY_TEST_PROFILE") else {
        return;
    };
    let mode = std::env::var("FLOWTOOLS_TRUST_RECOVERY_TEST_MODE").unwrap();
    let path = PathBuf::from(path);
    assert_eq!(path.parent().unwrap(), std::env::temp_dir());
    assert!(path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("flowtools-validation-package-trust-"));
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    let fixture = Fixture::new();
    let trust = recovery_manager(&fixture);
    let checkpoint = std::fs::read(path.join("recovery-checkpoint.json")).unwrap();
    let envelope = std::fs::read(path.join("recovery-approval.json")).unwrap();
    let now = fixture.policy.clock.now + 4;
    if mode == "before" {
        trust
            .recover_with_hook(
                &mut store,
                &checkpoint,
                &envelope,
                || now,
                || std::process::exit(23),
            )
            .unwrap();
    } else {
        trust
            .recover_trust(&mut store, &checkpoint, &envelope, || now)
            .unwrap();
        std::process::exit(23);
    }
}

#[test]
fn real_process_exit_preserves_pending_or_complete_recovery_without_partial_receipt() {
    for mode in ["before", "after"] {
        let mut world = World::new(false, false);
        let request = world.challenge();
        let approval = world.signed(&request);
        std::fs::write(
            world.path.join("recovery-checkpoint.json"),
            &world.checkpoint,
        )
        .unwrap();
        std::fs::write(world.path.join("recovery-approval.json"), &approval).unwrap();
        world.store.take();
        let result = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "packages::tests::trust_tests::recovery_tests::recovery_crash_child",
            ])
            .env("FLOWTOOLS_TRUST_RECOVERY_TEST_PROFILE", &world.path)
            .env("FLOWTOOLS_TRUST_RECOVERY_TEST_MODE", mode)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(result.code(), Some(23));
        world.store = Some(DataStore::open(&world.path.join("runtime.sqlite")).unwrap());
        if mode == "before" {
            world.blocked();
            assert!(world.value(REQUEST).is_some());
            assert!(world.value(RECEIPT).is_none());
            world.recover(&approval).unwrap();
        } else {
            assert!(world.value(REQUEST).is_none());
            assert!(world.value(RECEIPT).is_some());
            assert_eq!(
                world
                    .trust
                    .snapshot(world.store.as_mut().unwrap())
                    .unwrap()
                    .known_versions
                    .len(),
                2
            );
        }
    }
}
