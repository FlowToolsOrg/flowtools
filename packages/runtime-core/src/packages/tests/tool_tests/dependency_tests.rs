//! Real signed plugin/tool ZIP graphs, with no imports, installation or grants.
use super::super::dependency_tests::{bytes, manifest};
use super::*;
use crate::dependencies::DependencyError;
use crate::packages::{
    dependencies::{
        plan_signed_plugin,
        tools::{
            plan_signed_tools, SignedToolArchiveLock, SignedToolDependencyRequest,
            VerifiedToolDependencyPlan, SIGNED_TOOL_LOCK_PROFILE,
        },
        SignedArchiveLock, SignedDependencyError, SignedDependencyRequest,
    },
    manifest::{validate_plugin_manifest, CompatiblePluginArchive, ManifestCompatibility},
    trust::DescriptorVerification,
};

struct World {
    fixture: ToolFixture,
    trust: PackageTrustStore,
    store: DataStore,
    compatibility: ManifestCompatibility,
}
impl World {
    fn new() -> Self {
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
        let (store, trust) = fixture.setup();
        let compatibility = ManifestCompatibility {
            host_version: "0.1.0".into(),
            sdk_version: "0.0.0".into(),
            target: fixture.base.package.target.clone(),
        };
        Self {
            fixture,
            trust,
            store,
            compatibility,
        }
    }
    fn tool(&mut self) -> InertToolArchive {
        self.fixture.admit(&mut self.store, &self.trust).unwrap()
    }
    fn plugin(&mut self, value: Value) -> CompatiblePluginArchive {
        let (mut payload, manifest, archive) = bytes(&self.fixture.base, value, 1);
        payload.kind = PackageKind::Plugin;
        let receipt = self
            .trust
            .verify_descriptor(
                &mut self.store,
                DescriptorVerification {
                    envelope: &signed(PACKAGE_PAYLOAD_TYPE, &payload, &[3]),
                    manifest: &manifest,
                    archive: &archive,
                    target: payload.target.clone(),
                    now: self.fixture.now(),
                },
            )
            .unwrap();
        validate_plugin_manifest(
            archive::validate_archive(&receipt, &manifest, &archive).unwrap(),
            &self.compatibility,
        )
        .unwrap()
    }
    fn plan(
        &mut self,
        candidate: &CompatiblePluginArchive,
        providers: &[&CompatiblePluginArchive],
        tools: &[&InertToolArchive],
    ) -> Result<VerifiedToolDependencyPlan, SignedDependencyError> {
        plan_signed_tools(
            &self.trust,
            &mut self.store,
            SignedToolDependencyRequest {
                candidate,
                providers,
                tools,
                compatibility: &self.compatibility,
            },
            || self.fixture.now(),
        )
    }
    fn manifest(&self, id: &str, services: Vec<Value>, tool: bool) -> Value {
        let mut value = manifest(id, services);
        if tool {
            value["dependencies"]["tools"] = json!([self.declaration()]);
        }
        value
    }
    fn declaration(&self) -> Value {
        json!({"publisher":"fixture-publisher","id":"ffmpeg","version":"*","target":self.fixture.base.package.target,"buildFlavor":"fixture-static","digest":self.fixture.base.package.archive.sha256})
    }
    fn rotate(&mut self, revoked: Option<&str>) {
        let old = self.trust.snapshot(&mut self.store).unwrap();
        let mut root: RootPayload = serde_json::from_slice(
            &decode_envelope(
                &serde_json::to_vec(&self.fixture.base.golden["rootEnvelope"]).unwrap(),
                ROOT_PAYLOAD_TYPE,
            )
            .unwrap()
            .payload,
        )
        .unwrap();
        root.version += 1;
        root.previous_root_sha256 = Some(old.root_digest);
        if let Some(hash) = revoked {
            root.revoked_package_digests.push(hash.into());
        }
        let envelope = signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]);
        self.trust
            .rotate(&mut self.store, &envelope, self.fixture.now())
            .unwrap();
        self.fixture.base.golden["rootEnvelope"] = serde_json::from_slice(&envelope).unwrap();
    }
}
fn service(id: &str) -> Value {
    json!({"publisher":"fixture-publisher","id":id,"version":"^1.0.0","service":"text-transform","interfaceVersion":"^1.0.0"})
}

#[test]
fn signed_service_dag_and_two_consumers_pin_one_exact_tool_zip_deterministically() {
    let mut world = World::new();
    let tool = world.tool();
    let consumer = world.plugin(world.manifest("consumer", vec![service("middle")], true));
    let middle = world.plugin(world.manifest("middle", vec![service("provider")], true));
    let provider = world.plugin(world.manifest("provider", vec![], false));
    let first = world
        .plan(&consumer, &[&provider, &middle], &[&tool])
        .unwrap();
    let second = world
        .plan(&consumer, &[&middle, &provider], &[&tool])
        .unwrap();
    assert_eq!(first.lock(), second.lock());
    assert_eq!(first.lock().profile, SIGNED_TOOL_LOCK_PROFILE);
    assert_eq!(first.lock().tools.len(), 2);
    assert_eq!(first.lock().services.len(), 2);
    assert_eq!(first.lock().packages.len(), 3);
    assert_eq!(first.lock().verified_digest().unwrap(), first.lock().digest);
    assert_eq!(
        first.candidate().package_digest,
        consumer.archive().descriptor().package_digest()
    );
    for pin in &first.lock().tools {
        assert_eq!(
            pin.package_digest,
            tool.archive().descriptor().package_digest()
        );
        assert_ne!(
            pin.package_digest,
            crate::catalog::digest(&world.fixture.manifest)
        );
        assert_eq!(pin.target, world.fixture.base.package.target);
        assert_eq!(pin.build_flavor, "fixture-static");
    }
    assert_eq!(
        first
            .lock()
            .tools
            .iter()
            .map(|t| t.consumer.id.as_str())
            .collect::<Vec<_>>(),
        vec!["consumer", "middle"]
    );
    assert_eq!(first.verified_at(), world.fixture.now());
    assert_eq!(
        first.root_digest(),
        world.trust.snapshot(&mut world.store).unwrap().root_digest
    );
    assert_no_effects(&world.store);
}
#[test]
fn tool_selection_refuses_missing_version_target_flavor_hash_and_legacy_declarations() {
    for (change, expected) in [
        ("missing", DependencyError::Missing),
        ("publisher", DependencyError::Missing),
        ("version", DependencyError::VersionConflict),
        ("target", DependencyError::TargetMismatch),
        ("flavor", DependencyError::ArtifactConflict),
        ("hash", DependencyError::ArtifactConflict),
        ("metadata-hash", DependencyError::ArtifactConflict),
        ("legacy", DependencyError::InvalidDeclaration),
    ] {
        let mut world = World::new();
        let tool = world.tool();
        let mut value = world.manifest("consumer", vec![], true);
        let dep = &mut value["dependencies"]["tools"][0];
        match change {
            "publisher" => dep["publisher"] = json!("other-publisher"),
            "version" => dep["version"] = json!("^99.0.0"),
            "target" => dep["target"] = json!({"platform":"linux","arch":"x64"}),
            "flavor" => dep["buildFlavor"] = json!("unreviewed"),
            "hash" => dep["digest"] = json!("f".repeat(64)),
            "metadata-hash" => {
                dep["digest"] = json!(crate::catalog::digest(&world.fixture.manifest))
            }
            "legacy" => {
                for key in ["target", "buildFlavor", "digest"] {
                    dep.as_object_mut().unwrap().remove(key);
                }
            }
            _ => {}
        }
        let consumer = world.plugin(value);
        let tools = if change == "missing" {
            vec![]
        } else {
            vec![&tool]
        };
        assert_eq!(
            world.plan(&consumer, &[], &tools).err(),
            Some(SignedDependencyError::Dependency(expected)),
            "{change}"
        );
        assert_no_effects(&world.store);
    }
}
#[test]
fn root_substitution_and_duplicate_tool_candidates_are_refused() {
    let mut world = World::new();
    let tool = world.tool();
    let candidate = world.plugin(world.manifest("consumer", vec![], true));
    assert_eq!(
        world.plan(&candidate, &[&candidate], &[&tool]).err(),
        Some(SignedDependencyError::Dependency(
            DependencyError::DuplicateProvider
        ))
    );
    assert_eq!(
        world.plan(&candidate, &[], &[&tool, &tool]).err(),
        Some(SignedDependencyError::Dependency(
            DependencyError::DuplicateProvider
        ))
    );
    assert_no_effects(&world.store);
}
#[test]
fn lock_profiles_reject_cross_deserialization_and_never_change_the_g4_or_legacy_contract() {
    let mut world = World::new();
    let tool = world.tool();
    let candidate = world.plugin(world.manifest("consumer", vec![], true));
    let plan = world.plan(&candidate, &[], &[&tool]).unwrap();
    let value = serde_json::to_value(plan.lock()).unwrap();
    assert!(serde_json::from_value::<SignedArchiveLock>(value.clone()).is_err());
    assert!(serde_json::from_value::<crate::dependencies::DependencyLock>(value.clone()).is_err());
    let roundtrip: SignedToolArchiveLock = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(roundtrip, *plan.lock());
    let mut legacy_profile = value.clone();
    legacy_profile["profile"] =
        json!(super::super::super::dependencies::SIGNED_ARCHIVE_LOCK_PROFILE);
    assert!(serde_json::from_value::<SignedToolArchiveLock>(legacy_profile).is_err());
    let now = world.fixture.now();
    assert_eq!(
        plan_signed_plugin(
            &world.trust,
            &mut world.store,
            SignedDependencyRequest {
                candidate: &candidate,
                providers: &[],
                compatibility: &world.compatibility,
                now
            }
        )
        .err(),
        Some(SignedDependencyError::Dependency(DependencyError::Missing))
    );
    let mut changed = plan.lock().clone();
    changed.tools[0].package_digest = "f".repeat(64);
    assert_ne!(changed.verified_digest().unwrap(), plan.lock().digest);
    assert_no_effects(&world.store);
}
#[test]
fn fresh_final_clock_refuses_expired_and_rolled_back_plans_after_resolution() {
    for expired in [false, true] {
        let mut world = World::new();
        let tool = world.tool();
        let candidate = world.plugin(world.manifest("consumer", vec![], true));
        let mut calls = 0;
        let result = plan_signed_tools(
            &world.trust,
            &mut world.store,
            SignedToolDependencyRequest {
                candidate: &candidate,
                providers: &[],
                tools: &[&tool],
                compatibility: &world.compatibility,
            },
            || {
                calls += 1;
                if calls == 1 {
                    world.fixture.now()
                } else if expired {
                    world.fixture.base.package.expires_at
                } else {
                    world.fixture.now() - 1
                }
            },
        );
        assert_eq!(calls, 2);
        assert_eq!(
            result.err(),
            Some(SignedDependencyError::Package(if expired {
                PackageError::TrustExpired
            } else {
                PackageError::ClockRollback
            }))
        );
        assert_no_effects(&world.store);
    }
}
#[test]
fn unused_admitted_tool_receipts_still_require_current_root_revocation_and_release_floors() {
    for revoked in [false, true] {
        let mut world = World::new();
        let tool = world.tool();
        let hash = tool.archive().descriptor().package_digest().to_owned();
        world.rotate(revoked.then_some(hash.as_str()));
        let candidate = world.plugin(world.manifest("consumer", vec![], false));
        assert_eq!(
            world.plan(&candidate, &[], &[&tool]).err(),
            Some(SignedDependencyError::Package(if revoked {
                PackageError::PackageRevoked
            } else {
                PackageError::TrustInvalid
            }))
        );
        assert_no_effects(&world.store);
    }
    let mut world = World::new();
    let old = world.tool();
    world.fixture.base.package.version = "9.0.0".into();
    world.fixture.base.package.release_sequence += 1;
    world.fixture.manifest["version"] = json!("9.0.0");
    world.fixture.rebuild();
    let new = world.tool();
    let candidate = world.plugin(world.manifest("consumer", vec![], false));
    assert_eq!(
        world.plan(&candidate, &[], &[&old, &new]).err(),
        Some(SignedDependencyError::Package(
            PackageError::ReleaseRollback
        ))
    );
    assert_no_effects(&world.store);
}
#[test]
fn combined_archive_budget_refuses_before_trust_observation_and_new_profile_allows_empty_tools() {
    let mut world = World::new();
    let tool = world.tool();
    let candidate = world.plugin(world.manifest("consumer", vec![], false));
    let before = world.trust.snapshot(&mut world.store).unwrap();
    let providers = vec![&candidate; 511];
    assert_eq!(
        world.plan(&candidate, &providers, &[&tool]).err(),
        Some(SignedDependencyError::Dependency(
            DependencyError::BudgetExceeded
        ))
    );
    let after = world.trust.snapshot(&mut world.store).unwrap();
    assert_eq!(before.observed_time, after.observed_time);
    assert_eq!(before.known_versions.len(), after.known_versions.len());
    let empty = world.plan(&candidate, &[], &[]).unwrap();
    assert!(empty.lock().tools.is_empty());
    assert_eq!(empty.lock().packages.len(), 1);
    assert_no_effects(&world.store);
}
