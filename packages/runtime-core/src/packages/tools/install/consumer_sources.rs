//! Internal installed-ZIP reader for the inert consumer transaction. No caller
//! pins or review-policy selector, and no executable paths leave this module.
use super::*;
use std::collections::BTreeMap;

pub(crate) struct HeldToolArchives {
    pub(crate) archives: Vec<InertToolArchive>,
    _held: Vec<files::HeldBlob>,
    _layout: Option<files::Layout>,
}
// Recovery reads only known raw records; unknown raw groups may coexist after a
// backup restore. They are never returned, admitted, adopted or moved here.
pub(crate) fn validate_reconciliation_sources(
    trust: &PackageTrustStore,
    store: &mut DataStore,
) -> Result<()> {
    let (state, snapshot) = known_inventory(trust, store)?;
    crate::packages::install::tool_consumers::validate_reconciliation_references(
        store,
        &snapshot,
        &state.records.iter().map(Record::pin).collect::<Vec<_>>(),
    )?;
    if let Some(layout) = files::Layout::open_for_tools(store, false)? {
        layout.disk_bytes()?;
        for record in &state.records {
            layout.read(record)?;
        }
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    Ok(())
}
pub(crate) fn validate_inventory(trust: &PackageTrustStore, store: &mut DataStore) -> Result<()> {
    let state = inventory(trust, store)?;
    if let Some(layout) = files::Layout::open_for_tools(store, false)? {
        layout.assert_owned(&state.records, None)?;
        layout.disk_bytes()?;
        for record in &state.records {
            layout.read(record)?;
        }
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    Ok(())
}
pub(crate) fn read(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    clock: &mut dyn FnMut() -> u64,
) -> Result<HeldToolArchives> {
    read_with_admission(trust, store, clock, None, |t, s, b, c| {
        admit_tool_archive(t, s, b, c)
    })
}
#[cfg(test)]
pub(crate) fn read_fixture(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    clock: &mut dyn FnMut() -> u64,
) -> Result<HeldToolArchives> {
    read_with_admission(trust, store, clock, None, |t, s, b, c| {
        super::super::admit_fixture(t, s, b, c)
    })
}
// An accepted run retains exact old ZIP identities across updates. Selection
// values are not proofs: every selected actual held ZIP is admitted again.
pub(crate) fn read_exact(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    pins: &[ToolArchivePin],
    clock: &mut dyn FnMut() -> u64,
) -> Result<HeldToolArchives> {
    images::require_no_pending(&store.connection)?;
    read_with_admission(trust, store, clock, Some(pins), |t, s, b, c| {
        admit_tool_archive(t, s, b, c)
    })
}
#[cfg(test)]
pub(crate) fn read_exact_fixture(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    pins: &[ToolArchivePin],
    clock: &mut dyn FnMut() -> u64,
) -> Result<HeldToolArchives> {
    images::require_no_pending(&store.connection)?;
    read_with_admission(trust, store, clock, Some(pins), |t, s, b, c| {
        super::super::admit_fixture(t, s, b, c)
    })
}
fn read_with_admission(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    clock: &mut dyn FnMut() -> u64,
    requested: Option<&[ToolArchivePin]>,
    mut admission: impl FnMut(
        &PackageTrustStore,
        &mut DataStore,
        ToolArchiveInput<'_>,
        &mut dyn FnMut() -> u64,
    ) -> std::result::Result<InertToolArchive, ToolAdmissionError>,
) -> Result<HeldToolArchives> {
    native()?;
    // Own consumer pending is intentionally excluded: this read returns only
    // inert opaque candidates for that transaction's final re-admission.
    let blocked: bool = store.connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key IN ('package-install-pending-v1','package-reconciliation-pending-v1','tool-archive-install-pending-v1','tool-archive-reconciliation-pending-v1','tool-image-materialization-pending-v1','tool-image-reconciliation-pending-v1'))",
        [], |r| r.get(0),
    ).map_err(storage)?;
    if blocked {
        return Err(InstallError::RecoveryPending.into());
    }
    let state = inventory(trust, store)?;
    let layout = files::Layout::open_for_tools(store, false)?;
    if let Some(layout) = &layout {
        layout.assert_owned(&state.records, None)?;
        layout.disk_bytes()?;
        for record in &state.records {
            layout.read(record)?;
        }
    } else if !state.records.is_empty() {
        return Err(InstallError::ArtifactInvalid.into());
    }
    let selected = if let Some(pins) = requested {
        if pins.len() > MAX_RECORDS {
            return Err(InstallError::BudgetExceeded.into());
        }
        let mut unique = BTreeSet::new();
        let mut selected = Vec::new();
        for pin in pins {
            if !unique.insert(pin.archive_digest.as_str()) {
                return Err(invalid());
            }
            selected.push(
                state
                    .records
                    .iter()
                    .find(|record| record.pin() == *pin)
                    .ok_or(InstallError::ArtifactInvalid)?,
            );
        }
        selected
    } else {
        let mut latest = BTreeMap::new();
        for record in &state.records {
            let key = (
                &record.payload.publisher,
                &record.payload.id,
                &record.payload.build_flavor,
            );
            if latest.get(&key).is_none_or(|previous: &&Record| {
                previous.payload.release_sequence < record.payload.release_sequence
            }) {
                latest.insert(key, record);
            }
        }
        latest.into_values().collect()
    };
    let mut held = Vec::new();
    let mut archives = Vec::new();
    let mut bytes = 0u64;
    for record in selected {
        bytes = bytes
            .checked_add(record.payload.archive.size)
            .and_then(|n| n.checked_add(record.payload.manifest.size))
            .ok_or(InstallError::BudgetExceeded)?;
        if bytes > 134_217_728 {
            return Err(InstallError::BudgetExceeded.into());
        }
        let source = layout
            .as_ref()
            .ok_or(InstallError::ArtifactInvalid)?
            .hold(record)?;
        let envelope = packages::decode_base64(&record.envelope).map_err(|_| invalid())?;
        let archive = admission(
            trust,
            store,
            ToolArchiveInput {
                envelope: &envelope,
                manifest: &source.manifest,
                archive: &source.archive,
            },
            clock,
        )?;
        if value_digest(archive.archive().descriptor().payload())? != value_digest(&record.payload)?
        {
            return Err(invalid());
        }
        archives.push(archive);
        held.push(source);
    }
    Ok(HeldToolArchives {
        archives,
        _held: held,
        _layout: layout,
    })
}
