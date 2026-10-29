//! G6: strict inert tool-package admission and pure fixed media recipes.
//! No installed registry, extraction, executable identity, lease or execution.
use super::{
    archive::{validate_archive, ValidatedArchive},
    manifest::ManifestCompatibility,
    strict_json,
    trust::{DescriptorVerification, PackageTrustStore},
    PackageError, PackageFile, PackageKind, PackageSource, PackageTarget,
};
use crate::data::DataStore;
use serde::Deserialize;
use std::collections::BTreeSet;

pub mod media;

const MAX_TOOL_MANIFEST_BYTES: usize = 65_536;
const MAX_TOOL_FILES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolAdmissionError {
    Package(PackageError),
    ManifestInvalid,
    ManifestBudgetExceeded,
    PolicyDenied,
}
impl ToolAdmissionError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Package(error) => error.code(),
            Self::ManifestInvalid => "TOOL_MANIFEST_INVALID",
            Self::ManifestBudgetExceeded => "TOOL_MANIFEST_BUDGET_EXCEEDED",
            Self::PolicyDenied => "TOOL_POLICY_DENIED",
        }
    }
}
impl From<PackageError> for ToolAdmissionError {
    fn from(error: PackageError) -> Self {
        Self::Package(error)
    }
}

/// Original signed bytes assembled by T0. No client-selected target, reviewed
/// policy, executable path, argv, validation flag or trust override.
pub struct ToolArchiveInput<'a> {
    pub envelope: &'a [u8],
    pub manifest: &'a [u8],
    pub archive: &'a [u8],
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ToolProgram {
    Ffmpeg,
    Ffprobe,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgramDeclaration {
    role: ToolProgram,
    path: String,
}

/// Runtime owns this tool format. It is separate from SDK Plugin Manifest v1
/// and from the existing G4 ToolPin; neither contract is changed or reinterpreted.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ToolManifest {
    format_version: u16,
    kind: PackageKind,
    publisher: String,
    id: String,
    version: String,
    target: PackageTarget,
    build_flavor: String,
    source: PackageSource,
    license: String,
    files: Vec<PackageFile>,
    programs: Vec<ProgramDeclaration>,
    resources: Vec<String>,
}

/// A release-owned review record. No constructor, Serde or caller-policy API.
/// Exact source/license/flavor/layout approval is independent of publisher trust.
struct ReviewedToolProfile {
    publisher: &'static str,
    id: &'static str,
    registry: &'static str,
    artifact: &'static str,
    license: &'static str,
    build_flavor: &'static str,
    target: PackageTarget,
    programs: &'static [(ToolProgram, &'static str)],
    resources: &'static [&'static str],
}

// There are no reviewed production tool builds or production pins. Adding a
// profile requires release/security/license review and separate platform gates.
// A package signature, source token or claimed license cannot populate this list.
const REVIEWED_TOOLS: &[ReviewedToolProfile] = &[];

/// Only admission constructs this proof. It holds original inert bytes and
/// diagnostic roles, never a disk path or executable capability. Re-admission
/// against current trust is required before any future installation/use.
///
/// ```compile_fail
/// use flowtools_runtime_core::packages::tools::InertToolArchive;
/// let forged = InertToolArchive { archive: todo!(), programs: vec![] };
/// ```
pub struct InertToolArchive {
    archive: ValidatedArchive,
    programs: Vec<ToolProgram>,
}
impl InertToolArchive {
    pub fn archive(&self) -> &ValidatedArchive {
        &self.archive
    }
    pub fn programs(&self) -> &[ToolProgram] {
        &self.programs
    }
}

pub fn admit_tool_archive(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    input: ToolArchiveInput<'_>,
    clock: impl FnMut() -> u64,
) -> Result<InertToolArchive, ToolAdmissionError> {
    admit_with_profiles(trust, store, input, clock, REVIEWED_TOOLS)
}

fn admit_with_profiles(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    input: ToolArchiveInput<'_>,
    mut clock: impl FnMut() -> u64,
    profiles: &[ReviewedToolProfile],
) -> Result<InertToolArchive, ToolAdmissionError> {
    if input.manifest.len() > MAX_TOOL_MANIFEST_BYTES {
        return Err(ToolAdmissionError::ManifestBudgetExceeded);
    }
    let target = ManifestCompatibility::embedded()?.target;
    let descriptor = trust.verify_descriptor(
        store,
        DescriptorVerification {
            envelope: input.envelope,
            manifest: input.manifest,
            archive: input.archive,
            target: target.clone(),
            now: clock(),
        },
    )?;
    if descriptor.payload().files.len() > MAX_TOOL_FILES {
        return Err(ToolAdmissionError::ManifestBudgetExceeded);
    }
    if descriptor.payload().kind != PackageKind::Tool {
        return Err(ToolAdmissionError::ManifestInvalid);
    }
    let archive = validate_archive(&descriptor, input.manifest, input.archive)?;
    let programs = validate_manifest(&archive, &target, profiles)?;
    // Observe a fresh T0 clock after bounded decode/validation. An opaque receipt
    // from earlier in this operation cannot bypass expiry, floors or current root.
    trust.check_receipts(store, &[archive.descriptor()], &target, clock())?;
    Ok(InertToolArchive { archive, programs })
}

fn validate_manifest(
    archive: &ValidatedArchive,
    target: &PackageTarget,
    profiles: &[ReviewedToolProfile],
) -> Result<Vec<ToolProgram>, ToolAdmissionError> {
    let invalid = || ToolAdmissionError::ManifestInvalid;
    let manifest: ToolManifest = serde_json::from_value(
        strict_json(&archive.files()["manifest.json"]).map_err(|_| invalid())?,
    )
    .map_err(|_| invalid())?;
    let payload = archive.descriptor().payload();
    if manifest.format_version != 1
        || manifest.kind != PackageKind::Tool
        || manifest.publisher != payload.publisher
        || manifest.id != payload.id
        || manifest.version != payload.version
        || manifest.target != *target
        || manifest.build_flavor != payload.build_flavor
        || manifest.source.registry != payload.source.registry
        || manifest.source.artifact != payload.source.artifact
        || manifest.license != payload.license
        || manifest.files != payload.files
        || manifest.programs.is_empty()
        || manifest.programs.len() > 2
        || manifest.resources.len() > MAX_TOOL_FILES
    {
        return Err(invalid());
    }
    let profile = profiles
        .iter()
        .find(|profile| {
            profile.publisher == manifest.publisher
                && profile.id == manifest.id
                && profile.registry == manifest.source.registry
                && profile.artifact == manifest.source.artifact
                && profile.license == manifest.license
                && profile.build_flavor == manifest.build_flavor
                && profile.target == *target
        })
        .ok_or(ToolAdmissionError::PolicyDenied)?;
    let mut roles = BTreeSet::new();
    let mut paths = BTreeSet::new();
    if manifest.programs.len() != profile.programs.len()
        || manifest.resources.len() != profile.resources.len()
    {
        return Err(ToolAdmissionError::PolicyDenied);
    }
    for program in &manifest.programs {
        if !roles.insert(program.role)
            || !paths.insert(program.path.as_str())
            || !profile
                .programs
                .contains(&(program.role, program.path.as_str()))
        {
            return Err(ToolAdmissionError::PolicyDenied);
        }
    }
    for resource in &manifest.resources {
        if !paths.insert(resource.as_str()) || !profile.resources.contains(&resource.as_str()) {
            return Err(ToolAdmissionError::PolicyDenied);
        }
    }
    // Every signed business file must have a reviewed role, including license
    // material. Extra DLLs/helpers/install scripts cannot hide as signed resources.
    if paths.len() != manifest.files.len()
        || manifest
            .files
            .iter()
            .any(|file| file.size == 0 || !paths.contains(file.path.as_str()))
    {
        return Err(ToolAdmissionError::PolicyDenied);
    }
    Ok(roles.into_iter().collect())
}

#[cfg(test)]
pub(super) fn admit_fixture(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    input: ToolArchiveInput<'_>,
    clock: impl FnMut() -> u64,
) -> Result<InertToolArchive, ToolAdmissionError> {
    const FIXTURE: &[ReviewedToolProfile] = &[ReviewedToolProfile {
        publisher: "fixture-publisher",
        id: "ffmpeg",
        registry: "fixture-registry",
        artifact: "fixture-ffmpeg",
        license: "LGPL-2.1-or-later",
        build_flavor: "fixture-static",
        target: PackageTarget {
            platform: super::PackagePlatform::Windows,
            arch: super::PackageArch::X64,
        },
        programs: &[
            (ToolProgram::Ffmpeg, "bin/ffmpeg.exe"),
            (ToolProgram::Ffprobe, "bin/ffprobe.exe"),
        ],
        resources: &["licenses/ffmpeg.txt"],
    }];
    admit_with_profiles(trust, store, input, clock, FIXTURE)
}
