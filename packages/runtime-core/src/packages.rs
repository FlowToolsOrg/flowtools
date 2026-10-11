//! Read-only P2.5a DSSE descriptor verification, with caller-owned trust/floors.
//! No production pins, filesystem/archive IO, installation, grants or execution.
//! A receipt proves descriptor/byte integrity, not full Manifest compatibility,
//! archive safety, installed provenance, isolation or an executable identity.

use base64::{engine::general_purpose, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

pub const PACKAGE_PAYLOAD_TYPE: &str = "application/vnd.flowtools.package.v1+json";
pub const ROOT_PAYLOAD_TYPE: &str = "application/vnd.flowtools.trust-root.v1+json";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_PAYLOAD_BYTES: usize = 1_048_576;
const MAX_ENVELOPE_BYTES: usize = 1_572_864;
const MAX_ARCHIVE_BYTES: u64 = 67_108_864;
const MAX_ROOT_LIFETIME: u64 = 31_536_000;
const MAX_PACKAGE_LIFETIME: u64 = 2_592_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageError {
    EnvelopeInvalid,
    PayloadInvalid,
    SignatureInvalid,
    TrustInvalid,
    TrustExpired,
    ClockRollback,
    RootRollback,
    ScopeDenied,
    KeyRevoked,
    PackageRevoked,
    IntegrityMismatch,
    TargetMismatch,
    VersionConflict,
    ReleaseRollback,
    ManifestMismatch,
}

impl PackageError {
    pub fn code(self) -> &'static str {
        match self {
            Self::EnvelopeInvalid => "PACKAGE_ENVELOPE_INVALID",
            Self::PayloadInvalid => "PACKAGE_PAYLOAD_INVALID",
            Self::SignatureInvalid => "PACKAGE_SIGNATURE_INVALID",
            Self::TrustInvalid => "PACKAGE_TRUST_INVALID",
            Self::TrustExpired => "PACKAGE_TRUST_EXPIRED",
            Self::ClockRollback => "PACKAGE_CLOCK_ROLLBACK",
            Self::RootRollback => "PACKAGE_ROOT_ROLLBACK",
            Self::ScopeDenied => "PACKAGE_SCOPE_DENIED",
            Self::KeyRevoked => "PACKAGE_KEY_REVOKED",
            Self::PackageRevoked => "PACKAGE_REVOKED",
            Self::IntegrityMismatch => "PACKAGE_INTEGRITY_MISMATCH",
            Self::TargetMismatch => "PACKAGE_TARGET_MISMATCH",
            Self::VersionConflict => "PACKAGE_VERSION_CONFLICT",
            Self::ReleaseRollback => "PACKAGE_RELEASE_ROLLBACK",
            Self::ManifestMismatch => "PACKAGE_MANIFEST_MISMATCH",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    Plugin,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PackagePlatform {
    Windows,
    Macos,
    Linux,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PackageArch {
    X64,
    Arm64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct PackageTarget {
    pub platform: PackagePlatform,
    pub arch: PackageArch,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlobDescriptor {
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveDescriptor {
    pub format: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PackageFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageSource {
    pub registry: String,
    pub artifact: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePayload {
    pub format_version: u64,
    pub kind: PackageKind,
    pub publisher: String,
    pub id: String,
    pub version: String,
    pub release_sequence: u64,
    pub target: PackageTarget,
    pub issued_at: u64,
    pub expires_at: u64,
    pub manifest: BlobDescriptor,
    pub archive: ArchiveDescriptor,
    pub files: Vec<PackageFile>,
    pub source: PackageSource,
    pub license: String,
    pub build_flavor: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicKey {
    pub keyid: String,
    pub public_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherScope {
    pub kind: PackageKind,
    pub id: String,
    pub targets: Vec<PackageTarget>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherProvenance {
    pub authority: String,
    pub subject: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherPolicy {
    pub publisher: String,
    pub keys: Vec<PublicKey>,
    pub threshold: usize,
    pub packages: Vec<PublisherScope>,
    pub provenance: PublisherProvenance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RootPayload {
    pub format_version: u64,
    pub version: u64,
    #[serde(deserialize_with = "required_optional_digest")]
    pub previous_root_sha256: Option<String>,
    pub issued_at: u64,
    pub expires_at: u64,
    pub root_keys: Vec<PublicKey>,
    pub root_threshold: usize,
    pub publishers: Vec<PublisherPolicy>,
    pub revoked_key_ids: Vec<String>,
    pub revoked_package_digests: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct VerificationClock {
    pub now: u64,
    /// T0's previously persisted highest accepted time; never plugin supplied.
    pub floor: u64,
}

#[derive(Clone, Debug)]
pub struct BootstrapPins {
    /// Build/release-owned out-of-band pins. No production pins exist here.
    pub keys: Vec<PublicKey>,
    pub threshold: usize,
}

#[derive(Clone, Debug)]
pub struct KnownVersionIdentity {
    pub kind: PackageKind,
    pub publisher: String,
    pub id: String,
    pub target: PackageTarget,
    pub version: String,
    pub package_digest: String,
    pub release_sequence: u64,
}

#[derive(Clone, Debug)]
pub struct AdmissionPolicy {
    pub target: PackageTarget,
    pub clock: VerificationClock,
    pub known_versions: Vec<KnownVersionIdentity>,
    /// T0 looks this up for the candidate's exact kind/publisher/id/target.
    pub highest_release_sequence: u64,
    /// A separately approved exact known-good digest, never a wire run flag.
    pub approved_rollback_digest: Option<String>,
}

#[derive(Clone, Debug)]
pub struct VerifiedRoot {
    payload: RootPayload,
    payload_digest: String,
}

impl VerifiedRoot {
    pub fn payload(&self) -> &RootPayload {
        &self.payload
    }

    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }
}

#[derive(Clone, Debug)]
pub struct VerifiedPackageDescriptor {
    payload: PackagePayload,
    package_digest: String,
    descriptor_digest: String,
    root_digest: String,
    verified_at: u64,
}

impl VerifiedPackageDescriptor {
    pub fn payload(&self) -> &PackagePayload {
        &self.payload
    }

    pub fn package_digest(&self) -> &str {
        &self.package_digest
    }

    /// Exact signed metadata bytes; expiry renewal changes this digest without
    /// changing the immutable ZIP artifact identity returned by package_digest.
    pub fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }

    pub fn root_digest(&self) -> &str {
        &self.root_digest
    }

    pub fn verified_at(&self) -> u64 {
        self.verified_at
    }
}

// DSSE extensions are ignored per the standard. Signed application payloads
// reject unknown fields; all recursive duplicate JSON keys are rejected.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    payload_type: String,
    payload: String,
    signatures: Vec<EnvelopeSignature>,
}

#[derive(Deserialize)]
struct EnvelopeSignature {
    #[serde(default)]
    keyid: String,
    sig: String,
}

struct DecodedEnvelope {
    payload: Vec<u8>,
    pae: Vec<u8>,
    signatures: Vec<DecodedSignature>,
}

struct DecodedSignature {
    keyid: String,
    signature: Signature,
}

pub fn bootstrap_root(
    envelope: &[u8],
    pins: &BootstrapPins,
    clock: VerificationClock,
) -> Result<VerifiedRoot, PackageError> {
    validate_clock(clock)?;
    validate_key_set(&pins.keys, pins.threshold, 2)?;
    let decoded = decode_envelope(envelope, ROOT_PAYLOAD_TYPE)?;
    let root: RootPayload = parse_payload(&decoded.payload)?;
    validate_root(&root)?;
    if root.version != 1 || root.previous_root_sha256.is_some() {
        return Err(PackageError::RootRollback);
    }
    validate_time(root.issued_at, root.expires_at, clock, MAX_ROOT_LIFETIME)?;
    verify_threshold(&decoded, &pins.keys, pins.threshold, &root.revoked_key_ids)?;
    verify_threshold(
        &decoded,
        &root.root_keys,
        root.root_threshold,
        &root.revoked_key_ids,
    )?;
    Ok(VerifiedRoot {
        payload: root,
        payload_digest: hash(&decoded.payload),
    })
}

pub fn rotate_root(
    previous: &VerifiedRoot,
    envelope: &[u8],
    clock: VerificationClock,
) -> Result<VerifiedRoot, PackageError> {
    validate_clock(clock)?;
    let decoded = decode_envelope(envelope, ROOT_PAYLOAD_TYPE)?;
    let root: RootPayload = parse_payload(&decoded.payload)?;
    validate_root(&root)?;
    if previous.payload.version.checked_add(1) != Some(root.version)
        || root.previous_root_sha256.as_deref() != Some(previous.payload_digest())
        || root.issued_at < previous.payload.issued_at
        || !previous
            .payload
            .revoked_key_ids
            .iter()
            .all(|key| root.revoked_key_ids.contains(key))
        || !previous
            .payload
            .revoked_package_digests
            .iter()
            .all(|digest| root.revoked_package_digests.contains(digest))
    {
        return Err(PackageError::RootRollback);
    }
    // An expired root may authenticate recovery to a fresh successor, but it
    // can never admit a package. Old keys remain checked for old revocations.
    validate_time(root.issued_at, root.expires_at, clock, MAX_ROOT_LIFETIME)?;
    verify_threshold(
        &decoded,
        &previous.payload.root_keys,
        previous.payload.root_threshold,
        &previous.payload.revoked_key_ids,
    )?;
    verify_threshold(
        &decoded,
        &root.root_keys,
        root.root_threshold,
        &root.revoked_key_ids,
    )?;
    Ok(VerifiedRoot {
        payload: root,
        payload_digest: hash(&decoded.payload),
    })
}

pub fn verify_package(
    root: &VerifiedRoot,
    envelope: &[u8],
    manifest_bytes: &[u8],
    archive_bytes: &[u8],
    policy: &AdmissionPolicy,
) -> Result<VerifiedPackageDescriptor, PackageError> {
    validate_clock(policy.clock)?;
    validate_time(
        root.payload.issued_at,
        root.payload.expires_at,
        policy.clock,
        MAX_ROOT_LIFETIME,
    )?;
    let decoded = decode_envelope(envelope, PACKAGE_PAYLOAD_TYPE)?;
    let package: PackagePayload = parse_payload(&decoded.payload)?;
    validate_package(&package)?;
    validate_time(
        package.issued_at,
        package.expires_at,
        policy.clock,
        MAX_PACKAGE_LIFETIME,
    )?;
    if package.target != policy.target {
        return Err(PackageError::TargetMismatch);
    }
    let publisher = root
        .payload
        .publishers
        .iter()
        .find(|publisher| publisher.publisher == package.publisher)
        .ok_or(PackageError::ScopeDenied)?;
    if !publisher.packages.iter().any(|scope| {
        scope.kind == package.kind
            && scope.id == package.id
            && scope.targets.contains(&package.target)
    }) {
        return Err(PackageError::ScopeDenied);
    }
    verify_threshold(
        &decoded,
        &publisher.keys,
        publisher.threshold,
        &root.payload.revoked_key_ids,
    )?;
    let descriptor_digest = hash(&decoded.payload);
    let package_digest = package.archive.sha256.clone();
    if root
        .payload
        .revoked_package_digests
        .contains(&package_digest)
    {
        return Err(PackageError::PackageRevoked);
    }
    verify_blob(manifest_bytes, &package.manifest, MAX_PAYLOAD_BYTES as u64)?;
    verify_blob(
        archive_bytes,
        &BlobDescriptor {
            sha256: package.archive.sha256.clone(),
            size: package.archive.size,
        },
        MAX_ARCHIVE_BYTES,
    )?;
    validate_manifest_header(manifest_bytes, &package)?;
    validate_floors(&package, &package_digest, policy)?;
    Ok(VerifiedPackageDescriptor {
        payload: package,
        package_digest,
        descriptor_digest,
        root_digest: root.payload_digest.clone(),
        verified_at: policy.clock.now,
    })
}

fn validate_floors(
    package: &PackagePayload,
    digest: &str,
    policy: &AdmissionPolicy,
) -> Result<(), PackageError> {
    if policy.highest_release_sequence > MAX_SAFE_INTEGER
        || policy
            .approved_rollback_digest
            .as_ref()
            .is_some_and(|digest| !valid_digest(digest))
    {
        return Err(PackageError::TrustInvalid);
    }
    let mut known_good = false;
    let mut highest = policy.highest_release_sequence;
    for known in &policy.known_versions {
        if !valid_id(&known.publisher)
            || !valid_id(&known.id)
            || !valid_version(&known.version)
            || !valid_digest(&known.package_digest)
            || known.release_sequence == 0
            || known.release_sequence > MAX_SAFE_INTEGER
        {
            return Err(PackageError::TrustInvalid);
        }
        if known.kind != package.kind
            || known.publisher != package.publisher
            || known.id != package.id
            || known.target != package.target
        {
            continue;
        }
        highest = highest.max(known.release_sequence);
        if known.version == package.version && known.release_sequence != package.release_sequence {
            return Err(PackageError::VersionConflict);
        }
        if (known.version == package.version || known.release_sequence == package.release_sequence)
            && known.package_digest != digest
        {
            return Err(PackageError::VersionConflict);
        }
        known_good |= known.package_digest == digest;
    }
    if package.release_sequence < highest
        && !(known_good && policy.approved_rollback_digest.as_deref() == Some(digest))
    {
        return Err(PackageError::ReleaseRollback);
    }
    Ok(())
}

fn decode_envelope(bytes: &[u8], expected_type: &str) -> Result<DecodedEnvelope, PackageError> {
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err(PackageError::EnvelopeInvalid);
    }
    let value = strict_json(bytes).map_err(|_| PackageError::EnvelopeInvalid)?;
    let envelope: Envelope =
        serde_json::from_value(value).map_err(|_| PackageError::EnvelopeInvalid)?;
    if envelope.payload_type != expected_type
        || envelope.signatures.is_empty()
        || envelope.signatures.len() > 16
        || envelope
            .signatures
            .iter()
            .any(|signature| signature.keyid.len() > 128 || signature.sig.len() > 128)
    {
        return Err(PackageError::EnvelopeInvalid);
    }
    let payload = decode_base64(&envelope.payload)?;
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(PackageError::PayloadInvalid);
    }
    let signatures = envelope
        .signatures
        .into_iter()
        .map(|signature| {
            let bytes = decode_base64(&signature.sig)?;
            Ok(DecodedSignature {
                keyid: signature.keyid,
                signature: Signature::from_slice(&bytes)
                    .map_err(|_| PackageError::EnvelopeInvalid)?,
            })
        })
        .collect::<Result<Vec<_>, PackageError>>()?;
    let mut pae = format!(
        "DSSEv1 {} {} {} ",
        expected_type.len(),
        expected_type,
        payload.len()
    )
    .into_bytes();
    pae.extend_from_slice(&payload);
    Ok(DecodedEnvelope {
        payload,
        pae,
        signatures,
    })
}

fn decode_base64(value: &str) -> Result<Vec<u8>, PackageError> {
    [
        &general_purpose::STANDARD,
        &general_purpose::STANDARD_NO_PAD,
        &general_purpose::URL_SAFE,
        &general_purpose::URL_SAFE_NO_PAD,
    ]
    .into_iter()
    .find_map(|engine| engine.decode(value).ok())
    .ok_or(PackageError::EnvelopeInvalid)
}

fn parse_payload<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, PackageError> {
    serde_json::from_value(strict_json(bytes).map_err(|_| PackageError::PayloadInvalid)?)
        .map_err(|_| PackageError::PayloadInvalid)
}

fn verify_threshold(
    envelope: &DecodedEnvelope,
    keys: &[PublicKey],
    threshold: usize,
    revoked: &[String],
) -> Result<(), PackageError> {
    let mut accepted = HashSet::new();
    let mut revoked_valid = false;
    for signature in &envelope.signatures {
        for key in keys {
            // Hint may only narrow candidates. All authority comes from the
            // actual verified public key and the authenticated role policy.
            if !signature.keyid.is_empty() && signature.keyid != key.keyid {
                continue;
            }
            let verifying_key = public_key(key)?;
            if verifying_key
                .verify_strict(&envelope.pae, &signature.signature)
                .is_err()
            {
                continue;
            }
            if revoked.contains(&key.keyid) {
                revoked_valid = true;
            } else {
                accepted.insert(verifying_key.to_bytes());
            }
        }
    }
    if accepted.len() >= threshold {
        Ok(())
    } else if revoked_valid {
        Err(PackageError::KeyRevoked)
    } else {
        Err(PackageError::SignatureInvalid)
    }
}

fn public_key(key: &PublicKey) -> Result<VerifyingKey, PackageError> {
    let bytes: [u8; 32] = decode_base64(&key.public_key)
        .map_err(|_| PackageError::TrustInvalid)?
        .try_into()
        .map_err(|_| PackageError::TrustInvalid)?;
    let parsed = VerifyingKey::from_bytes(&bytes).map_err(|_| PackageError::TrustInvalid)?;
    if parsed.is_weak()
        || parsed.to_edwards().compress().to_bytes() != bytes
        || key.keyid != hash(&bytes)
    {
        return Err(PackageError::TrustInvalid);
    }
    Ok(parsed)
}

fn validate_key_set(
    keys: &[PublicKey],
    threshold: usize,
    minimum: usize,
) -> Result<(), PackageError> {
    if keys.len() > 16 || threshold < minimum || threshold > keys.len() {
        return Err(PackageError::TrustInvalid);
    }
    let mut bytes = HashSet::new();
    for key in keys {
        if !bytes.insert(public_key(key)?.to_bytes()) {
            return Err(PackageError::TrustInvalid);
        }
    }
    Ok(())
}

fn validate_root(root: &RootPayload) -> Result<(), PackageError> {
    if root.format_version != 1
        || root.version == 0
        || root.version > MAX_SAFE_INTEGER
        || root
            .previous_root_sha256
            .as_ref()
            .is_some_and(|digest| !valid_digest(digest))
        || root.publishers.len() > 128
        || !unique_digests(&root.revoked_key_ids)
        || !unique_digests(&root.revoked_package_digests)
    {
        return Err(PackageError::TrustInvalid);
    }
    validate_key_set(&root.root_keys, root.root_threshold, 2)?;
    let mut identities = HashSet::new();
    let mut role_keys: HashSet<_> = root.root_keys.iter().map(|key| &key.keyid).collect();
    if root
        .root_keys
        .iter()
        .filter(|key| !root.revoked_key_ids.contains(&key.keyid))
        .count()
        < root.root_threshold
    {
        return Err(PackageError::TrustInvalid);
    }
    for publisher in &root.publishers {
        if !valid_id(&publisher.publisher)
            || !identities.insert(&publisher.publisher)
            || publisher.packages.is_empty()
            || publisher.packages.len() > 128
            || !valid_id(&publisher.provenance.authority)
            || !valid_id(&publisher.provenance.subject)
        {
            return Err(PackageError::TrustInvalid);
        }
        validate_key_set(&publisher.keys, publisher.threshold, 1)?;
        for key in &publisher.keys {
            if !role_keys.insert(&key.keyid) {
                return Err(PackageError::TrustInvalid);
            }
        }
        let mut scopes = HashSet::new();
        for scope in &publisher.packages {
            if !valid_id(&scope.id)
                || !scopes.insert((&scope.kind, &scope.id))
                || scope.targets.is_empty()
                || scope.targets.len() > 6
                || scope.targets.iter().collect::<HashSet<_>>().len() != scope.targets.len()
            {
                return Err(PackageError::TrustInvalid);
            }
        }
    }
    Ok(())
}

fn validate_package(package: &PackagePayload) -> Result<(), PackageError> {
    if package.format_version != 1
        || !valid_id(&package.publisher)
        || !valid_id(&package.id)
        || !valid_version(&package.version)
        || package.release_sequence == 0
        || package.release_sequence > MAX_SAFE_INTEGER
        || !valid_digest(&package.manifest.sha256)
        || package.manifest.size == 0
        || package.manifest.size > MAX_PAYLOAD_BYTES as u64
        || package.archive.format != "zip"
        || !valid_digest(&package.archive.sha256)
        || package.archive.size == 0
        || package.archive.size > MAX_ARCHIVE_BYTES
        || package.files.is_empty()
        || package.files.len() > 1024
        || !valid_id(&package.source.registry)
        || !valid_id(&package.source.artifact)
        || !valid_id(&package.build_flavor)
        || package.license.is_empty()
        || package.license.len() > 128
        || !package.license.as_bytes()[0].is_ascii_alphanumeric()
        || !package
            .license
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-'))
    {
        return Err(PackageError::PayloadInvalid);
    }
    let mut paths = HashSet::new();
    let mut total = 0_u64;
    for file in &package.files {
        total = total
            .checked_add(file.size)
            .ok_or(PackageError::PayloadInvalid)?;
        if !valid_path(&file.path)
            || file.path == "manifest.json"
            || file.path.starts_with("manifest.json/")
            || !paths.insert(file.path.to_lowercase())
            || !valid_digest(&file.sha256)
            || file.size > MAX_ARCHIVE_BYTES
            || total > MAX_ARCHIVE_BYTES
        {
            return Err(PackageError::PayloadInvalid);
        }
    }
    for file in &package.files {
        let mut ancestor = file.path.as_str();
        while let Some((parent, _)) = ancestor.rsplit_once('/') {
            if paths.contains(&parent.to_lowercase()) {
                return Err(PackageError::PayloadInvalid);
            }
            ancestor = parent;
        }
    }
    Ok(())
}

fn validate_manifest_header(bytes: &[u8], package: &PackagePayload) -> Result<(), PackageError> {
    let manifest = strict_json(bytes).map_err(|_| PackageError::ManifestMismatch)?;
    if manifest["publisher"].as_str() != Some(&package.publisher)
        || manifest["id"].as_str() != Some(&package.id)
        || manifest["version"].as_str() != Some(&package.version)
    {
        return Err(PackageError::ManifestMismatch);
    }
    let files: Vec<PackageFile> = serde_json::from_value(manifest["files"].clone())
        .map_err(|_| PackageError::ManifestMismatch)?;
    if files != package.files {
        return Err(PackageError::ManifestMismatch);
    }
    let target =
        serde_json::to_value(&package.target).map_err(|_| PackageError::ManifestMismatch)?;
    let target_matches = match package.kind {
        PackageKind::Plugin => manifest["targets"]
            .as_array()
            .is_some_and(|targets| targets.contains(&target)),
        PackageKind::Tool => manifest["target"] == target,
    };
    if !target_matches {
        return Err(PackageError::ManifestMismatch);
    }
    Ok(())
}

fn validate_clock(clock: VerificationClock) -> Result<(), PackageError> {
    if clock.now > MAX_SAFE_INTEGER || clock.floor > MAX_SAFE_INTEGER || clock.now < clock.floor {
        Err(PackageError::ClockRollback)
    } else {
        Ok(())
    }
}

fn validate_time(
    issued_at: u64,
    expires_at: u64,
    clock: VerificationClock,
    max_lifetime: u64,
) -> Result<(), PackageError> {
    if issued_at > MAX_SAFE_INTEGER
        || expires_at > MAX_SAFE_INTEGER
        || expires_at <= issued_at
        || expires_at - issued_at > max_lifetime
    {
        Err(PackageError::PayloadInvalid)
    } else if issued_at > clock.now || expires_at <= clock.now {
        Err(PackageError::TrustExpired)
    } else {
        Ok(())
    }
}

fn verify_blob(bytes: &[u8], blob: &BlobDescriptor, budget: u64) -> Result<(), PackageError> {
    if blob.size > budget || bytes.len() as u64 != blob.size || hash(bytes) != blob.sha256 {
        Err(PackageError::IntegrityMismatch)
    } else {
        Ok(())
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn valid_version(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    value.len() <= 128
        && parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && (part.len() == 1 || !part.starts_with('0'))
                && part.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unique_digests(values: &[String]) -> bool {
    values.len() <= 4096
        && values.iter().all(|value| valid_digest(value))
        && values.iter().collect::<HashSet<_>>().len() == values.len()
}

fn valid_path(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 240
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-' | b'/')
        })
    {
        return false;
    }
    value.split('/').all(|part| {
        let stem = part.split('.').next().unwrap_or("").to_ascii_lowercase();
        !part.is_empty()
            && part != "."
            && part != ".."
            && !part.ends_with(['.', ' '])
            && !matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
            && !((stem.starts_with("com") || stem.starts_with("lpt"))
                && stem.len() == 4
                && stem.as_bytes()[3].is_ascii_digit())
    })
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn required_optional_digest<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

/// serde_json's default map accepts last-wins duplicate fields. Reject them
/// recursively before typed parsing, keeping the verified bytes authoritative.
fn strict_json(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    struct Unique(Value);
    impl<'de> Deserialize<'de> for Unique {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Unique;
                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("JSON without duplicate object keys")
                }
                fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Unique, E> {
                    Ok(Unique(Value::Bool(value)))
                }
                fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Unique, E> {
                    Ok(Unique(value.into()))
                }
                fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Unique, E> {
                    Ok(Unique(value.into()))
                }
                fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Unique, E> {
                    serde_json::Number::from_f64(value)
                        .map(|number| Unique(Value::Number(number)))
                        .ok_or_else(|| E::custom("Nonfinite JSON number"))
                }
                fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Unique, E> {
                    Ok(Unique(Value::String(value.to_owned())))
                }
                fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Unique, E> {
                    Ok(Unique(Value::String(value)))
                }
                fn visit_none<E: serde::de::Error>(self) -> Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
                fn visit_unit<E: serde::de::Error>(self) -> Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut access: A,
                ) -> Result<Unique, A::Error> {
                    let mut values = Vec::new();
                    while let Some(Unique(value)) = access.next_element()? {
                        values.push(value);
                    }
                    Ok(Unique(Value::Array(values)))
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut access: A,
                ) -> Result<Unique, A::Error> {
                    let mut values = BTreeMap::new();
                    while let Some((key, Unique(value))) = access.next_entry::<String, Unique>()? {
                        if values.insert(key, value).is_some() {
                            return Err(serde::de::Error::custom("Duplicate JSON object key"));
                        }
                    }
                    Ok(Unique(Value::Object(values.into_iter().collect())))
                }
            }
            deserializer.deserialize_any(Visitor)
        }
    }
    serde_json::from_slice::<Unique>(bytes).map(|unique| unique.0)
}

#[cfg(test)]
mod tests;
