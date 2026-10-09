use super::*;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;

struct Fixture {
    golden: Value,
    root: VerifiedRoot,
    policy: AdmissionPolicy,
    package: PackagePayload,
    manifest: Vec<u8>,
    archive: Vec<u8>,
}

impl Fixture {
    fn new() -> Self {
        let golden: Value = serde_json::from_str(include_str!(
            "../../../sdk/test/fixtures/signed-package-v1.json"
        ))
        .unwrap();
        let now = golden["now"].as_u64().unwrap();
        let pins = BootstrapPins {
            keys: serde_json::from_value(golden["pins"].clone()).unwrap(),
            threshold: golden["threshold"].as_u64().unwrap() as usize,
        };
        let clock = VerificationClock { now, floor: now };
        let root = bootstrap_root(
            &serde_json::to_vec(&golden["rootEnvelope"]).unwrap(),
            &pins,
            clock,
        )
        .unwrap();
        let package: PackagePayload = serde_json::from_value(golden["descriptor"].clone()).unwrap();
        let policy = AdmissionPolicy {
            target: package.target.clone(),
            clock,
            known_versions: Vec::new(),
            highest_release_sequence: 0,
            approved_rollback_digest: None,
        };
        let manifest = decode_base64(golden["manifestBase64"].as_str().unwrap()).unwrap();
        let archive = decode_base64(golden["archiveBase64"].as_str().unwrap()).unwrap();
        Self {
            golden,
            root,
            policy,
            package,
            manifest,
            archive,
        }
    }

    fn verify(&self) -> Result<VerifiedPackageDescriptor, PackageError> {
        verify_package(
            &self.root,
            &signed(PACKAGE_PAYLOAD_TYPE, &self.package, &[3]),
            &self.manifest,
            &self.archive,
            &self.policy,
        )
    }

    fn known(&self, digest: &str) -> KnownVersionIdentity {
        KnownVersionIdentity {
            kind: self.package.kind.clone(),
            publisher: self.package.publisher.clone(),
            id: self.package.id.clone(),
            target: self.package.target.clone(),
            version: self.package.version.clone(),
            release_sequence: self.package.release_sequence,
            package_digest: digest.into(),
        }
    }
}

fn public(seed: u8) -> PublicKey {
    let bytes = SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes();
    PublicKey {
        keyid: hash(&bytes),
        public_key: general_purpose::STANDARD.encode(bytes),
    }
}

fn signed<T: Serialize>(payload_type: &str, payload: &T, seeds: &[u8]) -> Vec<u8> {
    signed_bytes(payload_type, &serde_json::to_vec(payload).unwrap(), seeds)
}

fn signed_bytes(payload_type: &str, bytes: &[u8], seeds: &[u8]) -> Vec<u8> {
    let mut pae = format!(
        "DSSEv1 {} {} {} ",
        payload_type.len(),
        payload_type,
        bytes.len()
    )
    .into_bytes();
    pae.extend_from_slice(bytes);
    let signatures: Vec<_> = seeds.iter().map(|seed| {
        let signing = SigningKey::from_bytes(&[*seed; 32]);
        json!({"keyid":public(*seed).keyid,"sig":general_purpose::STANDARD.encode(signing.sign(&pae).to_bytes())})
    }).collect();
    serde_json::to_vec(&json!({"payloadType":payload_type,"payload":general_purpose::STANDARD.encode(bytes),"signatures":signatures})).unwrap()
}

fn next_root(fixture: &Fixture) -> RootPayload {
    let mut root = fixture.root.payload().clone();
    root.version += 1;
    root.previous_root_sha256 = Some(fixture.root.payload_digest().into());
    root.root_keys = vec![public(4), public(5)];
    root
}

#[test]
fn node_golden_signatures_raw_bytes_pae_and_receipts_agree() {
    let fixture = Fixture::new();
    let envelope = serde_json::to_vec(&fixture.golden["packageEnvelope"]).unwrap();
    let decoded = decode_envelope(&envelope, PACKAGE_PAYLOAD_TYPE).unwrap();
    assert_eq!(
        general_purpose::STANDARD.encode(&decoded.pae),
        fixture.golden["paeBase64"]
    );
    assert_eq!(fixture.root.payload_digest(), fixture.golden["rootDigest"]);
    let receipt = verify_package(
        &fixture.root,
        &envelope,
        &fixture.manifest,
        &fixture.archive,
        &fixture.policy,
    )
    .unwrap();
    assert_eq!(receipt.package_digest(), fixture.golden["packageDigest"]);
    assert_eq!(
        receipt.descriptor_digest(),
        fixture.golden["descriptorDigest"]
    );
    assert_eq!(receipt.root_digest(), fixture.root.payload_digest());
    assert_eq!(receipt.verified_at(), fixture.policy.clock.now);
    assert_eq!(receipt.payload().publisher, "fixture-publisher");
    // Byte proof deliberately reads a canary as inert archive data. No import,
    // extraction, filesystem/metadata commit or broker operation is available.
    assert!(fixture
        .archive
        .windows(24)
        .any(|bytes| bytes == b"FIXTURE_MUST_NOT_EXECUTE"));
}

#[test]
fn envelope_extensions_optional_keyid_and_all_standard_base64_forms_work() {
    let fixture = Fixture::new();
    for engine in [
        general_purpose::STANDARD,
        general_purpose::STANDARD_NO_PAD,
        general_purpose::URL_SAFE,
        general_purpose::URL_SAFE_NO_PAD,
    ] {
        let mut envelope = fixture.golden["packageEnvelope"].clone();
        let payload = decode_base64(envelope["payload"].as_str().unwrap()).unwrap();
        envelope["payload"] = json!(engine.encode(payload));
        let sig = decode_base64(envelope["signatures"][0]["sig"].as_str().unwrap()).unwrap();
        envelope["signatures"][0]["sig"] = json!(engine.encode(sig));
        envelope["signatures"][0]
            .as_object_mut()
            .unwrap()
            .remove("keyid");
        envelope["extension"] = json!({"untrusted":"ignored"});
        envelope["signatures"][0]["extension"] = json!(true);
        verify_package(
            &fixture.root,
            &serde_json::to_vec(&envelope).unwrap(),
            &fixture.manifest,
            &fixture.archive,
            &fixture.policy,
        )
        .unwrap();
    }
}

#[test]
fn raw_payload_bytes_and_signed_type_cannot_be_substituted() {
    let fixture = Fixture::new();
    let mut envelope = fixture.golden["packageEnvelope"].clone();
    let mut payload = decode_base64(envelope["payload"].as_str().unwrap()).unwrap();
    payload.push(b' ');
    envelope["payload"] = json!(general_purpose::STANDARD.encode(&payload));
    assert_eq!(
        verify_package(
            &fixture.root,
            &serde_json::to_vec(&envelope).unwrap(),
            &fixture.manifest,
            &fixture.archive,
            &fixture.policy
        )
        .unwrap_err(),
        PackageError::SignatureInvalid
    );
    // Valid JSON whitespace is acceptable when the exact new bytes are signed,
    // and intentionally has different metadata from the original payload.
    let receipt = verify_package(
        &fixture.root,
        &signed_bytes(PACKAGE_PAYLOAD_TYPE, &payload, &[3]),
        &fixture.manifest,
        &fixture.archive,
        &fixture.policy,
    )
    .unwrap();
    assert_eq!(receipt.package_digest(), fixture.golden["packageDigest"]);
    assert_ne!(
        receipt.descriptor_digest(),
        fixture.golden["descriptorDigest"]
    );
    let mut envelope = fixture.golden["packageEnvelope"].clone();
    envelope["payloadType"] = json!(ROOT_PAYLOAD_TYPE);
    assert_eq!(
        verify_package(
            &fixture.root,
            &serde_json::to_vec(&envelope).unwrap(),
            &fixture.manifest,
            &fixture.archive,
            &fixture.policy
        )
        .unwrap_err(),
        PackageError::EnvelopeInvalid
    );
    let payload = decode_base64(
        fixture.golden["packageEnvelope"]["payload"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let mut wrong_domain: Value =
        serde_json::from_slice(&signed_bytes(ROOT_PAYLOAD_TYPE, &payload, &[3])).unwrap();
    wrong_domain["payloadType"] = json!(PACKAGE_PAYLOAD_TYPE);
    assert_eq!(
        verify_package(
            &fixture.root,
            &serde_json::to_vec(&wrong_domain).unwrap(),
            &fixture.manifest,
            &fixture.archive,
            &fixture.policy
        )
        .unwrap_err(),
        PackageError::SignatureInvalid
    );
}

#[test]
fn duplicate_fields_unknown_payload_and_malformed_budgets_fail_closed() {
    let fixture = Fixture::new();
    let bytes = serde_json::to_vec(&fixture.package).unwrap();
    let duplicate = [b"{\"id\":\"fixture-plugin\",".as_slice(), &bytes[1..]].concat();
    assert_eq!(
        verify_package(
            &fixture.root,
            &signed_bytes(PACKAGE_PAYLOAD_TYPE, &duplicate, &[3]),
            &fixture.manifest,
            &fixture.archive,
            &fixture.policy
        )
        .unwrap_err(),
        PackageError::PayloadInvalid
    );
    let duplicate_nested = String::from_utf8(bytes.clone()).unwrap().replace(
        "\"platform\":\"windows\"",
        "\"platform\":\"windows\",\"platform\":\"windows\"",
    );
    assert_eq!(
        verify_package(
            &fixture.root,
            &signed_bytes(PACKAGE_PAYLOAD_TYPE, duplicate_nested.as_bytes(), &[3]),
            &fixture.manifest,
            &fixture.archive,
            &fixture.policy
        )
        .unwrap_err(),
        PackageError::PayloadInvalid
    );
    let mut unknown = serde_json::to_value(&fixture.package).unwrap();
    unknown["grants"] = json!(["all"]);
    assert_eq!(
        verify_package(
            &fixture.root,
            &signed(PACKAGE_PAYLOAD_TYPE, &unknown, &[3]),
            &fixture.manifest,
            &fixture.archive,
            &fixture.policy
        )
        .unwrap_err(),
        PackageError::PayloadInvalid
    );
    for payload in [
        b"null".as_slice(),
        b"{\"formatVersion\":1e400}".as_slice(),
        b"[".as_slice(),
        b"\xff".as_slice(),
    ] {
        assert_eq!(
            verify_package(
                &fixture.root,
                &signed_bytes(PACKAGE_PAYLOAD_TYPE, payload, &[3]),
                &fixture.manifest,
                &fixture.archive,
                &fixture.policy
            )
            .unwrap_err(),
            PackageError::PayloadInvalid
        );
    }
    assert_eq!(
        decode_envelope(&vec![b' '; MAX_ENVELOPE_BYTES + 1], PACKAGE_PAYLOAD_TYPE).err(),
        Some(PackageError::EnvelopeInvalid)
    );
    let excessive = signed_bytes(
        PACKAGE_PAYLOAD_TYPE,
        &vec![b' '; MAX_PAYLOAD_BYTES + 1],
        &[3],
    );
    assert_eq!(
        decode_envelope(&excessive, PACKAGE_PAYLOAD_TYPE).err(),
        Some(PackageError::PayloadInvalid)
    );
    let mut envelope = fixture.golden["packageEnvelope"].clone();
    envelope["signatures"] = json!(vec![envelope["signatures"][0].clone(); 17]);
    assert_eq!(
        decode_envelope(
            &serde_json::to_vec(&envelope).unwrap(),
            PACKAGE_PAYLOAD_TYPE
        )
        .err(),
        Some(PackageError::EnvelopeInvalid)
    );
    envelope["signatures"] = json!([]);
    assert_eq!(
        decode_envelope(
            &serde_json::to_vec(&envelope).unwrap(),
            PACKAGE_PAYLOAD_TYPE
        )
        .err(),
        Some(PackageError::EnvelopeInvalid)
    );
    let duplicate = [
        b"{\"payloadType\":\"application/vnd.flowtools.package.v1+json\",".as_slice(),
        &serde_json::to_vec(&fixture.golden["packageEnvelope"]).unwrap()[1..],
    ]
    .concat();
    assert_eq!(
        decode_envelope(&duplicate, PACKAGE_PAYLOAD_TYPE).err(),
        Some(PackageError::EnvelopeInvalid)
    );
}

#[test]
fn package_and_root_identity_scope_must_come_from_trusted_signers() {
    let fixture = Fixture::new();
    assert_eq!(
        verify_package(
            &fixture.root,
            &signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[6]),
            &fixture.manifest,
            &fixture.archive,
            &fixture.policy
        )
        .unwrap_err(),
        PackageError::SignatureInvalid
    );
    let mut envelope = fixture.golden["packageEnvelope"].clone();
    envelope["signatures"][0]["keyid"] = json!(public(1).keyid);
    assert_eq!(
        verify_package(
            &fixture.root,
            &serde_json::to_vec(&envelope).unwrap(),
            &fixture.manifest,
            &fixture.archive,
            &fixture.policy
        )
        .unwrap_err(),
        PackageError::SignatureInvalid
    );
    for field in ["publisher", "id", "kind", "target"] {
        let mut package = fixture.package.clone();
        match field {
            "publisher" => package.publisher = "impostor".into(),
            "id" => package.id = "impostor".into(),
            "kind" => package.kind = PackageKind::Tool,
            "target" => package.target.arch = PackageArch::Arm64,
            _ => unreachable!(),
        }
        let mut policy = fixture.policy.clone();
        policy.target = package.target.clone();
        assert_eq!(
            verify_package(
                &fixture.root,
                &signed(PACKAGE_PAYLOAD_TYPE, &package, &[3]),
                &fixture.manifest,
                &fixture.archive,
                &policy
            )
            .unwrap_err(),
            PackageError::ScopeDenied
        );
    }
    let mut policy = fixture.policy.clone();
    policy.target.arch = PackageArch::Arm64;
    assert_eq!(
        verify_package(
            &fixture.root,
            &signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[3]),
            &fixture.manifest,
            &fixture.archive,
            &policy
        )
        .unwrap_err(),
        PackageError::TargetMismatch
    );
}

#[test]
fn threshold_counts_unique_keys_and_rejects_weak_or_role_reused_keys() {
    let fixture = Fixture::new();
    let pins = BootstrapPins {
        keys: vec![public(1), public(2)],
        threshold: 2,
    };
    assert_eq!(
        bootstrap_root(
            &signed(ROOT_PAYLOAD_TYPE, fixture.root.payload(), &[1, 1]),
            &pins,
            fixture.policy.clock
        )
        .unwrap_err(),
        PackageError::SignatureInvalid
    );
    for defect in [
        "same-key",
        "weak",
        "noncanonical",
        "keyid",
        "reuse",
        "threshold",
        "root-threshold",
    ] {
        let mut root = fixture.root.payload().clone();
        match defect {
            "same-key" => root.root_keys[1] = public(1),
            "weak" => {
                let bytes = [0; 32];
                root.root_keys[1] = PublicKey {
                    keyid: hash(&bytes),
                    public_key: general_purpose::STANDARD.encode(bytes),
                };
            }
            "noncanonical" => {
                let mut bytes = [255; 32];
                bytes[31] = 127;
                let parsed = VerifyingKey::from_bytes(&bytes).unwrap();
                assert!(!parsed.is_weak());
                assert_ne!(parsed.to_edwards().compress().to_bytes(), bytes);
                root.root_keys[1] = PublicKey {
                    keyid: hash(&bytes),
                    public_key: general_purpose::STANDARD.encode(bytes),
                };
            }
            "keyid" => root.root_keys[1].keyid = "0".repeat(64),
            "reuse" => root.publishers[0].keys[0] = public(1),
            "threshold" => root.publishers[0].threshold = 0,
            "root-threshold" => root.root_threshold = 1,
            _ => unreachable!(),
        }
        assert_eq!(
            bootstrap_root(
                &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]),
                &pins,
                fixture.policy.clock
            )
            .unwrap_err(),
            PackageError::TrustInvalid,
            "{defect}"
        );
    }
    assert_eq!(
        bootstrap_root(
            &signed(ROOT_PAYLOAD_TYPE, fixture.root.payload(), &[1, 2]),
            &BootstrapPins {
                keys: vec![public(1)],
                threshold: 1
            },
            fixture.policy.clock
        )
        .unwrap_err(),
        PackageError::TrustInvalid
    );
}

#[test]
fn root_rotation_requires_sequential_hash_link_and_both_thresholds() {
    let fixture = Fixture::new();
    let next = next_root(&fixture);
    let rotated = rotate_root(
        &fixture.root,
        &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2, 4, 5]),
        fixture.policy.clock,
    )
    .unwrap();
    assert_eq!(rotated.payload().version, 2);
    assert_eq!(
        rotated.payload().previous_root_sha256.as_deref(),
        Some(fixture.root.payload_digest())
    );
    for seeds in [
        &[1, 2][..],
        &[4, 5][..],
        &[1, 1, 4, 5][..],
        &[1, 2, 4, 4][..],
    ] {
        assert_eq!(
            rotate_root(
                &fixture.root,
                &signed(ROOT_PAYLOAD_TYPE, &next, seeds),
                fixture.policy.clock
            )
            .unwrap_err(),
            PackageError::SignatureInvalid
        );
    }
    for defect in ["version", "previous", "previous-time"] {
        let mut next = next.clone();
        match defect {
            "version" => next.version = 3,
            "previous" => next.previous_root_sha256 = Some("0".repeat(64)),
            "previous-time" => next.issued_at -= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            rotate_root(
                &fixture.root,
                &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2, 4, 5]),
                fixture.policy.clock
            )
            .unwrap_err(),
            PackageError::RootRollback
        );
    }
    assert_eq!(
        rotate_root(
            &rotated,
            &signed(ROOT_PAYLOAD_TYPE, fixture.root.payload(), &[1, 2]),
            fixture.policy.clock
        )
        .unwrap_err(),
        PackageError::RootRollback
    );
}

#[test]
fn expired_root_only_authenticates_fresh_rotation_recovery() {
    let fixture = Fixture::new();
    let mut policy = fixture.policy.clone();
    policy.clock.now = fixture.root.payload().expires_at;
    assert_eq!(
        verify_package(
            &fixture.root,
            &signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[3]),
            &fixture.manifest,
            &fixture.archive,
            &policy
        )
        .unwrap_err(),
        PackageError::TrustExpired
    );
    let mut next = next_root(&fixture);
    next.issued_at = policy.clock.now;
    next.expires_at = policy.clock.now + 60;
    rotate_root(
        &fixture.root,
        &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2, 4, 5]),
        policy.clock,
    )
    .unwrap();
    next.expires_at = policy.clock.now;
    assert_eq!(
        rotate_root(
            &fixture.root,
            &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2, 4, 5]),
            policy.clock
        )
        .unwrap_err(),
        PackageError::PayloadInvalid
    );
}

#[test]
fn revocation_is_cumulative_and_prevents_admission_or_approved_rollback() {
    let fixture = Fixture::new();
    let digest = fixture.verify().unwrap().package_digest().to_owned();
    for revoke_key in [false, true] {
        let mut next = next_root(&fixture);
        if revoke_key {
            next.revoked_key_ids.push(public(3).keyid);
        } else {
            next.revoked_package_digests.push(digest.clone());
        }
        let root = rotate_root(
            &fixture.root,
            &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2, 4, 5]),
            fixture.policy.clock,
        )
        .unwrap();
        let mut policy = fixture.policy.clone();
        policy.highest_release_sequence = 2;
        policy.known_versions.push(fixture.known(&digest));
        policy.approved_rollback_digest = Some(digest.clone());
        assert_eq!(
            verify_package(
                &root,
                &signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[3]),
                &fixture.manifest,
                &fixture.archive,
                &policy
            )
            .unwrap_err(),
            if revoke_key {
                PackageError::KeyRevoked
            } else {
                PackageError::PackageRevoked
            }
        );
        let mut next = root.payload().clone();
        next.version += 1;
        next.previous_root_sha256 = Some(root.payload_digest().into());
        next.revoked_key_ids.clear();
        next.revoked_package_digests.clear();
        assert_eq!(
            rotate_root(
                &root,
                &signed(ROOT_PAYLOAD_TYPE, &next, &[4, 5]),
                policy.clock
            )
            .unwrap_err(),
            PackageError::RootRollback
        );
    }
}

#[test]
fn package_expiry_clock_floor_and_lifetime_budgets_are_not_bypassable() {
    let mut fixture = Fixture::new();
    fixture.policy.clock.floor += 1;
    assert_eq!(fixture.verify().unwrap_err(), PackageError::ClockRollback);
    fixture.policy.clock.floor -= 1;
    fixture.package.expires_at = fixture.policy.clock.now;
    assert_eq!(fixture.verify().unwrap_err(), PackageError::TrustExpired);
    fixture.package.expires_at = fixture.package.issued_at + MAX_PACKAGE_LIFETIME + 1;
    assert_eq!(fixture.verify().unwrap_err(), PackageError::PayloadInvalid);
    fixture.package.issued_at = fixture.policy.clock.now + 1;
    fixture.package.expires_at = fixture.policy.clock.now + 60;
    assert_eq!(fixture.verify().unwrap_err(), PackageError::TrustExpired);
    let mut root = fixture.root.payload().clone();
    root.expires_at = root.issued_at + MAX_ROOT_LIFETIME + 1;
    assert_eq!(
        bootstrap_root(
            &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]),
            &BootstrapPins {
                keys: vec![public(1), public(2)],
                threshold: 2
            },
            fixture.policy.clock
        )
        .unwrap_err(),
        PackageError::PayloadInvalid
    );
}

#[test]
fn immutable_versions_release_floors_and_exact_known_good_rollback() {
    let mut fixture = Fixture::new();
    let digest = fixture.verify().unwrap().package_digest().to_owned();
    fixture.policy.known_versions.push(fixture.known(&digest));
    fixture.policy.highest_release_sequence = 1;
    fixture.verify().unwrap();
    fixture.package.build_flavor = "changed".into();
    fixture.verify().unwrap();
    fixture.archive.push(b'!');
    fixture.package.archive.sha256 = hash(&fixture.archive);
    fixture.package.archive.size = fixture.archive.len() as u64;
    assert_eq!(fixture.verify().unwrap_err(), PackageError::VersionConflict);
    fixture.package.build_flavor = "default".into();
    fixture.archive.pop();
    fixture.package.archive.sha256 = hash(&fixture.archive);
    fixture.package.archive.size = fixture.archive.len() as u64;
    fixture.policy.highest_release_sequence = 2;
    assert_eq!(fixture.verify().unwrap_err(), PackageError::ReleaseRollback);
    fixture.package.release_sequence = 3;
    assert_eq!(fixture.verify().unwrap_err(), PackageError::VersionConflict);
    fixture.package.release_sequence = 1;
    fixture.policy.approved_rollback_digest = Some("0".repeat(64));
    assert_eq!(fixture.verify().unwrap_err(), PackageError::ReleaseRollback);
    fixture.policy.approved_rollback_digest = Some(digest);
    fixture.verify().unwrap();
    fixture.policy.known_versions.clear();
    assert_eq!(fixture.verify().unwrap_err(), PackageError::ReleaseRollback);
}

#[test]
fn actual_manifest_and_archive_hash_size_and_header_are_checked() {
    for defect in [
        "manifest-byte",
        "archive-byte",
        "manifest-size",
        "archive-size",
        "manifest-publisher",
        "manifest-id",
        "manifest-version",
        "manifest-target",
        "manifest-files",
    ] {
        let mut fixture = Fixture::new();
        let expected = if defect.starts_with("manifest-")
            && !matches!(defect, "manifest-byte" | "manifest-size")
        {
            let mut value: Value = serde_json::from_slice(&fixture.manifest).unwrap();
            match defect {
                "manifest-publisher" => value["publisher"] = json!("impostor"),
                "manifest-id" => value["id"] = json!("impostor"),
                "manifest-version" => value["version"] = json!("2.0.0"),
                "manifest-target" => value["targets"][0]["arch"] = json!("arm64"),
                "manifest-files" => value["files"][0]["sha256"] = json!("0".repeat(64)),
                _ => unreachable!(),
            }
            fixture.manifest = serde_json::to_vec(&value).unwrap();
            fixture.package.manifest = BlobDescriptor {
                sha256: hash(&fixture.manifest),
                size: fixture.manifest.len() as u64,
            };
            PackageError::ManifestMismatch
        } else {
            match defect {
                "manifest-byte" => fixture.manifest[0] ^= 1,
                "archive-byte" => fixture.archive[0] ^= 1,
                "manifest-size" => fixture.package.manifest.size += 1,
                "archive-size" => fixture.package.archive.size += 1,
                _ => unreachable!(),
            }
            PackageError::IntegrityMismatch
        };
        assert_eq!(fixture.verify().unwrap_err(), expected, "{defect}");
    }
}

#[test]
fn signed_file_profile_rejects_windows_traversal_devices_collisions_and_bombs() {
    for path in [
        "/escape",
        "../escape",
        "dist/../escape",
        "c:/escape",
        "\\\\server\\escape",
        "dist:ads",
        "dist\\escape",
        "con",
        "con.txt",
        "com0.exe",
        "lpt9",
        "dist/file.",
        "dist/file ",
        "dist//file",
        "dist/./file",
        "dist/FILE",
        "dist/%2e",
        "dist/中文",
        "manifest.json",
        "manifest.json/child.js",
    ] {
        let mut fixture = Fixture::new();
        fixture.package.files[0].path = path.into();
        assert_eq!(
            fixture.verify().unwrap_err(),
            PackageError::PayloadInvalid,
            "{path}"
        );
    }
    for defect in [
        "duplicate",
        "prefix",
        "size",
        "total",
        "count",
        "hash",
        "archive-format",
        "semver",
        "sequence",
        "license",
    ] {
        let mut fixture = Fixture::new();
        match defect {
            "duplicate" => fixture.package.files.push(fixture.package.files[0].clone()),
            "prefix" => {
                let mut file = fixture.package.files[0].clone();
                file.path = "dist".into();
                fixture.package.files.push(file);
            }
            "size" => fixture.package.files[0].size = MAX_ARCHIVE_BYTES + 1,
            "total" => {
                fixture.package.files[0].size = MAX_ARCHIVE_BYTES;
                let mut file = fixture.package.files[0].clone();
                file.path = "extra".into();
                file.size = 1;
                fixture.package.files.push(file);
            }
            "count" => fixture.package.files = vec![fixture.package.files[0].clone(); 1025],
            "hash" => fixture.package.files[0].sha256 = "A".repeat(64),
            "archive-format" => fixture.package.archive.format = "tar".into(),
            "semver" => fixture.package.version = "1.0.0+local".into(),
            "sequence" => fixture.package.release_sequence = MAX_SAFE_INTEGER + 1,
            "license" => fixture.package.license = "../license".into(),
            _ => unreachable!(),
        }
        assert_eq!(
            fixture.verify().unwrap_err(),
            PackageError::PayloadInvalid,
            "{defect}"
        );
    }
}

#[test]
fn signed_tool_descriptor_uses_same_trust_path_without_enabling_tools() {
    let mut fixture = Fixture::new();
    let mut next = next_root(&fixture);
    next.publishers[0].packages[0].kind = PackageKind::Tool;
    fixture.root = rotate_root(
        &fixture.root,
        &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2, 4, 5]),
        fixture.policy.clock,
    )
    .unwrap();
    fixture.package.kind = PackageKind::Tool;
    let mut manifest: Value = serde_json::from_slice(&fixture.manifest).unwrap();
    manifest["target"] = serde_json::to_value(&fixture.package.target).unwrap();
    manifest.as_object_mut().unwrap().remove("targets");
    fixture.manifest = serde_json::to_vec(&manifest).unwrap();
    fixture.package.manifest = BlobDescriptor {
        sha256: hash(&fixture.manifest),
        size: fixture.manifest.len() as u64,
    };
    let receipt = fixture.verify().unwrap();
    assert_eq!(receipt.payload().kind, PackageKind::Tool);
    // This only proves signed descriptor and blob/header integrity. In this
    // fixture the inert ZIP contains another header; extraction/full matching
    // is intentionally P2.5b, so this receipt cannot be an install permit.
}

#[test]
fn renewed_metadata_keeps_artifact_identity_and_cannot_revive_revoked_zip() {
    let mut fixture = Fixture::new();
    let original = fixture.verify().unwrap();
    let package_digest = original.package_digest().to_owned();
    let descriptor_digest = original.descriptor_digest().to_owned();
    fixture
        .policy
        .known_versions
        .push(fixture.known(&package_digest));
    fixture.policy.highest_release_sequence = fixture.package.release_sequence;
    fixture.package.issued_at += 1;
    fixture.package.expires_at += 60;
    fixture.package.source.artifact = "renewed-metadata".into();
    let renewed = fixture.verify().unwrap();
    assert_eq!(renewed.package_digest(), package_digest);
    assert_ne!(renewed.descriptor_digest(), descriptor_digest);
    let mut next = next_root(&fixture);
    next.revoked_package_digests.push(package_digest);
    fixture.root = rotate_root(
        &fixture.root,
        &signed(ROOT_PAYLOAD_TYPE, &next, &[1, 2, 4, 5]),
        fixture.policy.clock,
    )
    .unwrap();
    fixture.package.expires_at += 60;
    assert_eq!(fixture.verify().unwrap_err(), PackageError::PackageRevoked);
}

#[test]
fn signature_decoding_rejects_malformed_extras_before_any_valid_threshold() {
    let fixture = Fixture::new();
    let mut pad_bits = general_purpose::STANDARD.encode([0_u8; 64]);
    pad_bits.replace_range(pad_bits.len() - 4.., "AB==");
    for malformed in [
        "%".to_owned(),
        "=AAA".to_owned(),
        pad_bits,
        general_purpose::STANDARD.encode([0_u8; 63]),
        general_purpose::STANDARD.encode([0_u8; 65]),
    ] {
        let mut envelope = fixture.golden["packageEnvelope"].clone();
        envelope["signatures"].as_array_mut().unwrap().push(json!({
            "keyid": "untrusted-extra", "sig": malformed,
        }));
        assert_eq!(
            verify_package(
                &fixture.root,
                &serde_json::to_vec(&envelope).unwrap(),
                &fixture.manifest,
                &fixture.archive,
                &fixture.policy,
            )
            .unwrap_err(),
            PackageError::EnvelopeInvalid
        );
    }
    // Well-formed cryptographically invalid extras carry no authority and do
    // not prevent a valid threshold. A malformed encoding is a decode failure.
    let mut envelope = fixture.golden["packageEnvelope"].clone();
    envelope["signatures"].as_array_mut().unwrap().push(json!({
        "sig": general_purpose::STANDARD.encode([0_u8; 64]),
    }));
    verify_package(
        &fixture.root,
        &serde_json::to_vec(&envelope).unwrap(),
        &fixture.manifest,
        &fixture.archive,
        &fixture.policy,
    )
    .unwrap();
}

#[test]
fn scalar_malleability_low_order_r_and_wrong_key_lengths_fail_strict_verification() {
    let fixture = Fixture::new();
    // RFC 8032's little-endian subgroup order L. S must be strictly below L.
    let order: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde,
        0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10,
    ];
    let original = decode_base64(
        fixture.golden["packageEnvelope"]["signatures"][0]["sig"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for defect in [
        "s-equals-l",
        "s-plus-l",
        "low-order-r-zero",
        "low-order-r-identity",
    ] {
        let mut bytes = original.clone();
        match defect {
            "s-equals-l" => bytes[32..].copy_from_slice(&order),
            "s-plus-l" => {
                let mut carry = 0_u16;
                for (index, byte) in order.iter().enumerate() {
                    let sum = bytes[32 + index] as u16 + *byte as u16 + carry;
                    bytes[32 + index] = sum as u8;
                    carry = sum >> 8;
                }
                assert_eq!(carry, 0);
            }
            "low-order-r-zero" => bytes[..32].fill(0),
            "low-order-r-identity" => {
                bytes[..32].fill(0);
                bytes[0] = 1;
            }
            _ => unreachable!(),
        }
        let mut envelope = fixture.golden["packageEnvelope"].clone();
        envelope["signatures"][0]["sig"] = json!(general_purpose::STANDARD.encode(bytes));
        assert_eq!(
            verify_package(
                &fixture.root,
                &serde_json::to_vec(&envelope).unwrap(),
                &fixture.manifest,
                &fixture.archive,
                &fixture.policy
            )
            .unwrap_err(),
            PackageError::SignatureInvalid,
            "{defect}"
        );
    }
    for size in [31, 33] {
        let bytes = vec![1_u8; size];
        let key = PublicKey {
            keyid: hash(&bytes),
            public_key: general_purpose::STANDARD.encode(bytes),
        };
        assert_eq!(public_key(&key).unwrap_err(), PackageError::TrustInvalid);
    }
}

#[test]
fn root_unknown_missing_and_duplicate_policy_fields_are_rejected() {
    let fixture = Fixture::new();
    let pins = BootstrapPins {
        keys: vec![public(1), public(2)],
        threshold: 2,
    };
    for defect in [
        "missing-previous",
        "unknown",
        "duplicate-publisher",
        "duplicate-scope",
        "duplicate-target",
        "provenance",
        "unsafe-version",
        "key-budget",
        "publisher-budget",
        "scope-budget",
    ] {
        let mut root = serde_json::to_value(fixture.root.payload()).unwrap();
        let expected = match defect {
            "missing-previous" => {
                root.as_object_mut().unwrap().remove("previousRootSha256");
                PackageError::PayloadInvalid
            }
            "unknown" => {
                root["trustAll"] = json!(true);
                PackageError::PayloadInvalid
            }
            "duplicate-publisher" => {
                root["publishers"] = json!(vec![root["publishers"][0].clone(); 2]);
                PackageError::TrustInvalid
            }
            "duplicate-scope" => {
                root["publishers"][0]["packages"] =
                    json!(vec![root["publishers"][0]["packages"][0].clone(); 2]);
                PackageError::TrustInvalid
            }
            "duplicate-target" => {
                root["publishers"][0]["packages"][0]["targets"] =
                    json!(vec![
                        root["publishers"][0]["packages"][0]["targets"][0]
                            .clone();
                        2
                    ]);
                PackageError::TrustInvalid
            }
            "provenance" => {
                root["publishers"][0]["provenance"]["subject"] = json!("Unscoped Author");
                PackageError::TrustInvalid
            }
            "unsafe-version" => {
                root["version"] = json!(MAX_SAFE_INTEGER + 1);
                PackageError::TrustInvalid
            }
            "key-budget" => {
                root["rootKeys"] = json!(vec![root["rootKeys"][0].clone(); 17]);
                PackageError::TrustInvalid
            }
            "publisher-budget" => {
                root["publishers"] = json!(vec![root["publishers"][0].clone(); 129]);
                PackageError::TrustInvalid
            }
            "scope-budget" => {
                root["publishers"][0]["packages"] =
                    json!(vec![root["publishers"][0]["packages"][0].clone(); 129]);
                PackageError::TrustInvalid
            }
            _ => unreachable!(),
        };
        assert_eq!(
            bootstrap_root(
                &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]),
                &pins,
                fixture.policy.clock
            )
            .unwrap_err(),
            expected,
            "{defect}"
        );
    }
}
