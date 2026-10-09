use super::*;
use node_semver::Version;
use serde_json::{json, Value};

fn identity(id: &str) -> DependencyIdentity {
    DependencyIdentity {
        publisher: "flowtools".into(),
        id: id.into(),
    }
}
fn target(platform: &str) -> DependencyTarget {
    DependencyTarget {
        platform: platform.into(),
        arch: "x64".into(),
    }
}
fn service(id: &str, version: &str) -> Value {
    json!({"id":id,"version":version,"operations":[{"id":"run","headless":true,"interaction":"none"}]})
}
// Disposable metadata only. These fixtures never call a loader, entry or capability.
// Actual fixed-inventory admission still uses the SDK-generated complete manifests.
fn package(id: &str, version: &str, dependencies: Vec<Value>) -> Value {
    json!({
        "publisher":"flowtools","id":id,"version":version,
        "targets":[{"platform":"windows","arch":"x64"},{"platform":"linux","arch":"x64"}],
        "entries":{"services":"dist/services.js"},
        "files":[{"path":"dist/services.js","size":0,"sha256":"1".repeat(64)}],
        "services":[service("text-transform","1.2.0")],
        "dependencies":{"services":dependencies,"tools":[]}
    })
}
fn dependency(id: &str, package_range: &str, interface_range: &str) -> Value {
    json!({"publisher":"flowtools","id":id,"version":package_range,
        "service":"text-transform","interfaceVersion":interface_range})
}
fn catalog(packages: Vec<Value>) -> DependencyCatalog {
    DependencyCatalog::from_manifests(packages, vec![]).unwrap()
}
fn resolve(packages: Vec<Value>, roots: &[&str]) -> Result<DependencyPlan, DependencyError> {
    catalog(packages).resolve(
        &roots.iter().map(|id| identity(id)).collect::<Vec<_>>(),
        &target("windows"),
    )
}
fn tool(version: &str, platform: &str, digest: &str) -> ToolCandidate {
    ToolCandidate {
        publisher: "flowtools".into(),
        id: "fixture-tool".into(),
        version: version.into(),
        target: target(platform),
        build_flavor: "standard".into(),
        digest: digest.into(),
    }
}
fn tool_dependency(version: &str, platform: &str, digest: &str) -> Value {
    json!({"publisher":"flowtools","id":"fixture-tool","version":version,
        "target":target(platform),"buildFlavor":"standard","digest":digest})
}

#[test]
fn fixed_embedded_inventory_plans_only_metadata_without_execution() {
    let builtin = crate::catalog::BuiltinCatalog::embedded();
    let manifests: Vec<_> = builtin
        .list()
        .map(|(_, manifest)| manifest.clone())
        .collect();
    let roots: Vec<_> = manifests
        .iter()
        .map(|manifest| DependencyIdentity {
            publisher: manifest["publisher"].as_str().unwrap().into(),
            id: manifest["id"].as_str().unwrap().into(),
        })
        .collect();
    let plan = DependencyCatalog::from_manifests(manifests.clone(), vec![])
        .unwrap()
        .resolve(&roots, &target("windows"))
        .unwrap();
    assert_eq!(plan.mode, "plan-only");
    assert_eq!(plan.format_version, 1);
    assert_eq!(plan.lock.format_version, 1);
    assert_eq!(plan.lock.packages.len(), 12);
    assert_eq!(plan.lock.roots.len(), 12);
    assert!(
        plan.lock.services.is_empty() && plan.lock.tools.is_empty() && plan.lock.edges.is_empty()
    );
    assert_eq!(plan.lock.digest, plan.lock.verified_digest().unwrap());
    for pin in plan.lock.packages {
        let manifest = manifests
            .iter()
            .find(|manifest| manifest["id"] == pin.id)
            .unwrap();
        assert_eq!(pin.digest, crate::catalog::digest(manifest));
        assert_eq!(pin.version, manifest["version"]);
    }
}

#[test]
fn dag_locks_exact_identity_interface_artifact_topology_and_reverse_dependencies() {
    let packages = vec![
        package(
            "consumer",
            "1.0.0",
            vec![dependency("middle", "^2.0.0", "^1.0.0")],
        ),
        package(
            "middle",
            "2.1.0",
            vec![dependency("provider", "~3.0.0", "^1.2.0")],
        ),
        package("provider", "3.0.4", vec![]),
    ];
    let plan = resolve(packages.clone(), &["consumer"]).unwrap();
    assert_eq!(
        plan.lock.topology,
        vec![
            identity("provider"),
            identity("middle"),
            identity("consumer")
        ]
    );
    assert_eq!(plan.lock.roots, vec![identity("consumer")]);
    assert_eq!(plan.lock.edges.len(), 2);
    let pin = plan
        .lock
        .services
        .iter()
        .find(|pin| pin.consumer.id == "middle")
        .unwrap();
    assert_eq!(pin.provider.id, "provider");
    assert_eq!(pin.provider.publisher, "flowtools");
    assert_eq!(pin.provider.version, "3.0.4");
    assert_eq!(pin.provider.digest, crate::catalog::digest(&packages[2]));
    assert_eq!(pin.version, "1.2.0");
    let reverse = plan
        .lock
        .reverse_dependencies
        .iter()
        .find(|item| item.provider.id == "provider")
        .unwrap();
    assert_eq!(reverse.consumers, vec![identity("middle")]);
    assert!(plan
        .lock
        .reverse_dependencies
        .iter()
        .find(|item| item.provider.id == "consumer")
        .unwrap()
        .consumers
        .is_empty());
}

#[test]
fn complete_search_backtracks_an_earlier_provider_when_later_constraints_arrive() {
    // The provider sorts before roots and initially chooses 2.0.0. The second
    // consumer later imposes 1.x, requiring a return to the earlier decision.
    let plan = resolve(
        vec![
            package("a-provider", "2.0.0", vec![]),
            package("a-provider", "1.4.0", vec![]),
            package(
                "consumer-one",
                "1.0.0",
                vec![dependency("a-provider", "*", "^1.0.0")],
            ),
            package(
                "consumer-two",
                "1.0.0",
                vec![dependency("a-provider", "1.x", "^1.2.0")],
            ),
        ],
        &["consumer-one", "consumer-two"],
    )
    .unwrap();
    assert_eq!(
        plan.lock
            .packages
            .iter()
            .find(|pin| pin.id == "a-provider")
            .unwrap()
            .version,
        "1.4.0"
    );
    assert_eq!(plan.lock.services.len(), 2);
}

#[test]
fn transitive_unsatisfied_choice_backtracks_to_lower_version() {
    let plan = resolve(
        vec![
            package(
                "consumer",
                "1.0.0",
                vec![dependency("provider", "^1.0.0", "^1.0.0")],
            ),
            package("provider", "1.9.0", vec![dependency("missing", "*", "*")]),
            package("provider", "1.4.0", vec![]),
        ],
        &["consumer"],
    )
    .unwrap();
    assert_eq!(
        plan.lock
            .packages
            .iter()
            .find(|pin| pin.id == "provider")
            .unwrap()
            .version,
        "1.4.0"
    );
}

#[test]
fn npm_semver_shared_golden_vectors_include_prerelease_and_build_metadata() {
    let vectors: Vec<Value> =
        serde_json::from_str(include_str!("npm-semver-fixtures.json")).unwrap();
    assert!(vectors.len() >= 24);
    for fixture in vectors {
        let required = super::resolver::range(fixture["range"].as_str().unwrap()).unwrap();
        let version: Version = fixture["version"].as_str().unwrap().parse().unwrap();
        assert_eq!(
            required.satisfies(&version),
            fixture["matches"].as_bool().unwrap(),
            "{fixture}"
        );
    }
}

#[test]
fn numeric_and_canonical_declaration_boundaries_match_the_shared_sdk_subset() {
    let vectors: Vec<Value> =
        serde_json::from_str(include_str!("npm-semver-declaration-fixtures.json")).unwrap();
    for fixture in vectors {
        let value = fixture["value"].as_str().unwrap();
        let canonical =
            PackageCandidate::from_manifest(&package("provider", value, vec![])).is_ok();
        assert_eq!(
            canonical,
            fixture["canonicalValid"].as_bool().unwrap(),
            "exact: {fixture}"
        );
        let declaration = package(
            "consumer",
            "1.0.0",
            vec![dependency("provider", value, "*")],
        );
        assert_eq!(
            PackageCandidate::from_manifest(&declaration).is_ok(),
            fixture["rangeValid"].as_bool().unwrap(),
            "range: {fixture}"
        );
    }
}

#[test]
fn normalized_wildcard_and_empty_intersection_control_actual_provider_selection() {
    for (required, succeeds) in [
        ("<=1.x", true),
        (">1.x", false),
        ("<1.x", false),
        (">=1.x", true),
        (">=2.0.0 <1.0.0", false),
    ] {
        let result = resolve(
            vec![
                package(
                    "consumer",
                    "1.0.0",
                    vec![dependency("provider", required, "*")],
                ),
                package("provider", "1.9.0", vec![]),
            ],
            &["consumer"],
        );
        assert_eq!(result.is_ok(), succeeds, "{required}");
        if !succeeds {
            assert_eq!(result.unwrap_err(), DependencyError::VersionConflict);
        }
    }
}

#[test]
fn package_and_interface_version_ranges_are_independent() {
    let mut provider = package("provider", "4.3.0", vec![]);
    provider["services"][0]["version"] = json!("1.8.0");
    let consumer = package(
        "consumer",
        "1.0.0",
        vec![dependency("provider", "^4.0.0", "~1.8.0")],
    );
    let plan = resolve(vec![consumer.clone(), provider.clone()], &["consumer"]).unwrap();
    assert_eq!(plan.lock.services[0].provider.version, "4.3.0");
    assert_eq!(plan.lock.services[0].version, "1.8.0");
    provider["services"][0]["version"] = json!("2.0.0");
    assert_eq!(
        resolve(vec![consumer, provider], &["consumer"]).unwrap_err(),
        DependencyError::VersionConflict
    );
}

#[test]
fn distinct_services_from_one_provider_intersect_the_package_constraints() {
    let mut provider = package("provider", "2.3.0", vec![]);
    provider["services"]
        .as_array_mut()
        .unwrap()
        .push(service("other-service", "3.1.0"));
    let mut other = dependency("provider", "~2.3.0", "^3.0.0");
    other["service"] = json!("other-service");
    let consumer = package(
        "consumer",
        "1.0.0",
        vec![dependency("provider", "^2.0.0", "^1.0.0"), other],
    );
    let plan = resolve(vec![consumer, provider], &["consumer"]).unwrap();
    assert_eq!(plan.lock.packages.len(), 2);
    assert_eq!(plan.lock.services.len(), 2);
    assert_eq!(
        plan.lock
            .reverse_dependencies
            .iter()
            .find(|item| item.provider.id == "provider")
            .unwrap()
            .consumers,
        vec![identity("consumer")]
    );
}

#[test]
fn publisher_is_fixed_and_never_falls_back_to_another_publisher() {
    let mut provider = package("provider", "1.0.0", vec![]);
    provider["publisher"] = json!("other-publisher");
    let consumer = package("consumer", "1.0.0", vec![dependency("provider", "*", "*")]);
    assert_eq!(
        resolve(vec![consumer, provider], &["consumer"]).unwrap_err(),
        DependencyError::Missing
    );
    let mut root = identity("consumer");
    root.publisher = "spoofed".into();
    assert_eq!(
        catalog(vec![package("consumer", "1.0.0", vec![])])
            .resolve(&[root], &target("windows"))
            .unwrap_err(),
        DependencyError::Missing
    );
}

#[test]
fn duplicate_provider_and_changed_artifact_are_distinct_catalog_errors() {
    let original = package("provider", "1.0.0", vec![]);
    assert_eq!(
        DependencyCatalog::from_manifests(vec![original.clone(), original.clone()], vec![])
            .err()
            .unwrap(),
        DependencyError::DuplicateProvider
    );
    let mut changed = original.clone();
    changed["files"][0]["sha256"] = json!("2".repeat(64));
    assert_eq!(
        DependencyCatalog::from_manifests(vec![original, changed], vec![])
            .err()
            .unwrap(),
        DependencyError::ArtifactConflict
    );
}

#[test]
fn root_or_inventory_order_does_not_change_the_lock_digest() {
    let first = package("consumer", "1.0.0", vec![dependency("provider", "*", "*")]);
    let second = package("provider", "1.0.0", vec![]);
    let a = resolve(
        vec![first.clone(), second.clone()],
        &["provider", "consumer"],
    )
    .unwrap();
    let b = resolve(vec![second, first], &["consumer", "provider"]).unwrap();
    assert_eq!(a, b);
    let mut changed = a.lock.clone();
    changed.target = target("linux");
    assert_ne!(changed.verified_digest().unwrap(), a.lock.digest);
    changed = a.lock.clone();
    changed.packages[0].digest = "0".repeat(64);
    assert_ne!(changed.verified_digest().unwrap(), a.lock.digest);
    changed = a.lock.clone();
    changed.roots.pop();
    assert_ne!(changed.verified_digest().unwrap(), a.lock.digest);
    assert!(serde_json::to_value(&a).unwrap().get("actions").is_none());
}

#[test]
fn cycle_and_self_cycle_fail_and_a_lower_acyclic_version_can_succeed() {
    assert_eq!(
        resolve(
            vec![
                package("one", "1.0.0", vec![dependency("two", "*", "*")]),
                package("two", "1.0.0", vec![dependency("one", "*", "*")])
            ],
            &["one"]
        )
        .unwrap_err(),
        DependencyError::Cycle
    );
    assert_eq!(
        resolve(
            vec![package("one", "1.0.0", vec![dependency("one", "*", "*")])],
            &["one"]
        )
        .unwrap_err(),
        DependencyError::Cycle
    );
    let plan = resolve(
        vec![
            package("one", "2.0.0", vec![dependency("one", "*", "*")]),
            package("one", "1.0.0", vec![]),
        ],
        &["one"],
    )
    .unwrap();
    assert_eq!(plan.lock.packages[0].version, "1.0.0");
}

#[test]
fn missing_conflicting_and_unsupported_packages_are_explicit_errors() {
    assert_eq!(
        resolve(vec![], &["missing"]).unwrap_err(),
        DependencyError::Missing
    );
    let mut unsupported = package("provider", "1.0.0", vec![]);
    unsupported["targets"] = json!([target("linux")]);
    assert_eq!(
        resolve(vec![unsupported], &["provider"]).unwrap_err(),
        DependencyError::TargetMismatch
    );
    assert_eq!(
        resolve(
            vec![
                package(
                    "consumer",
                    "1.0.0",
                    vec![dependency("provider", "^2.0.0", "*")]
                ),
                package("provider", "1.0.0", vec![])
            ],
            &["consumer"]
        )
        .unwrap_err(),
        DependencyError::VersionConflict
    );
    let mut dep = dependency("provider", "*", "*");
    dep["service"] = json!("undeclared");
    assert_eq!(
        resolve(
            vec![
                package("consumer", "1.0.0", vec![dep]),
                package("provider", "1.0.0", vec![])
            ],
            &["consumer"]
        )
        .unwrap_err(),
        DependencyError::VersionConflict
    );
}

#[test]
fn legacy_dependencies_are_metadata_and_cannot_guess_interfaces_or_tools() {
    let bare = json!({"publisher":"flowtools","id":"provider","version":"*"});
    assert_eq!(
        resolve(
            vec![package("consumer", "1.0.0", vec![bare])],
            &["consumer"]
        )
        .unwrap_err(),
        DependencyError::InterfaceRequired
    );
    let mut consumer = package("consumer", "1.0.0", vec![]);
    consumer["dependencies"]["tools"] =
        json!([{"publisher":"flowtools","id":"fixture-tool","version":"*"}]);
    assert_eq!(
        resolve(vec![consumer], &["consumer"]).unwrap_err(),
        DependencyError::InvalidDeclaration
    );
}

#[test]
fn incomplete_duplicate_or_mixed_selectors_are_rejected_without_guessing() {
    let base = dependency("provider", "*", "*");
    let mut partial = base.clone();
    partial.as_object_mut().unwrap().remove("interfaceVersion");
    let bare = json!({"publisher":"flowtools","id":"provider","version":"*"});
    for deps in [
        vec![partial],
        vec![base.clone(), base.clone()],
        vec![bare, base],
    ] {
        assert_eq!(
            DependencyCatalog::from_manifests(vec![package("consumer", "1.0.0", deps)], vec![])
                .err()
                .unwrap(),
            DependencyError::InvalidDeclaration
        );
    }
    let hash = "1".repeat(64);
    let base = tool_dependency("*", "windows", &hash);
    let mut partial = base.clone();
    partial.as_object_mut().unwrap().remove("digest");
    let bare = json!({"publisher":"flowtools","id":"fixture-tool","version":"*"});
    let mut different = base.clone();
    different["digest"] = json!("2".repeat(64));
    for deps in [
        vec![partial],
        vec![base.clone(), different],
        vec![bare, base],
    ] {
        let mut consumer = package("consumer", "1.0.0", vec![]);
        consumer["dependencies"]["tools"] = json!(deps);
        assert_eq!(
            DependencyCatalog::from_manifests(vec![consumer], vec![])
                .err()
                .unwrap(),
            DependencyError::InvalidDeclaration
        );
    }
}

#[test]
fn service_entry_operations_and_versions_have_defensive_declaration_checks() {
    for (field, value) in [("version", json!("v1.0.0")), ("id", json!("Bad-ID"))] {
        let mut fixture = package("provider", "1.0.0", vec![]);
        fixture[field] = value;
        assert_eq!(
            PackageCandidate::from_manifest(&fixture).err().unwrap(),
            DependencyError::InvalidDeclaration
        );
    }
    let mut fixture = package("provider", "1.0.0", vec![]);
    fixture["services"][0]["operations"][0]["headless"] = json!(false);
    assert_eq!(
        PackageCandidate::from_manifest(&fixture).err().unwrap(),
        DependencyError::InvalidDeclaration
    );
    fixture = package("provider", "1.0.0", vec![]);
    fixture["services"][0]["operations"][0]["interaction"] = json!("required");
    assert_eq!(
        PackageCandidate::from_manifest(&fixture).err().unwrap(),
        DependencyError::InvalidDeclaration
    );
    fixture = package("provider", "1.0.0", vec![]);
    fixture["entries"]
        .as_object_mut()
        .unwrap()
        .remove("services");
    assert_eq!(
        PackageCandidate::from_manifest(&fixture).err().unwrap(),
        DependencyError::InvalidDeclaration
    );
    fixture = package("provider", "1.0.0", vec![]);
    fixture["files"] = json!([]);
    assert_eq!(
        PackageCandidate::from_manifest(&fixture).err().unwrap(),
        DependencyError::InvalidDeclaration
    );
    fixture = package("provider", "1.0.0", vec![]);
    fixture["services"]
        .as_array_mut()
        .unwrap()
        .push(service("text-transform", "1.3.0"));
    assert_eq!(
        PackageCandidate::from_manifest(&fixture).err().unwrap(),
        DependencyError::InvalidDeclaration
    );
}

#[test]
fn canonical_declaration_versions_match_sdk_normalization_without_build_metadata() {
    let fixture = package("provider", "1.2.3+build.7", vec![]);
    assert_eq!(
        PackageCandidate::from_manifest(&fixture).err().unwrap(),
        DependencyError::InvalidDeclaration
    );
    let mut fixture = package("provider", "1.2.3", vec![]);
    fixture["services"][0]["version"] = json!("1.2.3+build.7");
    assert_eq!(
        PackageCandidate::from_manifest(&fixture).err().unwrap(),
        DependencyError::InvalidDeclaration
    );
    assert_eq!(
        DependencyCatalog::from_manifests(
            vec![],
            vec![tool("1.2.3+build.7", "windows", &"1".repeat(64))]
        )
        .err()
        .unwrap(),
        DependencyError::InvalidDeclaration
    );
}

#[test]
fn consumer_specific_tool_versions_lock_target_flavor_and_exact_digest() {
    let one = "1".repeat(64);
    let two = "2".repeat(64);
    let mut a = package("consumer-one", "1.0.0", vec![]);
    a["dependencies"]["tools"] = json!([tool_dependency("^1.0.0", "windows", &one)]);
    let mut b = package("consumer-two", "1.0.0", vec![]);
    b["dependencies"]["tools"] = json!([tool_dependency("^2.0.0", "windows", &two)]);
    let plan = DependencyCatalog::from_manifests(
        vec![a, b],
        vec![
            tool("1.2.0", "windows", &one),
            tool("2.4.0", "windows", &two),
        ],
    )
    .unwrap()
    .resolve(
        &[identity("consumer-one"), identity("consumer-two")],
        &target("windows"),
    )
    .unwrap();
    assert_eq!(plan.lock.tools.len(), 2);
    assert_eq!(plan.lock.tools[0].consumer.id, "consumer-one");
    assert_eq!(plan.lock.tools[0].version, "1.2.0");
    assert_eq!(plan.lock.tools[1].consumer.id, "consumer-two");
    assert_eq!(plan.lock.tools[1].version, "2.4.0");
    assert_eq!(plan.lock.tools[0].digest, one);
    assert_eq!(plan.lock.tools[1].digest, two);
    assert!(plan
        .lock
        .tools
        .iter()
        .all(|pin| pin.target == target("windows") && pin.build_flavor == "standard"));
    assert!(plan.lock.edges.is_empty());
}

#[test]
fn multi_platform_declarations_lock_only_the_host_target_and_reject_uncovered_groups() {
    let windows_hash = "1".repeat(64);
    let linux_hash = "2".repeat(64);
    let mut consumer = package("consumer", "1.0.0", vec![]);
    consumer["dependencies"]["tools"] = json!([
        tool_dependency("*", "windows", &windows_hash),
        tool_dependency("*", "linux", &linux_hash)
    ]);
    let catalog = DependencyCatalog::from_manifests(
        vec![consumer],
        vec![
            tool("1.0.0", "windows", &windows_hash),
            tool("1.0.0", "linux", &linux_hash),
        ],
    )
    .unwrap();
    for (platform, hash) in [("windows", windows_hash), ("linux", linux_hash)] {
        let plan = catalog
            .resolve(&[identity("consumer")], &target(platform))
            .unwrap();
        assert_eq!(plan.lock.tools.len(), 1);
        assert_eq!(plan.lock.tools[0].target, target(platform));
        assert_eq!(plan.lock.tools[0].digest, hash);
    }
    let mut consumer = package("consumer", "1.0.0", vec![]);
    consumer["dependencies"]["tools"] = json!([tool_dependency("*", "linux", &"1".repeat(64))]);
    assert_eq!(
        resolve(vec![consumer], &["consumer"]).unwrap_err(),
        DependencyError::TargetMismatch
    );
}

#[test]
fn tools_fail_before_execution_for_missing_version_platform_flavor_or_hash() {
    let one = "1".repeat(64);
    let mut consumer = package("consumer", "1.0.0", vec![]);
    consumer["dependencies"]["tools"] = json!([tool_dependency("^1.0.0", "windows", &one)]);
    for (tools, expected) in [
        (vec![], DependencyError::Missing),
        (
            vec![tool("1.0.0", "linux", &one)],
            DependencyError::TargetMismatch,
        ),
        (
            vec![tool("2.0.0", "windows", &one)],
            DependencyError::VersionConflict,
        ),
        (
            vec![tool("1.0.0", "windows", &"2".repeat(64))],
            DependencyError::ArtifactConflict,
        ),
    ] {
        assert_eq!(
            DependencyCatalog::from_manifests(vec![consumer.clone()], tools)
                .unwrap()
                .resolve(&[identity("consumer")], &target("windows"))
                .unwrap_err(),
            expected
        );
    }
    let mut wrong_flavor = tool("1.0.0", "windows", &one);
    wrong_flavor.build_flavor = "other".into();
    assert_eq!(
        DependencyCatalog::from_manifests(vec![consumer], vec![wrong_flavor])
            .unwrap()
            .resolve(&[identity("consumer")], &target("windows"))
            .unwrap_err(),
        DependencyError::ArtifactConflict
    );
    assert_eq!(
        DependencyCatalog::from_manifests(
            vec![],
            vec![
                tool("1.0.0", "windows", &one),
                tool("1.0.0", "windows", &one)
            ]
        )
        .err()
        .unwrap(),
        DependencyError::DuplicateProvider
    );
    assert_eq!(
        DependencyCatalog::from_manifests(
            vec![],
            vec![
                tool("1.0.0", "windows", &one),
                tool("1.0.0", "windows", &"2".repeat(64))
            ]
        )
        .err()
        .unwrap(),
        DependencyError::ArtifactConflict
    );
}

#[test]
fn unavailable_tool_on_the_highest_provider_causes_complete_backtracking() {
    let mut highest = package("provider", "2.0.0", vec![]);
    highest["dependencies"]["tools"] = json!([tool_dependency("*", "windows", &"1".repeat(64))]);
    let plan = resolve(
        vec![highest, package("provider", "1.0.0", vec![])],
        &["provider"],
    )
    .unwrap();
    assert_eq!(plan.lock.packages[0].version, "1.0.0");
}

#[test]
fn invalid_root_and_catalog_resource_budgets_are_finite() {
    for valid in ["plugin-todo-list", "a", "0", &"a".repeat(128)] {
        assert!(valid_dependency_id(valid));
    }
    for invalid in ["", "../bad", "Bad", "two--parts", "a_", &"a".repeat(129)] {
        assert!(!valid_dependency_id(invalid));
    }
    let inventory = catalog(vec![]);
    assert_eq!(
        inventory.resolve(&[], &target("windows")).unwrap_err(),
        DependencyError::InvalidDeclaration
    );
    assert_eq!(
        inventory
            .resolve(&[identity("one"), identity("one")], &target("windows"))
            .unwrap_err(),
        DependencyError::InvalidDeclaration
    );
    assert_eq!(
        inventory
            .resolve(&[identity("bad/path")], &target("windows"))
            .unwrap_err(),
        DependencyError::InvalidDeclaration
    );
    assert_eq!(
        inventory
            .resolve(&[identity("one")], &target("unknown"))
            .unwrap_err(),
        DependencyError::InvalidDeclaration
    );
    let roots: Vec<_> = (0..129).map(|n| identity(&format!("root-{n}"))).collect();
    assert_eq!(
        inventory.resolve(&roots, &target("windows")).unwrap_err(),
        DependencyError::BudgetExceeded
    );
    let packages = (0..513)
        .map(|n| package(&format!("package-{n}"), "1.0.0", vec![]))
        .collect();
    assert_eq!(
        DependencyCatalog::from_manifests(packages, vec![])
            .err()
            .unwrap(),
        DependencyError::BudgetExceeded
    );
    let mut large = package("large", "1.0.0", vec![]);
    large["description"] = json!("x".repeat(crate::protocol::MAX_FRAME_BYTES));
    assert_eq!(
        PackageCandidate::from_manifest(&large).err().unwrap(),
        DependencyError::BudgetExceeded
    );
}

#[test]
fn exhaustive_unsatisfiable_branches_end_at_search_budget() {
    // Every choice has two candidates; the lexically last missing root prevents
    // a solution. The bounded exhaustive search must stop, without partial lock.
    let mut packages = Vec::new();
    let mut roots = Vec::new();
    for n in 0..14 {
        let name = format!("choice-{n:02}");
        roots.push(identity(&name));
        packages.push(package(&name, "2.0.0", vec![]));
        packages.push(package(&name, "1.0.0", vec![]));
    }
    roots.push(identity("zz-missing"));
    assert_eq!(
        catalog(packages)
            .resolve(&roots, &target("windows"))
            .unwrap_err(),
        DependencyError::BudgetExceeded
    );
}

#[test]
fn oversized_candidate_graph_can_backtrack_to_a_bounded_version() {
    let dependencies = (0..128)
        .map(|n| dependency(&format!("child-{n}"), "*", "*"))
        .collect();
    let highest = package("root", "2.0.0", dependencies);
    assert_eq!(
        resolve(vec![highest.clone()], &["root"]).unwrap_err(),
        DependencyError::BudgetExceeded
    );
    let plan = resolve(vec![highest, package("root", "1.0.0", vec![])], &["root"]).unwrap();
    assert_eq!(plan.lock.packages[0].version, "1.0.0");
}

#[test]
fn oversized_serialized_lock_can_backtrack_within_graph_bounds() {
    let provider_id = format!("provider-{}", "x".repeat(100));
    let mut provider = package(&provider_id, "1.0.0", vec![]);
    let services: Vec<_> = (0..64)
        .map(|n| service(&format!("service-{n:02}-{}", "x".repeat(100)), "1.0.0"))
        .collect();
    provider["services"] = json!(services);
    let mut packages = vec![provider];
    let mut root_dependencies = Vec::new();
    for n in 0..63 {
        let consumer_id = format!("consumer-{n:02}-{}", "x".repeat(100));
        let dependencies: Vec<_> = (0..64)
            .map(|service_n| {
                let mut dep = dependency(&provider_id, "*", "*");
                dep["service"] = json!(format!("service-{service_n:02}-{}", "x".repeat(100)));
                dep
            })
            .collect();
        packages.push(package(&consumer_id, "1.0.0", dependencies));
        root_dependencies.push(dependency(&consumer_id, "*", "*"));
    }
    // 65 packages and 4095 edges fit graph limits, but repeated exact identity
    // and service pins exceed the 1 MiB frame budget. No partial lock escapes.
    packages.push(package("root", "2.0.0", root_dependencies));
    assert_eq!(
        resolve(packages.clone(), &["root"]).unwrap_err(),
        DependencyError::BudgetExceeded
    );
    packages.push(package("root", "1.0.0", vec![]));
    let plan = resolve(packages, &["root"]).unwrap();
    assert_eq!(plan.lock.packages.len(), 1);
    assert_eq!(plan.lock.packages[0].version, "1.0.0");
}

#[test]
fn dependency_failures_map_to_stable_wire_codes() {
    for (error, code) in [
        (
            DependencyError::InvalidDeclaration,
            ErrorCode::DependencyInvalid,
        ),
        (
            DependencyError::InterfaceRequired,
            ErrorCode::DependencyInvalid,
        ),
        (DependencyError::Missing, ErrorCode::DependencyMissing),
        (
            DependencyError::VersionConflict,
            ErrorCode::DependencyConflict,
        ),
        (
            DependencyError::DuplicateProvider,
            ErrorCode::DependencyDuplicateProvider,
        ),
        (
            DependencyError::TargetMismatch,
            ErrorCode::DependencyPlatformMismatch,
        ),
        (
            DependencyError::ArtifactConflict,
            ErrorCode::DependencyArtifactMismatch,
        ),
        (DependencyError::Cycle, ErrorCode::DependencyCycle),
        (
            DependencyError::BudgetExceeded,
            ErrorCode::DependencyBudgetExceeded,
        ),
    ] {
        assert_eq!(ErrorCode::from(error), code);
    }
}
