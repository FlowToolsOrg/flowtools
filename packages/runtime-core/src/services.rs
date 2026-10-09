//! T0-owned fixed T1 service leases. No installation, paths, grants or daemons.
use crate::{
    catalog::{digest, BuiltinCatalog},
    dependencies::*,
    protocol::ErrorCode,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceTarget {
    pub publisher: String,
    pub id: String,
    pub service: String,
    pub operation: String,
}
impl ServiceTarget {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if [&self.publisher, &self.id, &self.service, &self.operation]
            .iter()
            .all(|id| crate::dependencies::valid_dependency_id(id))
        {
            Ok(())
        } else {
            Err(ErrorCode::InvalidRequest)
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceCallDiagnostic {
    pub run_id: String,
    pub parent_run_id: String,
    pub root_run_id: String,
    pub root_caller: String,
    pub consumer: DependencyIdentity,
    pub provider: PackagePin,
    pub service: String,
    pub operation: String,
    pub dependency_lock: String,
    pub deadline: f64,
    pub state: String,
    pub failure_code: Option<ErrorCode>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderChangePlan {
    pub mode: String,
    pub digest: String,
    pub provider: PackagePin,
    pub replacement: Option<PackagePin>,
    pub consumers: Vec<DependencyIdentity>,
    pub affected_locks: Vec<String>,
}
struct Lease {
    pin: PackagePin,
    roots: BTreeSet<String>,
    calls: BTreeSet<String>,
    running: Option<String>,
    draining: bool,
}
#[derive(Default)]
pub struct ServiceRegistry {
    accepted: BTreeMap<String, DependencyLock>,
    releasing: BTreeSet<String>,
    leases: BTreeMap<DependencyIdentity, Lease>,
    disabled: BTreeSet<DependencyIdentity>,
    retained: BTreeMap<String, DependencyLock>,
    calls: BTreeMap<String, ServiceCallDiagnostic>,
    outputs: BTreeMap<String, (u32, usize)>,
    plans: BTreeMap<String, ProviderChangePlan>,
    installed: Option<DependencyLock>,
}
impl ServiceRegistry {
    pub fn disabled(&self) -> Vec<DependencyIdentity> {
        self.disabled.iter().cloned().collect()
    }
    pub fn restore_disabled(&mut self, identities: Vec<DependencyIdentity>) {
        self.disabled = identities.into_iter().collect();
    }
    pub fn enabled(&self, identity: &DependencyIdentity) -> bool {
        !self.disabled.contains(identity)
    }
    pub fn presented_plan(&self, ticket: &str) -> Result<ProviderChangePlan, ErrorCode> {
        self.plans
            .get(ticket)
            .cloned()
            .ok_or(ErrorCode::ApprovalRequired)
    }
    pub fn set_catalog(&mut self, catalog: &BuiltinCatalog) -> Result<(), ErrorCode> {
        let ids = catalog.list().map(|(id, _)| id.clone()).collect::<Vec<_>>();
        self.installed = Some(Self::plan(catalog, &ids)?);
        Ok(())
    }
    pub fn plan(catalog: &BuiltinCatalog, ids: &[String]) -> Result<DependencyLock, ErrorCode> {
        let manifests: Vec<_> = catalog.list().map(|(_, m)| m.clone()).collect();
        let roots = ids
            .iter()
            .map(|id| {
                manifests
                    .iter()
                    .find(|m| m["id"] == *id)
                    .map(|m| DependencyIdentity {
                        publisher: m["publisher"].as_str().unwrap_or("").into(),
                        id: id.clone(),
                    })
                    .ok_or(ErrorCode::DependencyMissing)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let platform = match std::env::consts::OS {
            "windows" => "windows",
            "macos" => "macos",
            "linux" => "linux",
            _ => return Err(ErrorCode::DependencyPlatformMismatch),
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            _ => return Err(ErrorCode::DependencyPlatformMismatch),
        };
        Ok(DependencyCatalog::from_manifests(manifests, vec![])
            .map_err(ErrorCode::from)?
            .resolve(
                &roots,
                &DependencyTarget {
                    platform: platform.into(),
                    arch: arch.into(),
                },
            )
            .map_err(ErrorCode::from)?
            .lock)
    }
    pub fn accept(
        &mut self,
        root: &str,
        lock: DependencyLock,
        output_budget: usize,
    ) -> Result<(), ErrorCode> {
        if self.accepted.contains_key(root)
            || self.accepted.len() >= 128
            || self.retained.len() >= 1024
            || lock.verified_digest().map_err(ErrorCode::from)? != lock.digest
        {
            return Err(ErrorCode::DependencyInvalid);
        }
        for pin in &lock.packages {
            let id = pin.identity();
            if self.disabled.contains(&id) {
                return Err(ErrorCode::DependencyMissing);
            }
            if let Some(lease) = self.leases.get(&id) {
                if lease.draining {
                    return Err(ErrorCode::RuntimeBusy);
                }
                if lease.pin != *pin && (!lease.roots.is_empty() || !lease.calls.is_empty()) {
                    return Err(ErrorCode::DependencyConflict);
                }
            }
        }
        for pin in &lock.packages {
            let id = pin.identity();
            let lease = self.leases.entry(id).or_insert_with(|| Lease {
                pin: pin.clone(),
                roots: BTreeSet::new(),
                calls: BTreeSet::new(),
                running: None,
                draining: false,
            });
            if lease.pin != *pin {
                lease.pin = pin.clone();
            }
            lease.roots.insert(root.into());
        }
        self.retained.insert(lock.digest.clone(), lock.clone());
        self.accepted.insert(root.into(), lock);
        self.outputs
            .insert(root.into(), (0, output_budget.min(1_032_192)));
        Ok(())
    }
    pub fn release_root(&mut self, root: &str) {
        if self.calls.values().any(|call| {
            call.root_run_id == root && matches!(call.state.as_str(), "queued" | "running")
        }) {
            self.releasing.insert(root.into());
            return;
        }
        self.releasing.remove(root);
        self.accepted.remove(root);
        self.outputs.remove(root);
        for lease in self.leases.values_mut() {
            lease.roots.remove(root);
        }
    }
    pub fn charge_output(&mut self, root: &str, bytes: usize) -> Result<(), ErrorCode> {
        if let Some(budget) = self.outputs.get_mut(root) {
            budget.1 = budget
                .1
                .checked_sub(bytes)
                .ok_or(ErrorCode::BudgetExceeded)?;
        }
        Ok(())
    }
    pub fn lock(&self, root: &str) -> Result<&DependencyLock, ErrorCode> {
        self.accepted.get(root).ok_or(ErrorCode::SessionInvalid)
    }
    pub fn provider(
        &self,
        root: &str,
        consumer: &DependencyIdentity,
        target: &ServiceTarget,
    ) -> Result<PackagePin, ErrorCode> {
        target.validate()?;
        self.lock(root)?
            .services
            .iter()
            .find(|pin| {
                pin.consumer == *consumer
                    && pin.provider.publisher == target.publisher
                    && pin.provider.id == target.id
                    && pin.service == target.service
            })
            .map(|pin| pin.provider.clone())
            .ok_or(ErrorCode::DependencyMissing)
    }
    pub fn add_call(&mut self, call: ServiceCallDiagnostic) -> Result<(), ErrorCode> {
        if self.calls.contains_key(&call.run_id) || self.calls.len() >= 4096 {
            return Err(ErrorCode::BudgetExceeded);
        }
        let budget = self
            .outputs
            .get_mut(&call.root_run_id)
            .ok_or(ErrorCode::SessionInvalid)?;
        if budget.0 >= 64 {
            return Err(ErrorCode::BudgetExceeded);
        }
        budget.0 += 1;
        let lease = self
            .leases
            .get_mut(&call.provider.identity())
            .ok_or(ErrorCode::DependencyMissing)?;
        if lease.pin != call.provider {
            return Err(ErrorCode::DependencyConflict);
        }
        lease.calls.insert(call.run_id.clone());
        self.calls.insert(call.run_id.clone(), call);
        Ok(())
    }
    pub fn start_call(&mut self, id: &str) -> Result<(), ErrorCode> {
        let call = self.calls.get_mut(id).ok_or(ErrorCode::SessionInvalid)?;
        if call.state != "queued" {
            return Err(ErrorCode::SessionInvalid);
        }
        let lease = self.leases.get_mut(&call.provider.identity()).unwrap();
        if lease.running.is_some() {
            return Err(ErrorCode::RuntimeBusy);
        }
        lease.running = Some(id.into());
        call.state = "running".into();
        Ok(())
    }
    pub fn finish_call(
        &mut self,
        id: &str,
        result: Result<usize, ErrorCode>,
    ) -> Result<(), ErrorCode> {
        let call = self.calls.get_mut(id).ok_or(ErrorCode::SessionInvalid)?;
        let root = call.root_run_id.clone();
        if !matches!(call.state.as_str(), "queued" | "running") {
            return Err(ErrorCode::SessionInvalid);
        }
        let result = result.and_then(|bytes| {
            let budget = self
                .outputs
                .get_mut(&call.root_run_id)
                .ok_or(ErrorCode::Aborted)?;
            budget.1 = budget
                .1
                .checked_sub(bytes)
                .ok_or(ErrorCode::BudgetExceeded)?;
            Ok(())
        });
        call.state = if result.is_ok() {
            "succeeded"
        } else {
            "failed"
        }
        .into();
        call.failure_code = result.as_ref().err().cloned();
        let lease = self.leases.get_mut(&call.provider.identity()).unwrap();
        lease.calls.remove(id);
        if lease.running.as_deref() == Some(id) {
            lease.running = None;
        }
        if self.releasing.contains(&root) {
            self.release_root(&root);
        }
        result
    }
    pub fn diagnostics(&self, root: &str) -> Vec<ServiceCallDiagnostic> {
        self.calls
            .values()
            .filter(|c| c.root_run_id == root)
            .cloned()
            .collect()
    }
    pub fn change_plan(
        &self,
        provider: &PackagePin,
        replacement: Option<PackagePin>,
    ) -> Result<ProviderChangePlan, ErrorCode> {
        if let Some(ref pin) = replacement {
            if pin.identity() != provider.identity() || pin == provider {
                return Err(ErrorCode::DependencyInvalid);
            }
        }
        let mut consumers = BTreeSet::new();
        let mut locks = BTreeSet::new();
        for lock in self.installed.iter().chain(self.accepted.values()) {
            if lock.packages.contains(provider) {
                locks.insert(lock.digest.clone());
                for edge in &lock.reverse_dependencies {
                    if edge.provider == provider.identity() {
                        consumers.extend(edge.consumers.clone());
                    }
                }
            }
        }
        loop {
            let before = consumers.len();
            for lock in self.installed.iter().chain(self.accepted.values()) {
                for edge in &lock.reverse_dependencies {
                    if consumers.contains(&edge.provider) {
                        consumers.extend(edge.consumers.clone());
                    }
                }
            }
            if consumers.len() == before {
                break;
            }
        }
        let mut plan = ProviderChangePlan {
            mode: "plan-only".into(),
            digest: String::new(),
            provider: provider.clone(),
            replacement,
            consumers: consumers.into_iter().collect(),
            affected_locks: locks.into_iter().collect(),
        };
        plan.digest =
            digest(&serde_json::to_value(&plan).map_err(|_| ErrorCode::DependencyInvalid)?);
        Ok(plan)
    }
    /// Internal T0 confirmation: exact presented plan, never an implied run grant.
    pub fn begin_change(
        &mut self,
        plan: ProviderChangePlan,
        confirmation: &str,
        cascade: bool,
    ) -> Result<(), ErrorCode> {
        let current = self.change_plan(&plan.provider, plan.replacement.clone())?;
        if serde_json::to_value(&current).map_err(|_| ErrorCode::DependencyInvalid)?
            != serde_json::to_value(&plan).map_err(|_| ErrorCode::DependencyInvalid)?
            || confirmation != plan.digest
        {
            return Err(ErrorCode::ApprovalRequired);
        }
        if plan.replacement.is_none() && !plan.consumers.is_empty() && !cascade {
            return Err(ErrorCode::DependencyConflict);
        }
        if self.plans.len() >= 64 || self.plans.contains_key(&plan.digest) {
            return Err(ErrorCode::RuntimeBusy);
        }
        let lease = self
            .leases
            .entry(plan.provider.identity())
            .or_insert_with(|| Lease {
                pin: plan.provider.clone(),
                roots: BTreeSet::new(),
                calls: BTreeSet::new(),
                running: None,
                draining: false,
            });
        if lease.draining || lease.pin != plan.provider {
            return Err(ErrorCode::DependencyConflict);
        }
        lease.draining = true;
        self.plans.insert(plan.digest.clone(), plan);
        Ok(())
    }
    pub fn abort_change(&mut self, ticket: &str) {
        if let Some(plan) = self.plans.remove(ticket) {
            if let Some(lease) = self.leases.get_mut(&plan.provider.identity()) {
                lease.draining = false;
            }
        }
    }
    pub fn drained(&self, ticket: &str) -> Result<bool, ErrorCode> {
        let plan = self.plans.get(ticket).ok_or(ErrorCode::ApprovalRequired)?;
        let lease = &self.leases[&plan.provider.identity()];
        Ok(lease.roots.is_empty() && lease.calls.is_empty() && lease.running.is_none())
    }
    /// Backup/migration is a T0 adapter. No work runs until all old users drain.
    pub fn commit_change(
        &mut self,
        ticket: &str,
        migrate: impl FnOnce() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        let plan = self
            .plans
            .get(ticket)
            .ok_or(ErrorCode::ApprovalRequired)?
            .clone();
        let lease = &self.leases[&plan.provider.identity()];
        if !lease.roots.is_empty() || !lease.calls.is_empty() || lease.running.is_some() {
            return Err(ErrorCode::RuntimeBusy);
        }
        if let Err(code) = migrate() {
            self.abort_change(ticket);
            return Err(code);
        }
        let lease = self.leases.get_mut(&plan.provider.identity()).unwrap();
        if let Some(pin) = plan.replacement {
            lease.pin = pin;
        } else {
            self.disabled.insert(plan.provider.identity());
            self.disabled.extend(plan.consumers);
        }
        lease.draining = false;
        self.plans.remove(ticket);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_root_retains_all_pins_until_the_last_child_is_reaped() {
        let catalog = BuiltinCatalog::embedded();
        let mut registry = ServiceRegistry::default();
        registry.set_catalog(&catalog).unwrap();
        let lock = ServiceRegistry::plan(
            &catalog,
            &[
                "plugin-base64-encoder".into(),
                "plugin-random-picker".into(),
            ],
        )
        .unwrap();
        registry.accept("root", lock.clone(), 100).unwrap();
        let child_pin = lock
            .packages
            .iter()
            .find(|pin| pin.id == "plugin-base64-encoder")
            .unwrap()
            .clone();
        let referenced = lock
            .packages
            .iter()
            .find(|pin| pin.id == "plugin-random-picker")
            .unwrap();
        // Lease state only; declaration and broker admission have separate RPC tests.
        registry
            .add_call(ServiceCallDiagnostic {
                run_id: "child".into(),
                parent_run_id: "root".into(),
                root_run_id: "root".into(),
                root_caller: "local-cli".into(),
                consumer: referenced.identity(),
                provider: child_pin,
                service: "fixture".into(),
                operation: "run".into(),
                dependency_lock: lock.digest.clone(),
                deadline: 100.0,
                state: "queued".into(),
                failure_code: None,
            })
            .unwrap();
        registry.release_root("root");
        let plan = registry.change_plan(referenced, None).unwrap();
        registry
            .begin_change(plan.clone(), &plan.digest, false)
            .unwrap();
        assert_eq!(
            registry.commit_change(&plan.digest, || panic!("Child still holds the root lock")),
            Err(ErrorCode::RuntimeBusy)
        );
        assert_eq!(
            registry.finish_call("child", Err(ErrorCode::Aborted)),
            Err(ErrorCode::Aborted)
        );
        registry.commit_change(&plan.digest, || Ok(())).unwrap();
    }
    #[test]
    fn accepted_locks_bound_outputs_and_prevent_tampered_or_early_unload() {
        let catalog = BuiltinCatalog::embedded();
        let mut registry = ServiceRegistry::default();
        registry.set_catalog(&catalog).unwrap();
        let lock = ServiceRegistry::plan(&catalog, &["plugin-base64-encoder".into()]).unwrap();
        registry.accept("root", lock.clone(), 4).unwrap();
        registry.charge_output("root", 3).unwrap();
        assert_eq!(
            registry.charge_output("root", 2),
            Err(ErrorCode::BudgetExceeded)
        );
        let plan = registry.change_plan(&lock.packages[0], None).unwrap();
        let mut forged = plan.clone();
        forged.consumers.push(DependencyIdentity {
            publisher: "flowtools".into(),
            id: "unrelated".into(),
        });
        assert_eq!(
            registry.begin_change(forged, &plan.digest, true),
            Err(ErrorCode::ApprovalRequired)
        );
        registry
            .begin_change(plan.clone(), &plan.digest, false)
            .unwrap();
        assert_eq!(
            registry.accept("another", lock, 4),
            Err(ErrorCode::RuntimeBusy)
        );
        assert_eq!(
            registry.commit_change(&plan.digest, || panic!("Before accepted lease drain")),
            Err(ErrorCode::RuntimeBusy)
        );
        registry.release_root("root");
        registry.commit_change(&plan.digest, || Ok(())).unwrap();
        assert!(!registry.enabled(&plan.provider.identity()));
        assert!(!registry.retained.is_empty());
    }
}
