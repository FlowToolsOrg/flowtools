//! Existing-image source for accepted artifact retention. This module
//! cannot materialize, publish, expose a path, grant access or start a process.
use super::*;
use crate::packages::tools::InertToolArchive;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ImageReference {
    pin: ToolArchivePin,
    source_artifact_id: String,
    seal_id: String,
    seal_digest: String,
}
impl ImageReference {
    pub(crate) fn valid(&self) -> bool {
        uuid(&self.source_artifact_id)
            && uuid(&self.seal_id)
            && packages::valid_digest(&self.seal_digest)
            && packages::valid_digest(&self.pin.archive_digest)
            && crate::dependencies::valid_dependency_id(&self.pin.publisher)
            && crate::dependencies::valid_dependency_id(&self.pin.id)
    }
    pub(crate) fn pin(&self) -> &ToolArchivePin {
        &self.pin
    }
    fn from_record(record: &Record) -> Result<Self> {
        Ok(Self {
            pin: record.pin.clone(),
            source_artifact_id: record.source_artifact_id.clone(),
            seal_id: record.seal_id.clone(),
            seal_digest: record
                .seal_file
                .as_ref()
                .ok_or_else(invalid)?
                .content
                .sha256
                .clone(),
        })
    }
}
// Only the actual current source/image checks below can construct the held
// counterpart. Deserializing an ImageReference never creates one.
pub(crate) struct HeldImage {
    reference: ImageReference,
    _image: MaterializedInertTool,
}
impl HeldImage {
    pub(crate) fn reference(&self) -> &ImageReference {
        &self.reference
    }
}
pub(crate) fn hold_existing(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    admitted: &InertToolArchive,
    mut clock: impl FnMut() -> u64,
) -> Result<HeldImage> {
    require_no_pending(&store.connection)?;
    let receipt = admitted.archive().descriptor();
    trust
        .check_receipts(store, &[receipt], &receipt.payload().target, clock())
        .map_err(|e| ToolInstallError::Admission(e.into()))?;
    let source = source_state(trust, store)?;
    let original = source
        .records
        .iter()
        .find(|record| record.payload.archive.sha256 == receipt.package_digest())
        .ok_or(InstallError::ArtifactInvalid)?;
    if value_digest(&original.payload)? != value_digest(receipt.payload())? {
        return Err(invalid());
    }
    let state = inventory(store, &source)?;
    let record = state
        .records
        .iter()
        .find(|record| record.pin == original.pin())
        .ok_or(InstallError::ArtifactInvalid)?;
    let source_layout =
        files::Layout::open_for_tools(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    source_layout.assert_owned(&source.records, None)?;
    let source_blob = source_layout.hold(original)?;
    // hold() checks the descriptor's exact original ZIP, manifest and native
    // file policy; current admission cannot be rebound to same-ID other bytes.
    let layout = tree::Layout::open(store, false)?.ok_or(InstallError::ArtifactInvalid)?;
    layout.assert_owned(&state.records, None)?;
    check_usage(store, &layout, 0, 0)?;
    let mut held = layout.read(record, &state.profile_digest)?;
    trust
        .check_receipts(store, &[receipt], &original.payload.target, clock())
        .map_err(|e| ToolInstallError::Admission(e.into()))?;
    require_no_pending(&store.connection)?;
    let current_source = source_state(trust, store)?;
    let current_state = inventory(store, &current_source)?;
    if value_digest(&current_source)? != value_digest(&source)?
        || value_digest(&current_state)? != value_digest(&state)?
    {
        return Err(InstallError::InventoryChanged.into());
    }
    // ACL/identity checks are repeated after final trust IO; holding old handles
    // is not a cached grant or a permanent assertion about current policy.
    held.extend(layout.read(record, &state.profile_digest)?);
    held.extend(source_blob.guards);
    Ok(HeldImage {
        reference: ImageReference::from_record(record)?,
        _image: MaterializedInertTool {
            record: record.clone(),
            _layout: layout,
            _source_layout: source_layout,
            _held: held,
        },
    })
}
// Provenance/retention checks intentionally need no renewed execution grant:
// revoked/expired artifacts still have to remain pinned for interrupted jobs.
pub(crate) fn validate_references(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    references: &[ImageReference],
) -> Result<()> {
    let source = source_state(trust, store)?;
    let state = inventory(store, &source)?;
    for reference in references {
        if !state.records.iter().any(|record| {
            ImageReference::from_record(record).is_ok_and(|actual| actual == *reference)
        }) {
            return Err(InstallError::ArtifactInvalid.into());
        }
    }
    Ok(())
}
// Called with an already validated raw ledger. Never call source_state here:
// that reader itself uses this preflight before unrelated installation/cleanup.
pub(in crate::packages::tools::install) fn validate_retained_jobs(
    store: &DataStore,
    source: &super::super::Inventory,
) -> Result<()> {
    let leases = crate::runtime::retained_tool_leases(store).map_err(|error| match error {
        crate::protocol::ErrorCode::RecoveryPending => {
            ToolInstallError::Storage(InstallError::RecoveryPending)
        }
        crate::protocol::ErrorCode::StorageFailed => {
            ToolInstallError::Storage(InstallError::Storage)
        }
        crate::protocol::ErrorCode::BudgetExceeded => {
            ToolInstallError::Storage(InstallError::BudgetExceeded)
        }
        _ => invalid(),
    })?;
    if leases.is_empty() {
        return Ok(());
    }
    let state = inventory(store, source)?;
    for lease in leases {
        if !lease.matches_scope(store).map_err(|_| invalid())? {
            return Err(InstallError::RecoveryPending.into());
        }
        for reference in lease.references() {
            if !state.records.iter().any(|record| {
                ImageReference::from_record(record).is_ok_and(|actual| actual == *reference)
            }) {
                return Err(InstallError::ArtifactInvalid.into());
            }
        }
    }
    Ok(())
}
