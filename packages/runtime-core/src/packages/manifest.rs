//! SDK-generated Manifest structure with explicit, versioned native semantics.
//! Inert admission only: no installation, lock authority, grants or execution.
#[cfg(test)]
mod conformance_tests;
mod operation;

use super::{
    archive::ValidatedArchive, strict_json, PackageArch, PackageError, PackageKind,
    PackagePlatform, PackageTarget,
};
use crate::dependencies::{manifest_range, manifest_version};
use serde_json::{json, Value};
use std::{collections::HashSet, sync::OnceLock};

/// T0 build/runtime compatibility, never an operation payload or grant.
/// External callers obtain this from the current build via [`Self::embedded`].
///
/// ```compile_fail
/// use flowtools_runtime_core::packages::{PackageTarget, PackagePlatform, PackageArch};
/// use flowtools_runtime_core::packages::manifest::ManifestCompatibility;
/// let supplied = ManifestCompatibility {
///     host_version: "999.0.0".into(),
///     sdk_version: "999.0.0".into(),
///     target: PackageTarget { platform: PackagePlatform::Windows, arch: PackageArch::X64 },
/// };
/// ```
pub struct ManifestCompatibility {
    pub(crate) host_version: String,
    pub(crate) sdk_version: String,
    pub(crate) target: PackageTarget,
}

impl ManifestCompatibility {
    /// The compatibility authority is this T0 build, never package or IPC data.
    pub fn embedded() -> Result<Self, PackageError> {
        let host: Value =
            serde_json::from_str(include_str!("../../../../apps/runtime/package.json"))
                .map_err(|_| PackageError::ManifestIncompatible)?;
        let sdk: Value = serde_json::from_str(include_str!("../../../sdk/package.json"))
            .map_err(|_| PackageError::ManifestIncompatible)?;
        let platform = match std::env::consts::OS {
            "windows" => PackagePlatform::Windows,
            "macos" => PackagePlatform::Macos,
            "linux" => PackagePlatform::Linux,
            _ => return Err(PackageError::ManifestIncompatible),
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => PackageArch::X64,
            "aarch64" => PackageArch::Arm64,
            _ => return Err(PackageError::ManifestIncompatible),
        };
        Ok(Self {
            host_version: host["version"]
                .as_str()
                .ok_or(PackageError::ManifestIncompatible)?
                .into(),
            sdk_version: sdk["version"]
                .as_str()
                .ok_or(PackageError::ManifestIncompatible)?
                .into(),
            target: PackageTarget { platform, arch },
        })
    }
}

/// Contains the original verified bytes and normalized metadata. Still inert.
pub struct CompatiblePluginArchive {
    archive: ValidatedArchive,
    manifest: Value,
}
impl CompatiblePluginArchive {
    pub fn archive(&self) -> &ValidatedArchive {
        &self.archive
    }
    pub fn manifest(&self) -> &Value {
        &self.manifest
    }
}

pub fn validate_plugin_manifest(
    archive: ValidatedArchive,
    target: &ManifestCompatibility,
) -> Result<CompatiblePluginArchive, PackageError> {
    let descriptor = archive.descriptor().payload();
    if descriptor.kind != PackageKind::Plugin {
        return Err(PackageError::ManifestInvalid);
    }
    if descriptor.target != target.target {
        return Err(PackageError::ManifestIncompatible);
    }
    let bytes = archive
        .files()
        .get("manifest.json")
        .ok_or(PackageError::ManifestInvalid)?;
    let manifest = validate_bytes(bytes, target)?;
    Ok(CompatiblePluginArchive { archive, manifest })
}

struct Contract {
    manifest: Value,
    operation: Value,
    manifest_validator: jsonschema::Validator,
    operation_validator: jsonschema::Validator,
}
fn contract() -> Result<&'static Contract, PackageError> {
    static CONTRACT: OnceLock<Result<Contract, PackageError>> = OnceLock::new();
    CONTRACT
        .get_or_init(|| {
            let value: Value =
                serde_json::from_str(include_str!("../../.generated/manifest-validation.json"))
                    .map_err(|_| PackageError::ManifestInvalid)?;
            if value["formatVersion"] != 1 || value["semantics"] != "flowtools-manifest-v1" {
                return Err(PackageError::ManifestInvalid);
            }
            let manifest = value["manifest"].clone();
            let operation = value["operation"].clone();
            let manifest_validator =
                jsonschema::validator_for(&manifest).map_err(|_| PackageError::ManifestInvalid)?;
            let operation_validator =
                jsonschema::validator_for(&operation).map_err(|_| PackageError::ManifestInvalid)?;
            Ok(Contract {
                manifest,
                operation,
                manifest_validator,
                operation_validator,
            })
        })
        .as_ref()
        .map_err(|error| *error)
}

fn invalid<T>() -> Result<T, PackageError> {
    Err(PackageError::ManifestInvalid)
}
// JSON.parse produces IEEE754 Numbers. Preserve integral values in the safe
// domain in normalized metadata, rounding larger JSON numbers as JS does.
fn normalize_numbers(value: &mut Value) -> Result<(), PackageError> {
    match value {
        Value::Number(number) => {
            let number = number
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or(PackageError::ManifestInvalid)?;
            *value = if number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_991.0 {
                Value::from(number as i64)
            } else {
                Value::Number(
                    serde_json::Number::from_f64(number).ok_or(PackageError::ManifestInvalid)?,
                )
            };
        }
        Value::Array(values) => {
            for value in values {
                normalize_numbers(value)?;
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                normalize_numbers(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn validate_bytes(
    bytes: &[u8],
    target: &ManifestCompatibility,
) -> Result<Value, PackageError> {
    // The signed installation profile bounds original UTF-8 bytes separately
    // from the SDK's JSON.stringify UTF-16 data budget.
    if bytes.len() > 1_048_576 {
        return invalid();
    }
    let value = strict_json(bytes).map_err(|_| PackageError::ManifestInvalid)?;
    validate_value(&value, Some(target))
}

fn validate_value(
    value: &Value,
    target: Option<&ManifestCompatibility>,
) -> Result<Value, PackageError> {
    if !operation::bounded_json(value, 1_048_576) {
        return invalid();
    }
    let mut value = value.clone();
    normalize_numbers(&mut value)?;
    let contract = contract()?;
    if !contract.manifest_validator.is_valid(&value) {
        return invalid();
    }
    check(&contract.manifest, &value, &contract.manifest)?;
    if let Some(target) = target {
        let host = target
            .host_version
            .parse::<node_semver::Version>()
            .map_err(|_| PackageError::ManifestIncompatible)?;
        let sdk = target
            .sdk_version
            .parse::<node_semver::Version>()
            .map_err(|_| PackageError::ManifestIncompatible)?;
        let host_range = manifest_range(text(&value["engines"]["host"])?)
            .map_err(|_| PackageError::ManifestInvalid)?;
        let sdk_range = manifest_range(text(&value["engines"]["sdk"])?)
            .map_err(|_| PackageError::ManifestInvalid)?;
        let platform =
            serde_json::to_value(&target.target).map_err(|_| PackageError::ManifestIncompatible)?;
        if !host_range.satisfies(&host)
            || !sdk_range.satisfies(&sdk)
            || !array(&value["targets"])?.contains(&platform)
        {
            return Err(PackageError::ManifestIncompatible);
        }
    }
    value
        .as_object_mut()
        .ok_or(PackageError::ManifestInvalid)?
        .entry("maturity")
        .or_insert(json!("prototype"));
    Ok(value)
}
fn text(value: &Value) -> Result<&str, PackageError> {
    value.as_str().ok_or(PackageError::ManifestInvalid)
}
fn array(value: &Value) -> Result<&Vec<Value>, PackageError> {
    value.as_array().ok_or(PackageError::ManifestInvalid)
}
fn unique(items: impl IntoIterator<Item = Value>) -> bool {
    let mut values = Vec::new();
    for item in items {
        if values.iter().any(|value| same(value, &item)) {
            return false;
        }
        values.push(item);
    }
    true
}

fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => left.as_f64() == right.as_f64(),
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, left)| right.get(key).is_some_and(|right| same(left, right)))
        }
        _ => left == right,
    }
}
/// Walk the generated shape after structural validation. Unknown semantics fail.
fn check(schema: &Value, value: &Value, root: &Value) -> Result<(), PackageError> {
    if let Some(reference) = schema["$ref"].as_str() {
        let pointer = reference
            .strip_prefix('#')
            .ok_or(PackageError::ManifestInvalid)?;
        check(
            root.pointer(pointer).ok_or(PackageError::ManifestInvalid)?,
            value,
            root,
        )?;
    }
    if let Some(branches) = schema["allOf"].as_array() {
        for branch in branches {
            check(branch, value, root)?;
        }
    }
    if let Some(string) = value.as_str() {
        // JSON Schema counts Unicode scalars; Zod string limits count UTF-16.
        let size = string.encode_utf16().count() as u64;
        if schema["maxLength"]
            .as_u64()
            .is_some_and(|limit| size > limit)
            || schema["minLength"]
                .as_u64()
                .is_some_and(|limit| size < limit)
        {
            return invalid();
        }
    }
    if let Some(tag) = schema["x-flowtools-check"].as_str() {
        let valid = match tag {
            "version" => manifest_version(text(value)?).is_ok(),
            "range" => manifest_range(text(value)?).is_ok(),
            // Match SDK metadata; signed ZIP admission independently enforces its
            // stricter lowercase ASCII profile before this module is reached.
            "path" => portable_path(text(value)?),
            "unique" => unique(array(value)?.iter().cloned()),
            "manifest-v1" => manifest_semantics(value)?,
            "command-v1" => command_semantics(value)?,
            "dependencies-v1" => dependencies_semantics(value)?,
            "services-v1" => unique(array(value)?.iter().map(|item| item["id"].clone())),
            "service-v1" => {
                let operations = array(&value["operations"])?;
                unique(operations.iter().map(|item| item["id"].clone()))
                    && operations
                        .iter()
                        .all(|item| item["headless"] == true && item["interaction"] == "none")
            }
            "service-selector-v1" => {
                value.get("service").is_some() == value.get("interfaceVersion").is_some()
            }
            "tool-selector-v1" => {
                let fields =
                    ["target", "buildFlavor", "digest"].map(|key| value.get(key).is_some());
                fields.iter().all(|present| *present) || fields.iter().all(|present| !*present)
            }
            "operation-v1" => operation(value)?,
            "bounded-json" => true, // Checked once before any schema recursion.
            _ => false,
        };
        if !valid {
            return invalid();
        }
    }
    if let Some(object) = value.as_object() {
        for (key, child) in object {
            if let Some(shape) = schema["properties"].get(key) {
                check(shape, child, root)?;
            } else if schema["additionalProperties"].is_object() {
                check(&schema["additionalProperties"], child, root)?;
            }
            if schema["propertyNames"].is_object() {
                check(&schema["propertyNames"], &json!(key), root)?;
            }
        }
    }
    if let Some(items) = value.as_array() {
        if schema["items"].is_object() {
            for item in items {
                check(&schema["items"], item, root)?;
            }
        }
    }
    Ok(())
}
fn portable_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value
            .chars()
            .any(|character| (character as u32) < 32 || "\\:%?#<>\"|*".contains(character))
        && value.split('/').all(|part| {
            let device = part.split('.').next().unwrap_or("").to_ascii_lowercase();
            let numbered_device = device.len() == 4
                && (device.starts_with("com") || device.starts_with("lpt"))
                && matches!(device.as_bytes()[3], b'1'..=b'9');
            !matches!(part, "" | "." | "..")
                && !part.ends_with(['.', ' '])
                && !matches!(device.as_str(), "con" | "prn" | "aux" | "nul")
                && !numbered_device
        })
}
fn manifest_semantics(value: &Value) -> Result<bool, PackageError> {
    let files = array(&value["files"])?;
    let entries = value["entries"]
        .as_object()
        .ok_or(PackageError::ManifestInvalid)?;
    let total: u64 = files
        .iter()
        .map(|file| file["size"].as_u64().unwrap_or(u64::MAX))
        .try_fold(0u64, u64::checked_add)
        .ok_or(PackageError::ManifestInvalid)?;
    Ok(unique(
        array(&value["commands"])?
            .iter()
            .map(|item| item["id"].clone()),
    ) && total <= 67_108_864
        && unique(
            files
                .iter()
                .map(|file| json!(file["path"].as_str().unwrap_or("").to_lowercase())),
        )
        && unique(array(&value["targets"])?.iter().cloned())
        && !entries.is_empty()
        && unique(entries.values().cloned())
        && (array(&value["commands"])?.is_empty() || entries.contains_key("executor"))
        && value.get("services").is_none_or(|services| {
            services.as_array().unwrap().is_empty() != entries.contains_key("services")
        })
        && entries
            .values()
            .all(|entry| files.iter().any(|file| &file["path"] == entry)))
}
fn command_semantics(value: &Value) -> Result<bool, PackageError> {
    Ok((value["supportsColdStart"] != true
        || (value["headless"] == true && value["interaction"] != "required"))
        && value["inputSchema"]["type"] == "object"
        && unique(
            array(&value["permissions"])?
                .iter()
                .map(|item| item["capability"].clone()),
        ))
}
fn dependencies_semantics(value: &Value) -> Result<bool, PackageError> {
    for (kind, selector) in [("services", "service"), ("tools", "target")] {
        let mut keys = HashSet::new();
        let mut modes = std::collections::HashMap::new();
        for reference in array(&value[kind])? {
            let provider = (reference["publisher"].clone(), reference["id"].clone());
            let selected = reference.get(selector).is_some();
            if modes
                .insert(provider, selected)
                .is_some_and(|old| old != selected)
            {
                return Ok(false);
            }
            let key = if kind == "services" {
                json!([
                    reference["publisher"],
                    reference["id"],
                    reference["service"]
                ])
            } else {
                json!([
                    reference["publisher"],
                    reference["id"],
                    reference["target"]["platform"],
                    reference["target"]["arch"],
                    reference["buildFlavor"]
                ])
            };
            if !keys.insert(key.to_string()) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn operation(value: &Value) -> Result<bool, PackageError> {
    let contract = contract()?;
    if !contract.operation_validator.is_valid(value) {
        return Ok(false);
    }
    check(&contract.operation, value, &contract.operation)?;
    operation::validate_operation_schema(value)?;
    Ok(true)
}
