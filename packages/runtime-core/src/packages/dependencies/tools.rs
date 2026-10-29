//! A separate signed-ZIP lock domain for plugin graphs with admitted tool leaves.
//! Plans remain inert and cannot produce an installed lock, lease or process.
use super::{
    resolve_signed, SignedArchivePin, SignedDependencyError, SignedDependencyRequest,
    SignedServicePin,
};
use crate::{
    data::DataStore,
    dependencies::{
        DependencyEdge, DependencyError, DependencyIdentity, DependencyTarget, ReverseDependency,
    },
    packages::{
        manifest::{CompatiblePluginArchive, ManifestCompatibility},
        tools::InertToolArchive,
        trust::PackageTrustStore,
        PackageTarget,
    },
};
use serde::{Deserialize, Serialize};

pub const SIGNED_TOOL_LOCK_PROFILE: &str = "signed-plugin-tool-archives-v1";

/// The consumer and exact original ZIP identity are distinct from G4 ToolPin.
/// Public diagnostic values cannot manufacture the opaque admission proof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedToolArchivePin {
    pub consumer: DependencyIdentity,
    pub publisher: String,
    pub id: String,
    pub version: String,
    pub target: PackageTarget,
    pub build_flavor: String,
    pub package_digest: String,
}
impl SignedToolArchivePin {
    pub(super) fn from_archive(archive: &InertToolArchive, consumer: DependencyIdentity) -> Self {
        let receipt = archive.archive().descriptor();
        let p = receipt.payload();
        Self {
            consumer,
            publisher: p.publisher.clone(),
            id: p.id.clone(),
            version: p.version.clone(),
            target: p.target.clone(),
            build_flavor: p.build_flavor.clone(),
            package_digest: receipt.package_digest().into(),
        }
    }
}
fn profile<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    if value != SIGNED_TOOL_LOCK_PROFILE {
        return Err(serde::de::Error::custom(
            "Unsupported signed tool lock profile",
        ));
    }
    Ok(value)
}
/// The profile is included in the digest. Neither G4 nor the older tool-free
/// signed lock can silently deserialize or consume this separate data contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedToolArchiveLock {
    pub format_version: u16,
    #[serde(deserialize_with = "profile")]
    pub profile: String,
    pub digest: String,
    pub target: DependencyTarget,
    pub roots: Vec<DependencyIdentity>,
    pub packages: Vec<SignedArchivePin>,
    pub services: Vec<SignedServicePin>,
    pub tools: Vec<SignedToolArchivePin>,
    pub edges: Vec<DependencyEdge>,
    pub topology: Vec<DependencyIdentity>,
    pub reverse_dependencies: Vec<ReverseDependency>,
}
impl SignedToolArchiveLock {
    /// Digest consistency only: this does not verify signatures, current trust,
    /// compatibility, installation or permission to run any of the value pins.
    pub fn verified_digest(&self) -> Result<String, DependencyError> {
        if self.format_version != 1 || self.profile != SIGNED_TOOL_LOCK_PROFILE {
            return Err(DependencyError::InvalidDeclaration);
        }
        let mut value =
            serde_json::to_value(self).map_err(|_| DependencyError::InvalidDeclaration)?;
        value
            .as_object_mut()
            .ok_or(DependencyError::InvalidDeclaration)?
            .remove("digest");
        Ok(crate::catalog::digest(&value))
    }
}
/// Build-owned T0 inputs. Tool leaves must already pass actual signed archive
/// and private release-review admission. No ToolCandidate or policy flag enters.
pub struct SignedToolDependencyRequest<'a> {
    pub candidate: &'a CompatiblePluginArchive,
    pub providers: &'a [&'a CompatiblePluginArchive],
    pub tools: &'a [&'a InertToolArchive],
    pub compatibility: &'a ManifestCompatibility,
}
/// Only current, fully checked planning constructs this in-process proof.
/// ```compile_fail
/// use flowtools_runtime_core::packages::dependencies::tools::VerifiedToolDependencyPlan;
/// let forged=VerifiedToolDependencyPlan {lock:todo!(),candidate:todo!(),root_digest:String::new(),verified_at:0};
/// ```
pub struct VerifiedToolDependencyPlan {
    candidate: SignedArchivePin,
    lock: SignedToolArchiveLock,
    root_digest: String,
    verified_at: u64,
}
impl VerifiedToolDependencyPlan {
    pub fn candidate(&self) -> &SignedArchivePin {
        &self.candidate
    }
    pub fn lock(&self) -> &SignedToolArchiveLock {
        &self.lock
    }
    pub fn root_digest(&self) -> &str {
        &self.root_digest
    }
    pub fn verified_at(&self) -> u64 {
        self.verified_at
    }
}
pub fn plan_signed_tools(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    request: SignedToolDependencyRequest<'_>,
    mut clock: impl FnMut() -> u64,
) -> Result<VerifiedToolDependencyPlan, SignedDependencyError> {
    let resolved = resolve_signed(
        SignedDependencyRequest {
            candidate: request.candidate,
            providers: request.providers,
            compatibility: request.compatibility,
            now: clock(),
        },
        trust,
        store,
        request.tools,
    )?;
    let graph = resolved.graph;
    let mut lock = SignedToolArchiveLock {
        format_version: 1,
        profile: SIGNED_TOOL_LOCK_PROFILE.into(),
        digest: String::new(),
        target: graph.target,
        roots: graph.roots,
        packages: graph.packages,
        services: graph.services,
        tools: resolved.tools,
        edges: graph.edges,
        topology: graph.topology,
        reverse_dependencies: graph.reverse_dependencies,
    };
    lock.digest = lock.verified_digest()?;
    // Every supplied candidate, including unused leaves, is rechecked against
    // the fresh T0 clock after solving. Never return a stale receipt after work.
    let receipts = std::iter::once(request.candidate)
        .chain(request.providers.iter().copied())
        .map(|archive| archive.archive().descriptor())
        .chain(request.tools.iter().map(|tool| tool.archive().descriptor()))
        .collect::<Vec<_>>();
    let now = clock();
    let root = trust.check_receipts(store, &receipts, &request.compatibility.target, now)?;
    Ok(VerifiedToolDependencyPlan {
        candidate: resolved.candidate,
        lock,
        root_digest: root.payload_digest().into(),
        verified_at: now,
    })
}
