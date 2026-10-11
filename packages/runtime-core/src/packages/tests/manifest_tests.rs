use super::*;
use crate::packages::manifest::{validate_bytes, validate_plugin_manifest, ManifestCompatibility};
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn cases() -> Value {
    serde_json::from_str(include_str!(
        "../../../../sdk/test/fixtures/manifest-admission-v1.json"
    ))
    .unwrap()
}
fn target() -> ManifestCompatibility {
    ManifestCompatibility {
        host_version: "0.1.0".into(),
        sdk_version: "0.0.0".into(),
        target: PackageTarget {
            platform: PackagePlatform::Windows,
            arch: PackageArch::X64,
        },
    }
}
#[test]
fn all_twelve_sdk_validated_builtin_manifests_also_pass_native_full_validation() {
    let catalog = crate::catalog::BuiltinCatalog::embedded();
    let mut count = 0;
    for (_, value) in catalog.list() {
        assert!(
            validate_bytes(&serde_json::to_vec(value).unwrap(), &target()).is_ok(),
            "{}",
            value["id"]
        );
        count += 1;
    }
    assert_eq!(count, 12);
}
#[test]
fn sdk_and_rust_manifest_admission_agree_on_all_shared_cases() {
    let fixtures = cases();
    assert_eq!(fixtures["formatVersion"], 1);
    assert!(fixtures["cases"].as_array().unwrap().len() >= 90);
    for case in fixtures["cases"].as_array().unwrap() {
        let mut value = fixtures["base"].clone();
        for patch in case["patches"].as_array().unwrap() {
            let pointer = patch["path"].as_str().unwrap();
            if patch["remove"] == true {
                let (parent, key) = pointer.rsplit_once('/').unwrap();
                value
                    .pointer_mut(parent)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
            } else if let Some(slot) = value.pointer_mut(pointer) {
                *slot = patch["value"].clone();
            } else {
                let (parent, key) = pointer.rsplit_once('/').unwrap();
                value
                    .pointer_mut(parent)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), patch["value"].clone());
            }
        }
        let mut target_value = fixtures["target"].clone();
        if let Some(changes) = case["target"].as_object() {
            for (key, value) in changes {
                target_value[key] = value.clone();
            }
        }
        let compatibility = ManifestCompatibility {
            host_version: target_value["hostVersion"].as_str().unwrap().into(),
            sdk_version: target_value["sdkVersion"].as_str().unwrap().into(),
            target: serde_json::from_value(
                json!({"platform":target_value["platform"],"arch":target_value["arch"]}),
            )
            .unwrap(),
        };
        let result = validate_bytes(&serde_json::to_vec(&value).unwrap(), &compatibility);
        assert_eq!(
            result.is_ok(),
            case["accept"] == true,
            "{}: {:?}",
            case["name"],
            result
        );
    }
}
fn signed_archive(mut value: Value, kind: PackageKind) -> archive::ValidatedArchive {
    let mut fixture = Fixture::new();
    let source = b"throw new Error('MANIFEST_FIXTURE_MUST_NOT_EXECUTE')";
    fixture.package.kind = kind.clone();
    if kind == PackageKind::Tool {
        let mut root = fixture.root.payload().clone();
        root.publishers[0].packages[0].kind = PackageKind::Tool;
        let pins = BootstrapPins {
            keys: serde_json::from_value(fixture.golden["pins"].clone()).unwrap(),
            threshold: 2,
        };
        fixture.root = bootstrap_root(
            &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]),
            &pins,
            fixture.policy.clock,
        )
        .unwrap();
    }
    value["publisher"] = json!(fixture.package.publisher);
    value["id"] = json!(fixture.package.id);
    value["version"] = json!(fixture.package.version);
    fixture.package.files = vec![PackageFile {
        path: "dist/commands.js".into(),
        sha256: hash(source),
        size: source.len() as u64,
    }];
    value["files"] = serde_json::to_value(&fixture.package.files).unwrap();
    if kind == PackageKind::Tool {
        value["target"] = serde_json::to_value(&fixture.package.target).unwrap();
    }
    fixture.manifest = serde_json::to_vec(&value).unwrap();
    fixture.package.manifest = BlobDescriptor {
        sha256: hash(&fixture.manifest),
        size: fixture.manifest.len() as u64,
    };
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("manifest.json", fixture.manifest.as_slice()),
        ("dist/commands.js", source.as_slice()),
    ] {
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(bytes).unwrap();
    }
    fixture.archive = zip.finish().unwrap().into_inner();
    fixture.package.archive.sha256 = hash(&fixture.archive);
    fixture.package.archive.size = fixture.archive.len() as u64;
    archive::validate_archive(
        &fixture.verify().unwrap(),
        &fixture.manifest,
        &fixture.archive,
    )
    .unwrap()
}
#[test]
fn signed_zip_manifest_admission_preserves_original_bytes_and_prototype_default() {
    let mut value = cases()["base"].clone();
    value.as_object_mut().unwrap().remove("maturity");
    let archive = signed_archive(value, PackageKind::Plugin);
    let original = archive.files()["manifest.json"].clone();
    let admitted = validate_plugin_manifest(archive, &target()).unwrap();
    assert_eq!(admitted.manifest()["maturity"], "prototype");
    assert_eq!(admitted.archive().files()["manifest.json"], original);
    assert!(admitted.archive().files()["dist/commands.js"].starts_with(b"throw new Error"));
    assert_eq!(
        admitted.archive().descriptor().payload().archive.sha256,
        admitted.archive().descriptor().package_digest()
    );
}
#[test]
fn signed_and_byte_valid_is_not_enough_for_semantically_invalid_manifest_or_target() {
    let mut value = cases()["base"].clone();
    value["commands"][0]["supportsColdStart"] = json!(true);
    value["commands"][0]["headless"] = json!(false);
    assert!(matches!(
        validate_plugin_manifest(signed_archive(value, PackageKind::Plugin), &target()),
        Err(PackageError::ManifestInvalid)
    ));
    let mut compatibility = target();
    compatibility.sdk_version = "1.0.0".into();
    assert!(matches!(
        validate_plugin_manifest(
            signed_archive(cases()["base"].clone(), PackageKind::Plugin),
            &compatibility
        ),
        Err(PackageError::ManifestIncompatible)
    ));
    compatibility = target();
    compatibility.target.arch = PackageArch::Arm64;
    assert!(matches!(
        validate_plugin_manifest(
            signed_archive(cases()["base"].clone(), PackageKind::Plugin),
            &compatibility
        ),
        Err(PackageError::ManifestIncompatible)
    ));
}
#[test]
fn a_signed_tool_descriptor_cannot_be_used_as_plugin_manifest_admission() {
    assert!(matches!(
        validate_plugin_manifest(
            signed_archive(cases()["base"].clone(), PackageKind::Tool),
            &target()
        ),
        Err(PackageError::ManifestInvalid)
    ));
}
#[test]
fn raw_manifest_duplicate_keys_unsafe_json_depth_and_bytes_are_rejected() {
    let base = serde_json::to_string(&cases()["base"]).unwrap();
    let duplicate = base.replacen("{", "{\"formatVersion\":1,", 1);
    assert!(validate_bytes(duplicate.as_bytes(), &target()).is_err());
    let mut value = cases()["base"].clone();
    value["commands"][0]["inputSchema"]["properties"]["constructor"] = json!({"type":"string"});
    assert!(validate_bytes(&serde_json::to_vec(&value).unwrap(), &target()).is_err());
    let mut schema = json!({"type":"string"});
    for _ in 0..33 {
        schema = json!({"type":"array","items":schema});
    }
    value = cases()["base"].clone();
    value["commands"][0]["outputSchema"] = schema;
    assert!(validate_bytes(&serde_json::to_vec(&value).unwrap(), &target()).is_err());
    assert!(validate_bytes(&vec![b' '; 1_048_577], &target()).is_err());
    value = cases()["base"].clone();
    value["commands"][0]["outputSchema"] = json!({"const":vec![Value::Null;100_000]});
    assert!(validate_bytes(&serde_json::to_vec(&value).unwrap(), &target()).is_err());
    value = cases()["base"].clone();
    value["commands"][0]["outputSchema"] = json!({"const":"x".repeat(65_536)});
    assert!(validate_bytes(&serde_json::to_vec(&value).unwrap(), &target()).is_err());
    value = cases()["base"].clone();
    value["commands"][0]["outputSchema"] = serde_json::from_str("{\"enum\":[1,1.0]}").unwrap();
    assert!(validate_bytes(&serde_json::to_vec(&value).unwrap(), &target()).is_err());
    value["commands"][0]["outputSchema"] =
        serde_json::from_str("{\"const\":{\"x\":1},\"default\":{\"x\":1.0}}").unwrap();
    assert!(validate_bytes(&serde_json::to_vec(&value).unwrap(), &target()).is_ok());
}
