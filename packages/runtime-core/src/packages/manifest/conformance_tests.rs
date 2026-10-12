use super::*;
use std::collections::BTreeSet;

#[test]
fn build_owned_compatibility_matches_native_runtime_sdk_and_target() {
    let target = ManifestCompatibility::embedded().unwrap();
    let native_version = include_str!("../../../../../apps/runtime/Cargo.toml")
        .lines()
        .find_map(|line| {
            line.strip_prefix("version = ")
                .and_then(|version| serde_json::from_str::<String>(version).ok())
        })
        .unwrap();
    let sdk: Value = serde_json::from_str(include_str!("../../../../sdk/package.json")).unwrap();
    assert_eq!(target.host_version, native_version);
    assert_eq!(target.sdk_version, sdk["version"]);
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        unsupported => panic!("Unsupported build architecture: {unsupported}"),
    };
    assert_eq!(
        serde_json::to_value(&target.target).unwrap(),
        json!({"platform":std::env::consts::OS,"arch":arch})
    );
}

#[test]
fn sdk_generated_manifest_conformance_includes_normalized_data() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../manifest-conformance.json")).unwrap();
    assert_eq!(corpus["formatVersion"], 1);
    let cases = corpus["cases"].as_array().unwrap();
    assert!(cases.len() >= 405);
    let mut ids = BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id), "Duplicate SDK case: {id}");
        let manifest = match case["rawManifest"].as_str() {
            Some(raw) => serde_json::from_str(raw).unwrap(),
            None => case["manifest"].clone(),
        };
        let target = case.get("target").map(|target| ManifestCompatibility {
            host_version: target["hostVersion"].as_str().unwrap().into(),
            sdk_version: target["sdkVersion"].as_str().unwrap().into(),
            target: serde_json::from_value(json!({
                "platform":target["platform"], "arch":target["arch"]
            }))
            .unwrap(),
        });
        // SDK authoring data semantics are distinct from the signed archive's
        // original-UTF8-byte profile enforced by validate_bytes.
        let result = validate_value(&manifest, target.as_ref());
        assert_eq!(
            result.is_ok(),
            case["accepted"].as_bool().unwrap(),
            "{id}: {result:?}"
        );
        if let Ok(result) = result {
            let mut expected = case["normalized"].clone();
            normalize_numbers(&mut expected).unwrap();
            assert_eq!(result, expected, "{id}");
        }
    }
}

#[test]
fn full_manifest_propagates_additional_installation_matching_budget() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../manifest-conformance.json")).unwrap();
    let mut value = corpus["cases"][0]["manifest"].clone();
    assert_eq!(corpus["cases"][0]["id"], "valid-base");
    let choices: Vec<_> = (2..=256).chain(std::iter::once(1)).collect();
    value["commands"][0]["outputSchema"] = json!({
        "type":"array", "items":{"enum":choices}, "default":vec![1;30_000]
    });
    assert_eq!(
        validate_value(&value, None),
        Err(PackageError::ManifestBudgetExceeded)
    );
    assert_eq!(
        validate_bytes(
            &serde_json::to_vec(&value).unwrap(),
            &ManifestCompatibility::embedded().unwrap()
        ),
        Err(PackageError::ManifestBudgetExceeded)
    );
    assert_eq!(
        PackageError::ManifestBudgetExceeded.code(),
        "PACKAGE_MANIFEST_BUDGET_EXCEEDED"
    );
}
