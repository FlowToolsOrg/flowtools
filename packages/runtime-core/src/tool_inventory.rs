//! Read-only observations, never package admission, a lease or deletion consent.
use crate::protocol::ErrorCode;
use serde::{Deserialize, Serialize};
use specta::Type;

pub const PAGE_SIZE: u32 = 8;

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolInventoryQuery {
    pub offset: u32,
    pub snapshot: Option<String>,
}
impl ToolInventoryQuery {
    pub(crate) fn validate(&self) -> Result<(), ErrorCode> {
        if self.offset > 64
            || !self.offset.is_multiple_of(PAGE_SIZE)
            || (self.offset > 0 && self.snapshot.is_none())
            || self.snapshot.as_ref().is_some_and(|s| {
                s.len() != 64
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        {
            return Err(ErrorCode::InputInvalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ToolInventoryAvailability {
    NotConfigured,
    Observed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ToolDeletionBlocker {
    InstalledDependent,
    RetainedTask,
    DeletionNotImplemented,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDependent {
    pub publisher: String,
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolInventoryItem {
    pub publisher: String,
    pub id: String,
    pub version: String,
    pub build_flavor: String,
    pub archive_digest: String,
    pub source_registry: String,
    pub source_artifact: String,
    pub archive_bytes: u32,
    pub materialized: bool,
    pub dependents: Vec<ToolDependent>,
    pub retained_jobs: u32,
    pub deletion_blockers: Vec<ToolDeletionBlocker>,
}

/// All four shared installation pools, including their retained/quarantined bytes.
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolStorageUsage {
    pub bytes: u32,
    pub groups: u32,
    pub max_bytes: u32,
    pub max_groups: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolInventoryPage {
    pub availability: ToolInventoryAvailability,
    pub snapshot: Option<String>,
    pub total: u32,
    pub next_offset: Option<u32>,
    pub storage: Option<ToolStorageUsage>,
    pub items: Vec<ToolInventoryItem>,
}
impl ToolInventoryPage {
    #[cfg(any(windows, test))]
    pub(crate) fn observed(
        mut items: Vec<ToolInventoryItem>,
        storage: ToolStorageUsage,
        query: &ToolInventoryQuery,
    ) -> Result<Self, ErrorCode> {
        query.validate()?;
        if items.len() > 64 {
            return Err(ErrorCode::BudgetExceeded);
        }
        items.sort_by(|a, b| {
            (&a.publisher, &a.id, &a.version, &a.archive_digest).cmp(&(
                &b.publisher,
                &b.id,
                &b.version,
                &b.archive_digest,
            ))
        });
        let snapshot =
            crate::catalog::digest(&serde_json::json!({"items":items,"storage":storage}));
        if query.snapshot.as_ref().is_some_and(|s| *s != snapshot) {
            return Err(ErrorCode::RevisionConflict);
        }
        let total = items.len() as u32;
        if query.offset > 0 && query.offset >= total {
            return Err(ErrorCode::InputInvalid);
        }
        let end = (query.offset + PAGE_SIZE).min(total);
        let page = Self {
            availability: ToolInventoryAvailability::Observed,
            snapshot: Some(snapshot),
            total,
            next_offset: (end < total).then_some(end),
            storage: Some(storage),
            items: items
                .into_iter()
                .skip(query.offset as usize)
                .take(PAGE_SIZE as usize)
                .collect(),
        };
        if serde_json::to_vec(&page)
            .map_err(|_| ErrorCode::StorageFailed)?
            .len()
            > crate::protocol::MAX_FRAME_BYTES - 4096
        {
            return Err(ErrorCode::FrameTooLarge);
        }
        Ok(page)
    }
    pub(crate) fn not_configured(query: &ToolInventoryQuery) -> Result<Self, ErrorCode> {
        query.validate()?;
        if query.offset != 0 || query.snapshot.is_some() {
            return Err(ErrorCode::RevisionConflict);
        }
        Ok(Self {
            availability: ToolInventoryAvailability::NotConfigured,
            snapshot: None,
            total: 0,
            next_offset: None,
            storage: None,
            items: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_inventory_pages_preserve_one_snapshot_and_maximum_dependents_fit_the_wire() {
        let items = (0..64)
            .map(|n| ToolInventoryItem {
                publisher: "p".repeat(128),
                id: format!("tool-{n}"),
                version: "1.0.0".into(),
                build_flavor: "static".into(),
                archive_digest: format!("{n:064x}"),
                source_registry: "r".repeat(128),
                source_artifact: "a".repeat(128),
                archive_bytes: 1,
                materialized: false,
                dependents: (0..128)
                    .map(|n| ToolDependent {
                        publisher: "p".repeat(128),
                        id: format!("dependent-{n}"),
                        version: format!("{}.0.0", "1".repeat(124)),
                    })
                    .collect(),
                retained_jobs: 0,
                deletion_blockers: vec![
                    ToolDeletionBlocker::DeletionNotImplemented,
                    ToolDeletionBlocker::InstalledDependent,
                ],
            })
            .collect::<Vec<_>>();
        let usage = ToolStorageUsage {
            bytes: 64,
            groups: 64,
            max_bytes: 268_435_456,
            max_groups: 384,
        };
        let mut query = ToolInventoryQuery {
            offset: 0,
            snapshot: None,
        };
        let mut seen = std::collections::BTreeSet::new();
        for page_number in 0..8 {
            let page = ToolInventoryPage::observed(items.clone(), usage.clone(), &query).unwrap();
            assert_eq!(page.items.len(), 8);
            assert_eq!(page.total, 64);
            if let Some(snapshot) = &query.snapshot {
                assert_eq!(Some(snapshot), page.snapshot.as_ref());
            }
            for item in &page.items {
                assert!(seen.insert(item.archive_digest.clone()));
            }
            let bytes = serde_json::to_vec(&crate::protocol::Response {
                version: 1,
                request_id: "x".repeat(256),
                outcome: crate::protocol::Outcome::ToolInventory(page.clone()),
            })
            .unwrap();
            assert!(bytes.len() < crate::protocol::MAX_FRAME_BYTES);
            query.snapshot = page.snapshot;
            if page_number < 7 {
                query.offset = page.next_offset.unwrap();
            } else {
                assert!(page.next_offset.is_none());
            }
        }
        assert_eq!(seen.len(), 64);
        let mut changed = usage.clone();
        changed.bytes += 1;
        assert_eq!(
            ToolInventoryPage::observed(items.clone(), changed, &query).unwrap_err(),
            ErrorCode::RevisionConflict
        );
        query.offset = 64;
        assert_eq!(
            ToolInventoryPage::observed(items, usage, &query).unwrap_err(),
            ErrorCode::InputInvalid
        );
    }
    #[test]
    fn queries_bound_pages_and_require_an_exact_snapshot_after_the_first_page() {
        for (offset, snapshot, accepted) in [
            (0, None, true),
            (8, Some("a".repeat(64)), true),
            (1, None, false),
            (72, Some("a".repeat(64)), false),
            (8, None, false),
            (0, Some("A".repeat(64)), false),
            (0, Some("a".repeat(65)), false),
        ] {
            assert_eq!(
                ToolInventoryQuery { offset, snapshot }.validate().is_ok(),
                accepted
            );
        }
        let empty = ToolInventoryPage::not_configured(&ToolInventoryQuery {
            offset: 0,
            snapshot: None,
        })
        .unwrap();
        assert!(empty.snapshot.is_none() && empty.storage.is_none() && empty.items.is_empty());
        assert_eq!(
            ToolInventoryPage::not_configured(&ToolInventoryQuery {
                offset: 8,
                snapshot: Some("a".repeat(64))
            })
            .unwrap_err(),
            ErrorCode::RevisionConflict
        );
    }
}
