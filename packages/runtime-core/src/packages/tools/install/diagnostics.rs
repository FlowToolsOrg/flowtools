//! Strict stored-metadata observation. Does not re-admit tools or create roots.
use super::*;
use crate::{protocol::ErrorCode, tool_inventory::*};

pub(crate) fn observe(
    trust: &PackageTrustStore,
    store: &mut DataStore,
    query: &ToolInventoryQuery,
) -> std::result::Result<ToolInventoryPage, ErrorCode> {
    query.validate()?;
    let before = leases::metadata_stamp(&store.connection)?;
    images::require_no_pending(&store.connection).map_err(|e| leases::failed(e.into()))?;
    // These existing readers retain every profile, pending, native identity,
    // actual-byte and historical-job guard. None creates an admission receipt.
    let pins = inspect_inert_tools(trust, store).map_err(leases::failed)?;
    let materialized = images::inspect_inert_tool_images(trust, store).map_err(leases::failed)?;
    let consumers = packages::install::tool_consumers::inspect_inert_tool_consumers(trust, store)
        .map_err(leases::failed)?;
    let state = inventory(trust, store).map_err(leases::failed)?;
    let retained = crate::runtime::retained_tool_leases(store)?;
    let retained_digest =
        digest(&serde_json::to_value(&retained).map_err(|_| ErrorCode::StoreCorrupt)?);
    let mut usage = images::retained_usage(store).map_err(|e| leases::failed(e.into()))?;
    for layout in [
        files::Layout::open_for_tools(store, false),
        files::Layout::open(store, false),
    ] {
        if let Some(layout) = layout.map_err(|e| leases::failed(e.into()))? {
            usage = usage
                .combine(
                    layout
                        .disk_usage_reserving(0)
                        .map_err(|e| leases::failed(e.into()))?,
                )
                .map_err(|e| leases::failed(e.into()))?;
        }
    }
    usage = usage
        .combine(
            packages::install::tool_consumers::retained_usage(store)
                .map_err(|e| leases::failed(e.into()))?,
        )
        .map_err(|e| leases::failed(e.into()))?;
    if usage.bytes > files::MAX_DISK_BYTES || usage.groups > files::MAX_GROUPS {
        return Err(ErrorCode::BudgetExceeded);
    }
    let storage = ToolStorageUsage {
        bytes: usage.bytes as u32,
        groups: usage.groups as u32,
        max_bytes: files::MAX_DISK_BYTES as u32,
        max_groups: files::MAX_GROUPS as u32,
    };
    let mut items = Vec::new();
    for record in &state.records {
        let pin = record.pin();
        if !pins.contains(&pin) {
            return Err(ErrorCode::DependencyArtifactMismatch);
        }
        let mut dependents = consumers
            .iter()
            .filter(|c| {
                c.dependency_lock().tools.iter().any(|t| {
                    t.package_digest == pin.archive_digest
                        && t.publisher == pin.publisher
                        && t.id == pin.id
                        && t.version == pin.version
                        && t.target == pin.target
                        && t.build_flavor == pin.build_flavor
                })
            })
            .map(|c| {
                let p = c.pin();
                ToolDependent {
                    publisher: p.publisher,
                    id: p.id,
                    version: p.version,
                }
            })
            .collect::<Vec<_>>();
        dependents.sort_by(|a, b| {
            (&a.publisher, &a.id, &a.version).cmp(&(&b.publisher, &b.id, &b.version))
        });
        let retained_jobs = retained
            .iter()
            .filter(|l| l.references().iter().any(|r| r.pin() == &pin))
            .count() as u32;
        let mut deletion_blockers = vec![ToolDeletionBlocker::DeletionNotImplemented];
        if !dependents.is_empty() {
            deletion_blockers.push(ToolDeletionBlocker::InstalledDependent);
        }
        if retained_jobs > 0 {
            deletion_blockers.push(ToolDeletionBlocker::RetainedTask);
        }
        items.push(ToolInventoryItem {
            publisher: pin.publisher,
            id: pin.id,
            version: pin.version,
            build_flavor: pin.build_flavor,
            archive_digest: pin.archive_digest,
            source_registry: record.payload.source.registry.clone(),
            source_artifact: record.payload.source.artifact.clone(),
            archive_bytes: u32::try_from(record.payload.archive.size)
                .map_err(|_| ErrorCode::BudgetExceeded)?,
            materialized: materialized.contains(&record.pin()),
            dependents,
            retained_jobs,
            deletion_blockers,
        });
    }
    if leases::metadata_stamp(&store.connection)? != before
        || digest(
            &serde_json::to_value(crate::runtime::retained_tool_leases(store)?)
                .map_err(|_| ErrorCode::StoreCorrupt)?,
        ) != retained_digest
    {
        return Err(ErrorCode::RevisionConflict);
    }
    ToolInventoryPage::observed(items, storage, query)
}
