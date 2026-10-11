//! Inert signed dependency plans over currently trusted, byte-verified archives.
//! No installed inventory, tool admission, persisted locks, grants or execution.
use super::{
    manifest::{validate_bytes, CompatiblePluginArchive, ManifestCompatibility},
    trust::PackageTrustStore,
    PackageError,
};
use crate::{
    data::DataStore,
    dependencies::{
        DependencyCatalog, DependencyError, DependencyLock, PackageCandidate, PackagePin,
    },
};

const MAX_CANDIDATES: usize = 512;
const MAX_MANIFEST_BYTES: u64 = 16_777_216;

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
    candidate: PackagePin,
    lock: DependencyLock,
    root_digest: String,
    verified_at: u64,
}
impl VerifiedPluginDependencyPlan {
    pub fn candidate(&self) -> &PackagePin {
        &self.candidate
    }
    pub fn lock(&self) -> &DependencyLock {
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
    if request.providers.len() >= MAX_CANDIDATES {
        return Err(DependencyError::BudgetExceeded.into());
    }
    let archives: Vec<_> = std::iter::once(request.candidate)
        .chain(request.providers.iter().copied())
        .collect();
    let manifest_bytes: u64 = archives
        .iter()
        .map(|archive| archive.archive().descriptor().payload().manifest.size)
        .sum();
    if manifest_bytes > MAX_MANIFEST_BYTES {
        return Err(DependencyError::BudgetExceeded.into());
    }
    let receipts: Vec<_> = archives
        .iter()
        .map(|archive| archive.archive().descriptor())
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
    let candidate = PackagePin {
        publisher: payload.publisher.clone(),
        id: payload.id.clone(),
        version: payload.version.clone(),
        digest: descriptor.package_digest().into(),
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
    // Tools await their own complete admission/compatibility and G6 inventory.
    // An unverified ToolCandidate can never be supplied through this path.
    let catalog = DependencyCatalog::new(packages, vec![])?;
    let target = serde_json::from_value(
        serde_json::to_value(&payload.target)
            .map_err(|_| SignedDependencyError::Dependency(DependencyError::InvalidDeclaration))?,
    )
    .map_err(|_| DependencyError::InvalidDeclaration)?;
    let lock = catalog.resolve(&[candidate.identity()], &target)?.lock;
    if !lock.packages.contains(&candidate) || !lock.tools.is_empty() {
        return Err(DependencyError::ArtifactConflict.into());
    }
    Ok(VerifiedPluginDependencyPlan {
        candidate,
        lock,
        root_digest: root.payload_digest().into(),
        verified_at: request.now,
    })
}
