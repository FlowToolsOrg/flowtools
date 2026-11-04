//! Real signed descriptors and one writer; these proofs are not execution grants.
use super::*;
use crate::packages::trust::HeldDescriptor;
fn ready() -> (Fixture, PackageTrustStore, DataStore, HeldDescriptor) {
    let fixture = Fixture::new();
    let trust = manager(&fixture);
    let mut store = DataStore::open(&profile().join("runtime.sqlite")).unwrap();
    bootstrap(&mut store, &trust, &fixture);
    let original = verify(&mut store, &trust, &fixture, fixture.policy.clock.now).unwrap();
    let held = trust
        .hold_current_descriptor(&mut store, &original, fixture.policy.clock.now)
        .unwrap();
    (fixture, trust, store, held)
}
fn check_bytes(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    held: &HeldDescriptor,
    fixture: &Fixture,
    now: u64,
) -> Result<VerifiedPackageDescriptor, PackageError> {
    let envelope = signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[3]);
    trust.verify_held_descriptor(store, held, input(fixture, &envelope, now))
}
#[test]
fn held_exact_descriptor_survives_floor_advance_without_changing_floors_or_normal_admission() {
    let (fixture, trust, mut store, held) = ready();
    let now = fixture.policy.clock.now;
    let newer = revision("2.0.0", 2);
    verify(&mut store, &trust, &newer, now + 1).unwrap();
    let before = raw(&store);
    assert_eq!(
        verify(&mut store, &trust, &fixture, now + 1).err(),
        Some(PackageError::ReleaseRollback)
    );
    assert_eq!(
        trust
            .check_receipts(
                &mut store,
                &[held.original()],
                &fixture.package.target,
                now + 1
            )
            .err(),
        Some(PackageError::ReleaseRollback)
    );
    assert_eq!(
        trust
            .hold_current_descriptor(&mut store, held.original(), now + 1)
            .err(),
        Some(PackageError::ReleaseRollback)
    );
    trust
        .check_held_descriptor(&mut store, &held, now + 1)
        .unwrap();
    let current = check_bytes(&trust, &mut store, &held, &fixture, now + 1).unwrap();
    assert_eq!(current.package_digest(), held.original().package_digest());
    assert_eq!(
        current.descriptor_digest(),
        held.original().descriptor_digest()
    );
    assert_eq!(raw(&store), before);
    assert_eq!(trust.snapshot(&mut store).unwrap().known_versions.len(), 2);
}
#[test]
fn held_proof_never_renews_signed_metadata_or_selects_other_artifacts() {
    let (fixture, trust, mut store, held) = ready();
    let now = fixture.policy.clock.now;
    let mut renewed = Fixture::new();
    renewed.package.expires_at += 1;
    assert_eq!(
        check_bytes(&trust, &mut store, &held, &renewed, now).err(),
        Some(PackageError::TrustInvalid)
    );
    let other = revision("2.0.0", 2);
    verify(&mut store, &trust, &other, now).unwrap();
    assert_eq!(
        check_bytes(&trust, &mut store, &held, &other, now).err(),
        Some(PackageError::TrustInvalid)
    );
    assert_eq!(
        held.original().descriptor_digest(),
        fixture.verify().unwrap().descriptor_digest()
    );
}
#[test]
fn held_verification_still_checks_actual_signature_bytes_and_target() {
    let (fixture, trust, mut store, held) = ready();
    let now = fixture.policy.clock.now;
    let envelope = signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[3]);
    let mut archive = fixture.archive.clone();
    archive[0] ^= 1;
    let mut request = input(&fixture, &envelope, now);
    request.archive = &archive;
    assert_eq!(
        trust
            .verify_held_descriptor(&mut store, &held, request)
            .err(),
        Some(PackageError::IntegrityMismatch)
    );
    let mut manifest = fixture.manifest.clone();
    manifest[0] ^= 1;
    let mut request = input(&fixture, &envelope, now);
    request.manifest = &manifest;
    assert_eq!(
        trust
            .verify_held_descriptor(&mut store, &held, request)
            .err(),
        Some(PackageError::IntegrityMismatch)
    );
    let wrong = signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[1]);
    assert_eq!(
        trust
            .verify_held_descriptor(&mut store, &held, input(&fixture, &wrong, now))
            .err(),
        Some(PackageError::SignatureInvalid)
    );
    let mut request = input(&fixture, &envelope, now);
    request.target.arch = PackageArch::Arm64;
    assert_eq!(
        trust
            .verify_held_descriptor(&mut store, &held, request)
            .err(),
        Some(PackageError::TargetMismatch)
    );
}
#[test]
fn root_rotation_and_revocation_never_borrow_an_old_verification() {
    for revoke in [false, true] {
        let (fixture, trust, mut store, held) = ready();
        let now = fixture.policy.clock.now;
        let mut next = next_root(&fixture);
        if revoke {
            next.revoked_package_digests
                .push(fixture.package.archive.sha256.clone());
        }
        trust
            .rotate(
                &mut store,
                &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2, 4, 5]),
                now + 1,
            )
            .unwrap();
        let expected = if revoke {
            PackageError::PackageRevoked
        } else {
            PackageError::TrustInvalid
        };
        assert_eq!(
            trust
                .check_held_descriptor(&mut store, &held, now + 1)
                .err(),
            Some(expected)
        );
        assert_eq!(
            check_bytes(&trust, &mut store, &held, &fixture, now + 1).err(),
            Some(expected)
        );
    }
}
#[test]
fn writer_reopen_and_other_profile_cannot_reuse_an_in_memory_proof() {
    let (fixture, trust, mut store, held) = ready();
    let now = fixture.policy.clock.now;
    let path = store.database_path().unwrap().to_owned();
    let old_writer = store.writer_instance();
    let mut other = DataStore::open(&profile().join("runtime.sqlite")).unwrap();
    bootstrap(&mut other, &trust, &fixture);
    verify(&mut other, &trust, &fixture, now).unwrap();
    let before = raw(&other);
    assert_eq!(
        trust.check_held_descriptor(&mut other, &held, now).err(),
        Some(PackageError::TrustInvalid)
    );
    assert_eq!(raw(&other), before);
    drop(store);
    store = DataStore::open(&path).unwrap();
    assert_ne!(store.writer_instance(), old_writer);
    assert_eq!(
        check_bytes(&trust, &mut store, &held, &fixture, now).err(),
        Some(PackageError::TrustInvalid)
    );
    let original = verify(&mut store, &trust, &fixture, now).unwrap();
    let fresh = trust
        .hold_current_descriptor(&mut store, &original, now)
        .unwrap();
    trust
        .check_held_descriptor(&mut store, &fresh, now)
        .unwrap();
}
#[test]
fn restore_generation_and_bounded_malformed_metadata_refuse_without_reset() {
    for generation in [json!("new-generation"), json!(""), json!("x".repeat(129))] {
        let (fixture, trust, mut store, held) = ready();
        store
            .connection
            .execute(
                "INSERT INTO core_metadata(key,value) VALUES('recovery-generation',?1)",
                [generation.as_str().unwrap()],
            )
            .unwrap();
        assert_eq!(
            trust
                .check_held_descriptor(&mut store, &held, fixture.policy.clock.now)
                .err(),
            Some(PackageError::TrustInvalid)
        );
    }
    let (fixture, trust, mut store, held) = ready();
    store
        .connection
        .execute(
            "INSERT INTO core_metadata(key,value) VALUES('recovery-generation',zeroblob(1048576))",
            [],
        )
        .unwrap();
    assert_eq!(
        check_bytes(
            &trust,
            &mut store,
            &held,
            &fixture,
            fixture.policy.clock.now
        )
        .err(),
        Some(PackageError::TrustInvalid)
    );
    let size: i64 = store
        .connection
        .query_row(
            "SELECT length(value) FROM core_metadata WHERE key='recovery-generation'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(size, 1048576);
}
#[test]
fn expiry_and_durable_time_floor_remain_fatal_for_a_held_descriptor() {
    let (fixture, trust, mut store, held) = ready();
    let now = fixture.policy.clock.now;
    trust
        .check_held_descriptor(&mut store, &held, now + 10)
        .unwrap();
    assert_eq!(
        trust
            .check_held_descriptor(&mut store, &held, now + 9)
            .err(),
        Some(PackageError::ClockRollback)
    );
    assert_eq!(
        check_bytes(
            &trust,
            &mut store,
            &held,
            &fixture,
            fixture.package.expires_at + 1
        )
        .err(),
        Some(PackageError::TrustExpired)
    );
}
#[test]
fn missing_original_identity_and_memory_store_never_create_continuation() {
    let (fixture, trust, mut store, held) = ready();
    let now = fixture.policy.clock.now;
    let mut state: Value = serde_json::from_str(&raw(&store)).unwrap();
    state["knownVersions"] = json!([]);
    replace_state(&store, &state);
    assert_eq!(
        trust.check_held_descriptor(&mut store, &held, now).err(),
        Some(PackageError::TrustInvalid)
    );
    assert_eq!(
        check_bytes(&trust, &mut store, &held, &fixture, now).err(),
        Some(PackageError::TrustInvalid)
    );
    let mut memory = DataStore::memory().unwrap();
    bootstrap(&mut memory, &trust, &fixture);
    let original = verify(&mut memory, &trust, &fixture, now).unwrap();
    assert_eq!(
        trust
            .hold_current_descriptor(&mut memory, &original, now)
            .err(),
        Some(PackageError::TrustInvalid)
    );
}
