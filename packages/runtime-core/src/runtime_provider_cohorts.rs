//! Opaque T0 catalog updates. A cohort changes metadata only after every old
//! provider drains; it never installs tools, transfers grants or selects a Host.
use super::*;
use crate::{dependencies::PackagePin, services::ProviderChangePlan};

/// A presented whole-catalog change, bound to this Core instance. Confirmation
/// is migration consent only; ordinary new-version execution still needs grants.
///
/// ```compile_fail
/// use flowtools_runtime_core::runtime::ProviderCohortPlan;
/// fn wire<T: serde::Serialize>() {}
/// wire::<ProviderCohortPlan>();
/// ```
#[derive(Debug)]
pub struct ProviderCohortPlan {
    instance: String,
    digest: String,
    changes: Vec<ProviderChangePlan>,
}
impl ProviderCohortPlan {
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn changes(&self) -> &[ProviderChangePlan] {
        &self.changes
    }
}
pub(super) struct Candidate {
    before: String,
    catalog: BuiltinCatalog,
    changes: Vec<ProviderChangePlan>,
    begun: bool,
}
fn pin(id: &str, manifest: &Value) -> Result<PackagePin, ErrorCode> {
    Ok(PackagePin {
        publisher: manifest["publisher"]
            .as_str()
            .ok_or(ErrorCode::DependencyInvalid)?
            .into(),
        id: id.into(),
        version: manifest["version"]
            .as_str()
            .ok_or(ErrorCode::DependencyInvalid)?
            .into(),
        digest: digest(manifest),
    })
}
fn same_plan(left: &ProviderChangePlan, right: &ProviderChangePlan) -> bool {
    serde_json::to_value(left).ok() == serde_json::to_value(right).ok()
}

// Migration SQL cannot silently replace policy/trust/install authority while
// committing a previously checked catalog. Values are bounded before decoding.
fn authority_frame(connection: &rusqlite::Connection) -> Result<String, ErrorCode> {
    let mut values = Vec::new();
    let mut bytes = 0usize;
    let mut statement = connection.prepare("SELECT CASE WHEN typeof(identity)='text' AND length(CAST(identity AS BLOB))<=65536 THEN identity ELSE NULL END,epoch,CASE WHEN typeof(state)='text' AND length(CAST(state AS BLOB))<=65536 THEN state ELSE NULL END FROM grants ORDER BY identity LIMIT 257").map_err(|_| ErrorCode::StorageFailed)?;
    let mut rows = statement.query([]).map_err(|_| ErrorCode::StorageFailed)?;
    while let Some(row) = rows.next().map_err(|_| ErrorCode::StorageFailed)? {
        let identity: String = row.get(0).map_err(|_| ErrorCode::StoreCorrupt)?;
        let epoch: u32 = row.get(1).map_err(|_| ErrorCode::StoreCorrupt)?;
        let state: String = row.get(2).map_err(|_| ErrorCode::StoreCorrupt)?;
        bytes = bytes
            .checked_add(identity.len())
            .and_then(|n| n.checked_add(state.len()))
            .ok_or(ErrorCode::BudgetExceeded)?;
        if identity.len() > 65536 || values.len() >= 256 || bytes > 16_777_216 || epoch == 0 {
            return Err(ErrorCode::StoreCorrupt);
        }
        values.push((identity, epoch, digest(&json!(state))));
    }
    let mut metadata = Vec::new();
    let mut statement = connection.prepare("SELECT key,CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=1048576 THEN value ELSE NULL END FROM core_metadata WHERE key IN ('catalog-digest','cold-start','policy-audit','recovery-generation') ORDER BY key").map_err(|_| ErrorCode::StorageFailed)?;
    let mut rows = statement.query([]).map_err(|_| ErrorCode::StorageFailed)?;
    while let Some(row) = rows.next().map_err(|_| ErrorCode::StorageFailed)? {
        let key: String = row.get(0).map_err(|_| ErrorCode::StoreCorrupt)?;
        let value: String = row.get(1).map_err(|_| ErrorCode::StoreCorrupt)?;
        metadata.push((key, digest(&json!(value))));
    }
    #[cfg(windows)]
    let tools = leases::metadata_stamp(connection)?;
    #[cfg(not(windows))]
    let tools = "no-native-tools";
    Ok(digest(&json!([values, metadata, tools])))
}

impl RuntimeCore {
    pub(super) fn cohort_member(&self, ticket: &str) -> bool {
        self.provider_cohorts
            .values()
            .any(|candidate| candidate.changes.iter().any(|plan| plan.digest == ticket))
    }
    pub(super) fn cohort_provider(&self, provider: &PackagePin) -> bool {
        self.provider_cohorts.values().any(|candidate| {
            candidate
                .changes
                .iter()
                .any(|plan| plan.provider.identity() == provider.identity())
        })
    }
    fn cohort(&self, plan: &ProviderCohortPlan) -> Result<&Candidate, ErrorCode> {
        if plan.instance != self.instance_id {
            return Err(ErrorCode::InstanceMismatch);
        }
        let candidate = self
            .provider_cohorts
            .get(&plan.digest)
            .ok_or(ErrorCode::ApprovalRequired)?;
        if candidate.before != self.catalog.snapshot_digest()
            || !self.services.matches_catalog(&self.catalog)
        {
            return Err(ErrorCode::ApprovalRequired);
        }
        Ok(candidate)
    }
    /// Fixed build-owned snapshots only, with the same members/publishers and
    /// strictly newer versions for every changed member. No rollback or import.
    pub fn stage_provider_cohort(
        &mut self,
        catalog: BuiltinCatalog,
    ) -> Result<ProviderCohortPlan, ErrorCode> {
        if self.stopping {
            return Err(ErrorCode::RuntimeBusy);
        }
        if self.provider_candidates.len() + self.provider_cohorts.len() >= 64
            || self.retained_catalogs.len() >= 64
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        if !self.services.matches_catalog(&self.catalog) {
            self.refresh_service_catalog()?;
        }
        if catalog.list().count() != self.catalog.list().count() {
            return Err(ErrorCode::DependencyInvalid);
        }
        let mut changes = Vec::new();
        for (id, old) in self.catalog.list() {
            let new = catalog
                .list()
                .find(|(key, _)| *key == id)
                .ok_or(ErrorCode::DependencyMissing)?
                .1;
            if old == new {
                continue;
            }
            let provider = pin(id, old)?;
            let replacement = pin(id, new)?;
            let old_version: node_semver::Version = provider
                .version
                .parse()
                .map_err(|_| ErrorCode::DependencyInvalid)?;
            let new_version: node_semver::Version = replacement
                .version
                .parse()
                .map_err(|_| ErrorCode::DependencyInvalid)?;
            if replacement.identity() != provider.identity() || new_version <= old_version {
                return Err(ErrorCode::DependencyInvalid);
            }
            if self.cohort_provider(&provider)
                || self.services.change_pending(&provider)
                || self.provider_candidates.values().any(|candidate| {
                    candidate
                        .list()
                        .any(|(key, value)| key == id && value != old)
                })
            {
                return Err(ErrorCode::RuntimeBusy);
            }
            changes.push(self.services.change_plan(&provider, Some(replacement))?);
        }
        if changes.is_empty() {
            return Err(ErrorCode::DependencyInvalid);
        }
        if changes.len()
            + self
                .provider_cohorts
                .values()
                .map(|candidate| candidate.changes.len())
                .sum::<usize>()
            > 64
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let ids = catalog.list().map(|(id, _)| id.clone()).collect::<Vec<_>>();
        self.plan_catalog(&catalog, &ids)?;
        let before = self.catalog.snapshot_digest();
        let ticket = digest(
            &json!({"instance":self.instance_id,"before":before,"after":catalog.snapshot_digest(),"changes":changes}),
        );
        self.provider_cohorts.insert(
            ticket.clone(),
            Candidate {
                before,
                catalog,
                changes: changes.clone(),
                begun: false,
            },
        );
        Ok(ProviderCohortPlan {
            instance: self.instance_id.clone(),
            digest: ticket,
            changes,
        })
    }
    pub fn begin_provider_cohort(
        &mut self,
        plan: &ProviderCohortPlan,
        confirmation: &str,
    ) -> Result<(), ErrorCode> {
        if self.stopping {
            return Err(ErrorCode::RuntimeBusy);
        }
        let candidate = self.cohort(plan)?;
        if confirmation != plan.digest || candidate.begun {
            return Err(ErrorCode::ApprovalRequired);
        }
        let changes = candidate.changes.clone();
        for change in &changes {
            if !same_plan(
                change,
                &self
                    .services
                    .change_plan(&change.provider, change.replacement.clone())?,
            ) {
                return Err(ErrorCode::ApprovalRequired);
            }
        }
        let mut entered: Vec<String> = Vec::new();
        for change in &changes {
            if let Err(code) = self
                .services
                .begin_change(change.clone(), &change.digest, false)
            {
                for ticket in entered {
                    self.services.abort_change(&ticket);
                }
                return Err(code);
            }
            entered.push(change.digest.clone());
        }
        self.provider_cohorts.get_mut(&plan.digest).unwrap().begun = true;
        Ok(())
    }
    pub fn abort_provider_cohort(&mut self, plan: &ProviderCohortPlan) -> Result<(), ErrorCode> {
        if self.stopping {
            return Err(ErrorCode::RuntimeBusy);
        }
        if plan.instance != self.instance_id {
            return Err(ErrorCode::InstanceMismatch);
        }
        let candidate = self
            .provider_cohorts
            .remove(&plan.digest)
            .ok_or(ErrorCode::ApprovalRequired)?;
        if candidate.begun {
            for change in candidate.changes {
                self.services.abort_change(&change.digest);
            }
        }
        Ok(())
    }
    /// One backup and guarded format-1 migration transaction for the complete
    /// catalog. This does not activate tools or transfer old-version grants.
    pub fn commit_provider_cohort(
        &mut self,
        plan: &ProviderCohortPlan,
        migrate: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        if self.stopping {
            return Err(ErrorCode::RuntimeBusy);
        }
        let candidate = self.cohort(plan)?;
        if !candidate.begun {
            return Err(ErrorCode::ApprovalRequired);
        }
        let changes = candidate.changes.clone();
        let catalog = candidate.catalog.clone();
        for change in &changes {
            if !same_plan(change, &self.services.presented_plan(&change.digest)?) {
                return Err(ErrorCode::ApprovalRequired);
            }
            if !self.services.drained(&change.digest)? {
                return Err(ErrorCode::RuntimeBusy);
            }
        }
        let ids = catalog.list().map(|(id, _)| id.clone()).collect::<Vec<_>>();
        let lock;
        #[cfg(windows)]
        let _held;
        match ServiceRegistry::plan(&catalog, &ids) {
            Ok(planned) => {
                lock = planned;
                #[cfg(windows)]
                {
                    _held = None::<PreparedArtifacts>;
                }
            }
            Err(code) => {
                #[cfg(windows)]
                {
                    let tools = self.tools.as_mut().ok_or(code)?;
                    let held = tools.prepare(&mut self.data, &catalog, &ids)?;
                    if leases::metadata_stamp(&self.data.connection)? != held.metadata_stamp() {
                        return Err(ErrorCode::StoreCorrupt);
                    }
                    lock = held.lock().clone();
                    _held = Some(held);
                }
                #[cfg(not(windows))]
                {
                    return Err(code);
                }
            }
        }
        match service_journal::read(&self.data.connection) {
            Ok(previous) if previous == self.provider_journal => (),
            result => {
                let code = result.err().unwrap_or(ErrorCode::StoreCorrupt);
                self.fail_closed();
                return Err(code);
            }
        }
        let frame = authority_frame(&self.data.connection)?;
        let backup = match self.data.backup() {
            Ok(path) => path,
            Err(code) => {
                self.abort_provider_cohort(plan)?;
                return Err(code);
            }
        };
        let backup_id = backup
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("runtime-backup-"))
            .ok_or(ErrorCode::StorageFailed)?
            .into();
        let mut journal = ProviderJournal {
            format_version: 1,
            phase: "prepared".into(),
            active_catalog: self.catalog.snapshot_digest(),
            pending_catalog: Some(catalog.snapshot_digest()),
            backup_id,
            disabled: self.services.disabled(),
        };
        let prepared = serde_json::to_string(&journal).map_err(|_| ErrorCode::StorageFailed)?;
        journal.phase = "committed".into();
        journal.active_catalog = journal.pending_catalog.take().unwrap();
        let committed = serde_json::to_string(&journal).map_err(|_| ErrorCode::StorageFailed)?;
        if let Err(failure) = service_journal::migrate(
            &mut self.data,
            &mut self.provider_journal,
            prepared,
            committed,
            migrate,
            |connection| {
                if authority_frame(connection)? != frame {
                    return Err(ErrorCode::StoreCorrupt);
                }
                Ok(())
            },
        ) {
            if failure.restored {
                self.abort_provider_cohort(plan)?;
            } else {
                self.fail_closed();
            }
            return Err(failure.code);
        }
        for change in &changes {
            if let Err(code) = self.services.commit_change(&change.digest, || Ok(())) {
                self.fail_closed();
                return Err(code);
            }
        }
        if let Err(code) = self.services.install_resolved_catalog(&catalog, lock) {
            self.fail_closed();
            return Err(code);
        }
        self.retained_catalogs.push(self.catalog.clone());
        self.catalog = catalog;
        if let Some(policy) = self.policy.as_mut() {
            policy.set_catalog(self.catalog.clone());
        }
        self.provider_cohorts.remove(&plan.digest);
        Ok(())
    }
}
