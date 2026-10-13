use super::*;
use crate::{
    data::DataStore,
    dependencies::{DependencyError, DependencyIdentity},
    packages::{
        dependencies::{
            plan_signed_plugin, SignedArchiveLock, SignedDependencyError, SignedDependencyRequest,
            SIGNED_ARCHIVE_LOCK_PROFILE,
        },
        manifest::{validate_plugin_manifest, CompatiblePluginArchive, ManifestCompatibility},
        trust::{DescriptorVerification, PackageTrustStore},
    },
};
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

struct World {
    fixture: Fixture,
    trust: PackageTrustStore,
    store: DataStore,
    compatibility: ManifestCompatibility,
}
impl World {
    fn new() -> Self {
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
                targets: vec![fixture.package.target.clone(), linux()],
            })
            .collect();
        let envelope = signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]);
        let mut store = DataStore::memory().unwrap();
        fixture.root = trust
            .bootstrap(&mut store, &envelope, fixture.policy.clock.now)
            .unwrap();
        let compatibility = ManifestCompatibility {
            host_version: "0.1.0".into(),
            sdk_version: "0.0.0".into(),
            target: fixture.package.target.clone(),
        };
        Self {
            fixture,
            trust,
            store,
            compatibility,
        }
    }
    fn now(&self) -> u64 {
        self.fixture.policy.clock.now
    }
    fn admit(&mut self, value: Value, sequence: u64) -> CompatiblePluginArchive {
        let (payload, manifest, archive) = bytes(&self.fixture, value, sequence);
        let now = self.now();
        let descriptor = self
            .trust
            .verify_descriptor(
                &mut self.store,
                DescriptorVerification {
                    envelope: &signed(PACKAGE_PAYLOAD_TYPE, &payload, &[3]),
                    manifest: &manifest,
                    archive: &archive,
                    target: payload.target.clone(),
                    now,
                },
            )
            .unwrap();
        let compatibility = ManifestCompatibility {
            host_version: self.compatibility.host_version.clone(),
            sdk_version: self.compatibility.sdk_version.clone(),
            target: payload.target.clone(),
        };
        validate_plugin_manifest(
            archive::validate_archive(&descriptor, &manifest, &archive).unwrap(),
            &compatibility,
        )
        .unwrap()
    }
    fn plan(
        &mut self,
        candidate: &CompatiblePluginArchive,
        providers: &[&CompatiblePluginArchive],
    ) -> Result<crate::packages::dependencies::VerifiedPluginDependencyPlan, SignedDependencyError>
    {
        let now = self.now();
        plan_signed_plugin(
            &self.trust,
            &mut self.store,
            SignedDependencyRequest {
                candidate,
                providers,
                compatibility: &self.compatibility,
                now,
            },
        )
    }
    fn rotate(&mut self, revoked: Option<&str>) {
        let mut root = self.fixture.root.payload().clone();
        root.version += 1;
        root.previous_root_sha256 = Some(self.fixture.root.payload_digest().into());
        if let Some(digest) = revoked {
            root.revoked_package_digests.push(digest.into());
        }
        let now = self.now();
        self.fixture.root = self
            .trust
            .rotate(
                &mut self.store,
                &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]),
                now,
            )
            .unwrap();
    }
}
fn linux() -> PackageTarget {
    PackageTarget {
        platform: PackagePlatform::Linux,
        arch: PackageArch::X64,
    }
}
fn identity(id: &str) -> DependencyIdentity {
    DependencyIdentity {
        publisher: "fixture-publisher".into(),
        id: id.into(),
    }
}
fn dependency(id: &str) -> Value {
    json!({"publisher":"fixture-publisher", "id":id, "version":"^1.0.0",
        "service":"text-transform", "interfaceVersion":"^1.0.0"})
}
pub(super) fn manifest(id: &str, dependencies: Vec<Value>) -> Value {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../sdk/test/fixtures/manifest-admission-v1.json"
    ))
    .unwrap();
    value = value["base"].clone();
    value["publisher"] = json!("fixture-publisher");
    value["id"] = json!(id);
    value["version"] = json!("1.0.0");
    value["dependencies"]["services"] = json!(dependencies);
    value["entries"]["services"] = json!("dist/services.js");
    value["services"] = json!([{"id":"text-transform", "version":"1.2.0",
        "operations":[value["commands"][0].clone()]}]);
    value
}
// Real signed stored ZIPs with full SDK-compatible manifests. Entries are inert
// canaries; planning must never import, stage or execute either file.
pub(super) fn bytes(
    fixture: &Fixture,
    mut value: Value,
    sequence: u64,
) -> (PackagePayload, Vec<u8>, Vec<u8>) {
    let mut payload = fixture.package.clone();
    payload.id = value["id"].as_str().unwrap().into();
    payload.version = value["version"].as_str().unwrap().into();
    payload.release_sequence = sequence;
    payload.target = serde_json::from_value(value["targets"][0].clone()).unwrap();
    let source = b"throw new Error('SIGNED_DEPENDENCY_FIXTURE_MUST_NOT_EXECUTE')";
    payload.files = ["dist/commands.js", "dist/services.js"]
        .iter()
        .map(|path| PackageFile {
            path: (*path).into(),
            sha256: hash(source),
            size: source.len() as u64,
        })
        .collect();
    value["files"] = serde_json::to_value(&payload.files).unwrap();
    let manifest = serde_json::to_vec_pretty(&value).unwrap();
    payload.manifest = BlobDescriptor {
        sha256: hash(&manifest),
        size: manifest.len() as u64,
    };
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        ("manifest.json", manifest.as_slice()),
        ("dist/commands.js", source.as_slice()),
        ("dist/services.js", source.as_slice()),
    ] {
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(data).unwrap();
    }
    let archive = zip.finish().unwrap().into_inner();
    payload.archive.sha256 = hash(&archive);
    payload.archive.size = archive.len() as u64;
    (payload, manifest, archive)
}

#[test]
fn signed_dag_pins_zip_bytes_service_interfaces_and_deterministic_reverse_topology() {
    let mut world = World::new();
    let consumer = world.admit(manifest("consumer", vec![dependency("middle")]), 1);
    let middle = world.admit(manifest("middle", vec![dependency("provider")]), 1);
    let provider = world.admit(manifest("provider", vec![]), 1);
    let plan = world.plan(&consumer, &[&middle, &provider]).unwrap();
    let reordered = world.plan(&consumer, &[&provider, &middle]).unwrap();
    assert_eq!(plan.lock().digest, reordered.lock().digest);
    assert_eq!(plan.lock().digest, plan.lock().verified_digest().unwrap());
    assert_eq!(plan.lock().profile, SIGNED_ARCHIVE_LOCK_PROFILE);
    assert_eq!(plan.lock().roots, vec![identity("consumer")]);
    assert_eq!(
        plan.lock().topology,
        vec![
            identity("provider"),
            identity("middle"),
            identity("consumer")
        ]
    );
    assert_eq!(plan.root_digest(), world.fixture.root.payload_digest());
    assert_eq!(plan.verified_at(), world.now());
    assert_eq!(plan.lock().target.platform, "windows");
    for archive in [&consumer, &middle, &provider] {
        let descriptor = archive.archive().descriptor();
        let pin = plan
            .lock()
            .packages
            .iter()
            .find(|pin| pin.id == descriptor.payload().id)
            .unwrap();
        assert_eq!(pin.package_digest, descriptor.package_digest());
        assert_ne!(
            pin.package_digest,
            crate::catalog::digest(archive.manifest())
        );
        assert_eq!(pin.publisher, descriptor.payload().publisher);
        assert_eq!(pin.version, descriptor.payload().version);
        assert_eq!(pin.target, descriptor.payload().target);
    }
    assert_eq!(
        plan.candidate().package_digest,
        consumer.archive().descriptor().package_digest()
    );
    assert_eq!(plan.lock().services.len(), 2);
    let service = plan
        .lock()
        .services
        .iter()
        .find(|pin| pin.consumer.id == "middle")
        .unwrap();
    assert_eq!(
        service.provider.package_digest,
        provider.archive().descriptor().package_digest()
    );
    assert_eq!(service.version, "1.2.0");
    assert_eq!(plan.lock().edges.len(), 2);
    assert_eq!(
        plan.lock()
            .reverse_dependencies
            .iter()
            .find(|r| r.provider.id == "provider")
            .unwrap()
            .consumers,
        vec![identity("middle")]
    );
    assert!(plan.lock().tools.is_empty());
    for table in ["grants", "jobs", "plugin_data", "imports"] {
        let count: i64 = world
            .store
            .connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
    let metadata: i64 = world.store.connection.query_row("SELECT count(*) FROM core_metadata WHERE key != 'package-trust-state-v1' AND key != 'schema-version'", [], |row| row.get(0)).unwrap();
    assert_eq!(metadata, 0);
}

#[test]
fn signed_archive_profile_separates_g4_semantic_pins_and_rejects_missing_or_changed_profiles() {
    use crate::dependencies::{DependencyCatalog, DependencyLock, PackageCandidate};
    let mut world = World::new();
    let consumer = world.admit(manifest("consumer", vec![]), 1);
    let plan = world.plan(&consumer, &[]).unwrap();
    let catalog = DependencyCatalog::new(
        vec![PackageCandidate::from_verified_archive(&consumer).unwrap()],
        vec![],
    )
    .unwrap();
    let semantic = catalog
        .resolve(&[identity("consumer")], &plan.lock().target)
        .unwrap()
        .lock;
    assert_eq!(
        semantic.packages[0].digest,
        crate::catalog::digest(consumer.manifest())
    );
    assert_ne!(semantic.packages[0].digest, plan.candidate().package_digest);
    assert!(
        serde_json::from_value::<SignedArchiveLock>(serde_json::to_value(&semantic).unwrap())
            .is_err()
    );
    let signed = serde_json::to_value(plan.lock()).unwrap();
    assert!(serde_json::from_value::<DependencyLock>(signed.clone()).is_err());
    assert_eq!(
        serde_json::from_value::<SignedArchiveLock>(signed.clone()).unwrap(),
        *plan.lock()
    );
    for profile in [None, Some("g4-semantic-manifests-v1"), Some("")] {
        let mut changed = signed.clone();
        if let Some(profile) = profile {
            changed["profile"] = json!(profile);
        } else {
            changed.as_object_mut().unwrap().remove("profile");
        }
        assert!(serde_json::from_value::<SignedArchiveLock>(changed).is_err());
    }
    let mut domainless = signed.clone();
    domainless.as_object_mut().unwrap().remove("profile");
    domainless.as_object_mut().unwrap().remove("digest");
    assert_ne!(crate::catalog::digest(&domainless), plan.lock().digest);
    let mut changed = plan.lock().clone();
    changed.profile = "g4-semantic-manifests-v1".into();
    assert_eq!(
        changed.verified_digest(),
        Err(DependencyError::InvalidDeclaration)
    );
    let mut changed = plan.lock().clone();
    changed.packages[0].package_digest = "f".repeat(64);
    assert_ne!(changed.verified_digest().unwrap(), plan.lock().digest);
    let mut changed = plan.lock().clone();
    changed.packages[0].target = linux();
    assert_ne!(changed.verified_digest().unwrap(), plan.lock().digest);
}

#[test]
fn signed_service_selection_refuses_missing_publisher_package_or_interface_and_cycles() {
    for (change, expected) in [
        ("publisher", DependencyError::Missing),
        ("version", DependencyError::VersionConflict),
        ("interface", DependencyError::VersionConflict),
        ("cycle", DependencyError::Cycle),
        ("legacy", DependencyError::InterfaceRequired),
    ] {
        let mut world = World::new();
        let mut dep = dependency("provider");
        match change {
            "publisher" => dep["publisher"] = json!("other-publisher"),
            "version" => dep["version"] = json!("^2.0.0"),
            "interface" => dep["interfaceVersion"] = json!("^2.0.0"),
            "legacy" => {
                dep.as_object_mut().unwrap().remove("service");
                dep.as_object_mut().unwrap().remove("interfaceVersion");
            }
            _ => {}
        }
        let consumer = world.admit(manifest("consumer", vec![dep]), 1);
        let provider = world.admit(
            manifest(
                "provider",
                if change == "cycle" {
                    vec![dependency("consumer")]
                } else {
                    vec![]
                },
            ),
            1,
        );
        assert_eq!(
            world.plan(&consumer, &[&provider]).err(),
            Some(SignedDependencyError::Dependency(expected)),
            "{change}"
        );
    }
    let mut world = World::new();
    let consumer = world.admit(manifest("consumer", vec![dependency("provider")]), 1);
    assert_eq!(
        world.plan(&consumer, &[]).err(),
        Some(SignedDependencyError::Dependency(DependencyError::Missing))
    );
}

#[test]
fn duplicate_providers_and_alternate_root_versions_cannot_substitute_the_candidate() {
    let mut world = World::new();
    let consumer = world.admit(manifest("consumer", vec![dependency("provider")]), 1);
    let provider = world.admit(manifest("provider", vec![]), 1);
    assert_eq!(
        world.plan(&consumer, &[&provider, &provider]).err(),
        Some(SignedDependencyError::Dependency(
            DependencyError::DuplicateProvider
        ))
    );
    assert_eq!(
        world.plan(&consumer, &[&consumer]).err(),
        Some(SignedDependencyError::Dependency(
            DependencyError::DuplicateProvider
        ))
    );
    let mut revised = manifest("consumer", vec![]);
    revised["version"] = json!("1.1.0");
    let newer = world.admit(revised, 2);
    assert_eq!(
        world.plan(&newer, &[&consumer]).err(),
        Some(SignedDependencyError::Package(
            PackageError::ReleaseRollback
        ))
    );
    // The same identity is refused even after it has freshly passed current floors.
    assert_eq!(
        world.plan(&newer, &[&newer]).err(),
        Some(SignedDependencyError::Dependency(
            DependencyError::DuplicateProvider
        ))
    );
}

#[test]
fn actual_admitted_target_and_current_host_sdk_compatibility_override_cached_metadata() {
    let mut world = World::new();
    let mut multi = manifest("consumer", vec![]);
    multi["targets"] = json!([world.compatibility.target, linux()]);
    let consumer = world.admit(multi, 1);
    world.compatibility.target = linux();
    assert_eq!(
        world.plan(&consumer, &[]).err(),
        Some(SignedDependencyError::Package(PackageError::TargetMismatch))
    );
    world.compatibility.target = world.fixture.package.target.clone();
    world.compatibility.host_version = "1.0.0".into();
    assert_eq!(
        world.plan(&consumer, &[]).err(),
        Some(SignedDependencyError::Package(
            PackageError::ManifestIncompatible
        ))
    );
    world.compatibility.host_version = "0.1.0".into();
    world.compatibility.sdk_version = "1.0.0".into();
    assert_eq!(
        world.plan(&consumer, &[]).err(),
        Some(SignedDependencyError::Package(
            PackageError::ManifestIncompatible
        ))
    );
    world.compatibility.sdk_version = "0.0.0".into();
    let mut value = manifest("provider", vec![]);
    value["targets"] = json!([linux()]);
    let linux_provider = world.admit(value, 1);
    assert_eq!(
        world.plan(&consumer, &[&linux_provider]).err(),
        Some(SignedDependencyError::Package(PackageError::TargetMismatch))
    );
}

#[test]
fn declared_tools_never_create_an_unverified_tool_inventory() {
    for selected in [false, true] {
        let mut world = World::new();
        let mut value = manifest("consumer", vec![]);
        let mut tool =
            json!({"publisher":"fixture-publisher", "id":"fixture-tool", "version":"^1.0.0"});
        if selected {
            tool["target"] = json!(world.compatibility.target);
            tool["buildFlavor"] = json!("standard");
            tool["digest"] = json!("a".repeat(64));
        }
        value["dependencies"]["tools"] = json!([tool]);
        let consumer = world.admit(value, 1);
        assert_eq!(
            world.plan(&consumer, &[]).err(),
            Some(SignedDependencyError::Dependency(if selected {
                DependencyError::Missing
            } else {
                DependencyError::InvalidDeclaration
            }))
        );
    }
}

#[test]
fn pure_verifier_receipts_and_changed_bytes_without_durable_identity_are_refused() {
    let mut world = World::new();
    let (payload, manifest, archive) = bytes(&world.fixture, manifest("consumer", vec![]), 1);
    let descriptor = verify_package(
        &world.fixture.root,
        &signed(PACKAGE_PAYLOAD_TYPE, &payload, &[3]),
        &manifest,
        &archive,
        &world.fixture.policy,
    )
    .unwrap();
    let consumer = validate_plugin_manifest(
        archive::validate_archive(&descriptor, &manifest, &archive).unwrap(),
        &world.compatibility,
    )
    .unwrap();
    assert_eq!(
        world.plan(&consumer, &[]).err(),
        Some(SignedDependencyError::Package(PackageError::TrustInvalid))
    );
    let observed = world.admit(consumer.manifest().clone(), 1);
    assert!(world.plan(&observed, &[]).is_ok());
    // Valid signature and compatible metadata still cannot invent an observation.
    let mut value = observed.manifest().clone();
    value["name"] = json!("Changed package bytes");
    let (payload, manifest, archive) = bytes(&world.fixture, value, 1);
    let descriptor = verify_package(
        &world.fixture.root,
        &signed(PACKAGE_PAYLOAD_TYPE, &payload, &[3]),
        &manifest,
        &archive,
        &world.fixture.policy,
    )
    .unwrap();
    let changed = validate_plugin_manifest(
        archive::validate_archive(&descriptor, &manifest, &archive).unwrap(),
        &world.compatibility,
    )
    .unwrap();
    assert_eq!(
        world.plan(&changed, &[]).err(),
        Some(SignedDependencyError::Package(PackageError::TrustInvalid))
    );
}

#[test]
fn cached_archives_require_current_root_and_revocations_override_prior_acceptance() {
    for revoked in [false, true] {
        let mut world = World::new();
        let consumer = world.admit(manifest("consumer", vec![]), 1);
        world.rotate(revoked.then(|| consumer.archive().descriptor().package_digest()));
        assert_eq!(
            world.plan(&consumer, &[]).err(),
            Some(SignedDependencyError::Package(if revoked {
                PackageError::PackageRevoked
            } else {
                PackageError::TrustInvalid
            }))
        );
        if !revoked {
            let refreshed = world.admit(consumer.manifest().clone(), 1);
            let plan = world.plan(&refreshed, &[]).unwrap();
            assert_eq!(plan.root_digest(), world.fixture.root.payload_digest());
        }
    }
}

#[test]
fn current_expiry_clock_and_release_floors_refuse_stale_candidate_pools() {
    let mut world = World::new();
    let consumer = world.admit(manifest("consumer", vec![dependency("provider")]), 1);
    let old = world.admit(manifest("provider", vec![]), 1);
    let mut value = manifest("provider", vec![]);
    value["version"] = json!("1.1.0");
    let current = world.admit(value, 2);
    assert_eq!(
        world.plan(&consumer, &[&old, &current]).err(),
        Some(SignedDependencyError::Package(
            PackageError::ReleaseRollback
        ))
    );
    assert!(world.plan(&consumer, &[&current]).is_ok());
    world.fixture.policy.clock.now -= 1;
    assert_eq!(
        world.plan(&consumer, &[&current]).err(),
        Some(SignedDependencyError::Package(PackageError::ClockRollback))
    );
    world.fixture.policy.clock.now = consumer.archive().descriptor().payload().expires_at;
    assert_eq!(
        world.plan(&consumer, &[&current]).err(),
        Some(SignedDependencyError::Package(PackageError::TrustExpired))
    );
    // Rejected expiration still commits the observed T0 time floor.
    assert_eq!(
        world
            .trust
            .snapshot(&mut world.store)
            .unwrap()
            .observed_time,
        world.now()
    );
    let mut world = World::new();
    let consumer = world.admit(manifest("consumer", vec![]), 1);
    world.fixture.policy.clock.now = world.fixture.root.payload().expires_at;
    assert_eq!(
        world.plan(&consumer, &[]).err(),
        Some(SignedDependencyError::Package(PackageError::TrustExpired))
    );
}

#[test]
fn missing_corrupt_recovery_or_write_failed_durable_trust_refuses_a_plan() {
    for mode in ["missing", "corrupt", "recovery", "write"] {
        let mut world = World::new();
        let consumer = world.admit(manifest("consumer", vec![]), 1);
        let (sql, expected) = match mode {
            "missing" => ("DELETE FROM core_metadata WHERE key='package-trust-state-v1'", PackageError::TrustStateRequired),
            "corrupt" => ("UPDATE core_metadata SET value='{}' WHERE key='package-trust-state-v1'", PackageError::TrustStateInvalid),
            "recovery" => ("INSERT INTO core_metadata VALUES('package-trust-recovery-required','1')", PackageError::TrustRecoveryRequired),
            _ => ("CREATE TRIGGER trust_write BEFORE UPDATE ON core_metadata BEGIN SELECT RAISE(ABORT,'fixture-write-failed'); END", PackageError::TrustStorageFailed),
        };
        world.store.connection.execute_batch(sql).unwrap();
        world.fixture.policy.clock.now += 1;
        assert_eq!(
            world.plan(&consumer, &[]).err(),
            Some(SignedDependencyError::Package(expected)),
            "{mode}"
        );
    }
}

#[test]
fn candidate_count_and_total_signed_manifest_budget_refuse_before_trust_io() {
    let mut world = World::new();
    let consumer = world.admit(manifest("consumer", vec![]), 1);
    world
        .store
        .connection
        .execute_batch("DELETE FROM core_metadata WHERE key='package-trust-state-v1'")
        .unwrap();
    assert_eq!(
        world.plan(&consumer, &vec![&consumer; 512]).err(),
        Some(SignedDependencyError::Dependency(
            DependencyError::BudgetExceeded
        ))
    );
    let mut world = World::new();
    let mut value = manifest("consumer", vec![]);
    value["commands"][0]["inputSchema"]["properties"]["text"]["enum"] = json!(["a".repeat(40_000)]);
    let consumer = world.admit(value, 1);
    assert_eq!(
        world.plan(&consumer, &vec![&consumer; 511]).err(),
        Some(SignedDependencyError::Dependency(
            DependencyError::BudgetExceeded
        ))
    );
}
