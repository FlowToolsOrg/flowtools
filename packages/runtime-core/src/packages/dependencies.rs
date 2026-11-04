//! Inert signed dependency plans over currently trusted, byte-verified archives.
//! No installed inventory, persisted locks, grants or execution. The separate
//! tools profile consumes only opaque, already admitted tool archives.
pub mod tools;
use super::{
    manifest::{validate_bytes, CompatiblePluginArchive, ManifestCompatibility},
    trust::PackageTrustStore,
    PackageError, PackageTarget,
};
use crate::{
    data::DataStore,
    dependencies::{
        DependencyCatalog, DependencyEdge, DependencyError, DependencyIdentity, DependencyTarget,
        PackageCandidate, PackagePin, ReverseDependency, ToolCandidate, ToolPin,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_CANDIDATES: usize = 512;
const MAX_MANIFEST_BYTES: u64 = 16_777_216;
pub const SIGNED_ARCHIVE_LOCK_PROFILE: &str = "signed-plugin-archives-v1";

/// Exact signed ZIP bytes and admitted target. This is deliberately distinct
/// from the G4 semantic Manifest PackagePin and is never an execution grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedArchivePin {
    pub publisher: String,
    pub id: String,
    pub version: String,
    pub target: PackageTarget,
    pub package_digest: String,
}
impl SignedArchivePin {
    pub fn identity(&self) -> DependencyIdentity {
        DependencyIdentity {
            publisher: self.publisher.clone(),
            id: self.id.clone(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedServicePin {
    pub consumer: DependencyIdentity,
    pub provider: SignedArchivePin,
    pub service: String,
    pub version: String,
}
fn signed_profile<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let profile = String::deserialize(deserializer)?;
    if profile != SIGNED_ARCHIVE_LOCK_PROFILE {
        return Err(serde::de::Error::custom(
            "Unsupported signed archive lock profile",
        ));
    }
    Ok(profile)
}
/// The profile is part of the digest domain. G4 DependencyLock v1 cannot be
/// deserialized or silently reinterpreted as this signed archive lock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedArchiveLock {
    pub format_version: u16,
    #[serde(deserialize_with = "signed_profile")]
    pub profile: String,
    pub digest: String,
    pub target: DependencyTarget,
    pub roots: Vec<DependencyIdentity>,
    pub packages: Vec<SignedArchivePin>,
    pub services: Vec<SignedServicePin>,
    /// Legacy tool-free profile: this field stays empty. The separate tools
    /// profile has its own admitted ZIP pins and cannot be substituted here.
    pub tools: Vec<ToolPin>,
    pub edges: Vec<DependencyEdge>,
    pub topology: Vec<DependencyIdentity>,
    pub reverse_dependencies: Vec<ReverseDependency>,
}
impl SignedArchiveLock {
    pub fn verified_digest(&self) -> Result<String, DependencyError> {
        if self.format_version != 1
            || self.profile != SIGNED_ARCHIVE_LOCK_PROFILE
            || !self.tools.is_empty()
        {
            return Err(DependencyError::InvalidDeclaration);
        }
        let mut payload =
            serde_json::to_value(self).map_err(|_| DependencyError::InvalidDeclaration)?;
        payload
            .as_object_mut()
            .ok_or(DependencyError::InvalidDeclaration)?
            .remove("digest");
        Ok(crate::catalog::digest(&payload))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SignedDependencyError {
    Package(PackageError),
    Dependency(DependencyError),
}
impl From<PackageError> for SignedDependencyError {
    fn from(value: PackageError) -> Self {
        Self::Package(value)
    }
}
impl From<DependencyError> for SignedDependencyError {
    fn from(value: DependencyError) -> Self {
        Self::Dependency(value)
    }
}

/// T0-borrowed inputs. Every provider is a fully verified inert candidate, not
/// an installed record or client-selectable metadata/digest/tool list.
pub struct SignedDependencyRequest<'a> {
    pub candidate: &'a CompatiblePluginArchive,
    pub providers: &'a [&'a CompatiblePluginArchive],
    pub compatibility: &'a ManifestCompatibility,
    pub now: u64,
}

/// Only this module constructs the proof. Value pins are inspectable, but do
/// not create an installation, health result, lease, grant or runner identity.
pub struct VerifiedPluginDependencyPlan {
    candidate: SignedArchivePin,
    lock: SignedArchiveLock,
    root_digest: String,
    verified_at: u64,
}
impl VerifiedPluginDependencyPlan {
    pub fn candidate(&self) -> &SignedArchivePin {
        &self.candidate
    }
    pub fn lock(&self) -> &SignedArchiveLock {
        &self.lock
    }
    pub fn root_digest(&self) -> &str {
        &self.root_digest
    }
    pub fn verified_at(&self) -> u64 {
        self.verified_at
    }
}

pub fn plan_signed_plugin(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    request: SignedDependencyRequest<'_>,
) -> Result<VerifiedPluginDependencyPlan, SignedDependencyError> {
    let resolved = resolve_signed(request, trust, store, &[])?;
    if !resolved.tools.is_empty() {
        return Err(DependencyError::ArtifactConflict.into());
    }
    Ok(VerifiedPluginDependencyPlan {
        candidate: resolved.candidate,
        lock: resolved.graph,
        root_digest: resolved.root_digest,
        verified_at: resolved.verified_at,
    })
}
struct ResolvedSigned {
    candidate: SignedArchivePin,
    graph: SignedArchiveLock,
    root_digest: String,
    verified_at: u64,
    tools: Vec<tools::SignedToolArchivePin>,
}
fn resolve_signed(
    request: SignedDependencyRequest<'_>,
    trust: &PackageTrustStore,
    store: &mut DataStore,
    tool_archives: &[&super::tools::InertToolArchive],
) -> Result<ResolvedSigned, SignedDependencyError> {
    if request
        .providers
        .len()
        .checked_add(tool_archives.len())
        .is_none_or(|n| n >= MAX_CANDIDATES)
    {
        return Err(DependencyError::BudgetExceeded.into());
    }
    let archives: Vec<_> = std::iter::once(request.candidate)
        .chain(request.providers.iter().copied())
        .collect();
    let manifest_bytes: u64 = archives
        .iter()
        .map(|archive| archive.archive().descriptor().payload().manifest.size)
        .chain(
            tool_archives
                .iter()
                .map(|tool| tool.archive().descriptor().payload().manifest.size),
        )
        .sum();
    if manifest_bytes > MAX_MANIFEST_BYTES {
        return Err(DependencyError::BudgetExceeded.into());
    }
    let receipts: Vec<_> = archives
        .iter()
        .map(|archive| archive.archive().descriptor())
        .chain(tool_archives.iter().map(|tool| tool.archive().descriptor()))
        .collect();
    let root =
        trust.check_receipts(store, &receipts, &request.compatibility.target, request.now)?;
    for archive in &archives {
        validate_bytes(
            &archive.archive().files()["manifest.json"],
            request.compatibility,
        )?;
    }
    let descriptor = request.candidate.archive().descriptor();
    let payload = descriptor.payload();
    let target: DependencyTarget = serde_json::from_value(
        serde_json::to_value(&payload.target).map_err(|_| DependencyError::InvalidDeclaration)?,
    )
    .map_err(|_| DependencyError::InvalidDeclaration)?;
    let candidate = SignedArchivePin {
        publisher: payload.publisher.clone(),
        id: payload.id.clone(),
        version: payload.version.clone(),
        target: payload.target.clone(),
        package_digest: descriptor.package_digest().into(),
    };
    // Do not allow the solver to choose another version of the install candidate.
    if request.providers.iter().any(|archive| {
        let provider = archive.archive().descriptor().payload();
        provider.publisher == candidate.publisher && provider.id == candidate.id
    }) {
        return Err(DependencyError::DuplicateProvider.into());
    }
    let packages = archives
        .iter()
        .map(|archive| PackageCandidate::from_verified_archive(archive))
        .collect::<Result<Vec<_>, _>>()?;
    // This temporary solver projection never escapes as a G4 lock. Its tool
    // candidates come only from opaque admission and pin original signed ZIPs.
    // The legacy entry supplies none and retains its previous refusal behavior.
    let tool_candidates = tool_archives
        .iter()
        .map(|tool| {
            let receipt = tool.archive().descriptor();
            let payload = receipt.payload();
            Ok(ToolCandidate {
                publisher: payload.publisher.clone(),
                id: payload.id.clone(),
                version: payload.version.clone(),
                target: serde_json::from_value(
                    serde_json::to_value(&payload.target)
                        .map_err(|_| DependencyError::InvalidDeclaration)?,
                )
                .map_err(|_| DependencyError::InvalidDeclaration)?,
                build_flavor: payload.build_flavor.clone(),
                digest: receipt.package_digest().into(),
            })
        })
        .collect::<Result<Vec<_>, DependencyError>>()?;
    let catalog = DependencyCatalog::new(packages, tool_candidates)?;
    let semantic = catalog.resolve(&[candidate.identity()], &target)?.lock;
    let tool_pins = semantic
        .tools
        .iter()
        .map(|pin| {
            let archive = tool_archives
                .iter()
                .find(|archive| {
                    let receipt = archive.archive().descriptor();
                    let payload = receipt.payload();
                    payload.publisher == pin.publisher
                        && payload.id == pin.id
                        && payload.version == pin.version
                        && payload.build_flavor == pin.build_flavor
                        && receipt.package_digest() == pin.digest
                })
                .ok_or(DependencyError::ArtifactConflict)?;
            Ok(tools::SignedToolArchivePin::from_archive(
                archive,
                pin.consumer.clone(),
            ))
        })
        .collect::<Result<Vec<_>, DependencyError>>()?;
    let mut exact = BTreeMap::new();
    for archive in &archives {
        let descriptor = archive.archive().descriptor();
        let payload = descriptor.payload();
        let key = PackagePin {
            publisher: payload.publisher.clone(),
            id: payload.id.clone(),
            version: payload.version.clone(),
            digest: crate::catalog::digest(archive.manifest()),
        };
        let pin = SignedArchivePin {
            publisher: payload.publisher.clone(),
            id: payload.id.clone(),
            version: payload.version.clone(),
            target: payload.target.clone(),
            package_digest: descriptor.package_digest().into(),
        };
        if exact.insert(key, pin).is_some() {
            return Err(DependencyError::DuplicateProvider.into());
        }
    }
    let packages = semantic
        .packages
        .iter()
        .map(|pin| {
            exact
                .get(pin)
                .cloned()
                .ok_or(DependencyError::ArtifactConflict)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let services = semantic
        .services
        .into_iter()
        .map(|pin| {
            Ok(SignedServicePin {
                consumer: pin.consumer,
                provider: exact
                    .get(&pin.provider)
                    .cloned()
                    .ok_or(DependencyError::ArtifactConflict)?,
                service: pin.service,
                version: pin.version,
            })
        })
        .collect::<Result<Vec<_>, DependencyError>>()?;
    let mut lock = SignedArchiveLock {
        format_version: 1,
        profile: SIGNED_ARCHIVE_LOCK_PROFILE.into(),
        digest: String::new(),
        target: semantic.target,
        roots: semantic.roots,
        packages,
        services,
        tools: vec![],
        edges: semantic.edges,
        topology: semantic.topology,
        reverse_dependencies: semantic.reverse_dependencies,
    };
    if !lock.packages.contains(&candidate) {
        return Err(DependencyError::ArtifactConflict.into());
    }
    lock.digest = lock.verified_digest()?;
    Ok(ResolvedSigned {
        tools: tool_pins,
        candidate,
        graph: lock,
        root_digest: root.payload_digest().into(),
        verified_at: request.now,
    })
}
