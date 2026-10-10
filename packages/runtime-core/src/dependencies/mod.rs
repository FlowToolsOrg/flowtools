//! Host-owned, bounded dependency planning. No IO, installation, activation or grants.
//! Catalogs come from the fixed build inventory or disposable test fixtures, never IPC.
mod resolver;
#[cfg(test)]
mod tests;

use crate::protocol::ErrorCode;
pub(crate) use resolver::valid_dependency_id;
pub use resolver::{DependencyCatalog, PackageCandidate, ToolCandidate};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DependencyError {
    InvalidDeclaration,
    InterfaceRequired,
    Missing,
    VersionConflict,
    DuplicateProvider,
    TargetMismatch,
    ArtifactConflict,
    Cycle,
    BudgetExceeded,
}
impl From<DependencyError> for ErrorCode {
    fn from(error: DependencyError) -> Self {
        match error {
            DependencyError::InvalidDeclaration | DependencyError::InterfaceRequired => {
                Self::DependencyInvalid
            }
            DependencyError::Missing => Self::DependencyMissing,
            DependencyError::VersionConflict => Self::DependencyConflict,
            DependencyError::DuplicateProvider => Self::DependencyDuplicateProvider,
            DependencyError::TargetMismatch => Self::DependencyPlatformMismatch,
            DependencyError::ArtifactConflict => Self::DependencyArtifactMismatch,
            DependencyError::Cycle => Self::DependencyCycle,
            DependencyError::BudgetExceeded => Self::DependencyBudgetExceeded,
        }
    }
}
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyIdentity {
    pub publisher: String,
    pub id: String,
}
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyTarget {
    pub platform: String,
    pub arch: String,
}
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePin {
    pub publisher: String,
    pub id: String,
    pub version: String,
    pub digest: String,
}
impl PackagePin {
    pub fn identity(&self) -> DependencyIdentity {
        DependencyIdentity {
            publisher: self.publisher.clone(),
            id: self.id.clone(),
        }
    }
}
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServicePin {
    pub consumer: DependencyIdentity,
    pub provider: PackagePin,
    pub service: String,
    pub version: String,
}
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolPin {
    pub consumer: DependencyIdentity,
    pub publisher: String,
    pub id: String,
    pub version: String,
    pub target: DependencyTarget,
    pub build_flavor: String,
    pub digest: String,
}
/// Service edges form the provider DAG. Tool pins are leaves, retaining the exact
/// consumer and artifact separately without activating any plugin provider.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyEdge {
    pub consumer: DependencyIdentity,
    pub provider: DependencyIdentity,
    pub service: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReverseDependency {
    pub provider: DependencyIdentity,
    pub consumers: Vec<DependencyIdentity>,
}
/// Value pins, never grants, executable paths, installation or activation receipts.
/// The digest binds every other field and excludes the digest field itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyLock {
    pub format_version: u16,
    pub digest: String,
    pub target: DependencyTarget,
    pub roots: Vec<DependencyIdentity>,
    pub packages: Vec<PackagePin>,
    pub services: Vec<ServicePin>,
    pub tools: Vec<ToolPin>,
    pub edges: Vec<DependencyEdge>,
    /// Providers before consumers; deterministic identity order breaks ties.
    pub topology: Vec<DependencyIdentity>,
    /// Direct service consumers, including providers without consumers.
    pub reverse_dependencies: Vec<ReverseDependency>,
}
impl DependencyLock {
    pub fn verified_digest(&self) -> Result<String, DependencyError> {
        let mut payload =
            serde_json::to_value(self).map_err(|_| DependencyError::InvalidDeclaration)?;
        payload
            .as_object_mut()
            .ok_or(DependencyError::InvalidDeclaration)?
            .remove("digest");
        Ok(crate::catalog::digest(&payload))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyPlan {
    pub format_version: u16,
    pub mode: String,
    pub lock: DependencyLock,
}
