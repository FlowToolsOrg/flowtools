//! Resource authorization is another closed grant variant in the same broker.
use super::*;
use crate::resources::{ResourceGrant, ResourceRight};
impl CapabilityBroker {
    pub fn approve_resources(
        &mut self,
        identity: CommandIdentity,
        grant: ResourceGrant,
    ) -> Result<u32, ErrorCode> {
        if !grant.valid()
            || grant.target.caller() != identity.caller
            || grant.plugin_id != identity.plugin_id
            || grant.command_id != identity.command_id
            || grant.package_digest != identity.package_digest
        {
            return Err(ErrorCode::InvalidRequest);
        }
        let policy = self.policies.entry(identity).or_insert(Policy {
            epoch: 0,
            grant: None,
            resource: None,
        });
        policy.epoch = policy.epoch.checked_add(1).ok_or(ErrorCode::RuntimeBusy)?;
        policy.grant = None;
        policy.resource = Some(grant);
        Ok(policy.epoch)
    }
    pub fn restore_resource_policy(
        &mut self,
        identity: CommandIdentity,
        epoch: u32,
        grant: Option<ResourceGrant>,
    ) -> Result<(), ErrorCode> {
        if epoch == 0 {
            return Err(ErrorCode::StoreCorrupt);
        }
        if let Some(grant) = &grant {
            Self::default().approve_resources(identity.clone(), grant.clone())?;
        }
        self.policies.insert(
            identity,
            Policy {
                epoch,
                grant: None,
                resource: grant,
            },
        );
        Ok(())
    }
    pub fn recheck_resource(
        &self,
        session: &BrokerSession,
        slot: &str,
        right: ResourceRight,
        at: f64,
    ) -> Result<CommandIdentity, ErrorCode> {
        self.check_session(session, at)?;
        let binding = &self.bindings[&session.id];
        // E07 resource handoff is not implemented. No root/provider borrowing.
        if !binding.parents.is_empty() {
            return Err(ErrorCode::ScopeDenied);
        }
        if !crate::resources::declared(&binding.command, slot, right) {
            return Err(ErrorCode::CapabilityUndeclared);
        }
        let grant = self
            .policies
            .get(&binding.identity)
            .and_then(|p| p.resource.as_ref())
            .ok_or(ErrorCode::ApprovalRequired)?;
        if !grant.allows(slot, right) {
            return Err(ErrorCode::ScopeDenied);
        }
        Ok(binding.identity.clone())
    }
    pub fn invoke_resource(
        &mut self,
        session: &BrokerSession,
        slot: &str,
        right: ResourceRight,
        at: f64,
    ) -> Result<CommandIdentity, ErrorCode> {
        let identity = self.recheck_resource(session, slot, right, at)?;
        self.check_call_budget(session)?;
        self.bindings.get_mut(&session.id).unwrap().calls += 1;
        Ok(identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> (CommandIdentity, Value, ResourceGrant) {
        let catalog = crate::catalog::BuiltinCatalog::embedded();
        let (manifest, command) = catalog.command("plugin-base64-encoder", "run").unwrap();
        let identity = CommandIdentity::from_manifest("local-cli", manifest, command);
        let mut command = command.clone();
        command["capabilityProfile"] = json!(crate::native_contract::NATIVE_CAPABILITY_PROFILE);
        command["effects"] = json!(["file-read"]);
        command["permissions"] = json!([{"capability":"fs","operations":["read","stat","list"],"scopes":[{"kind":"resource","slot":"source"}]}]);
        let grant = ResourceGrant {
            format_version: 2,
            capability_profile: crate::native_contract::NATIVE_CAPABILITY_PROFILE.into(),
            plugin_id: identity.plugin_id.clone(),
            command_id: identity.command_id.clone(),
            target: crate::protocol::GrantTarget::Cli,
            package_digest: identity.package_digest.clone(),
            scopes: vec![crate::resources::ResourceScope {
                slot: "source".into(),
                operations: vec![ResourceRight::Read, ResourceRight::Stat],
            }],
            expires_at: 10_000.0,
            max_calls: 1,
        };
        (identity, command, grant)
    }
    #[test]
    fn resource_grants_keep_epochs_and_last_call_recheck_without_legacy_scope_expansion() {
        let (identity, command, grant) = fixture();
        let mut broker = CapabilityBroker::default();
        assert_eq!(
            broker.authorize_command(&identity, &command, 100.0),
            Err(ErrorCode::ApprovalRequired)
        );
        broker
            .approve_resources(identity.clone(), grant.clone())
            .unwrap();
        let bound = broker
            .bind(identity.clone(), command.clone(), 1000.0, 100.0)
            .unwrap();
        assert_eq!(
            broker.invoke_resource(&bound, "source", ResourceRight::List, 100.0),
            Err(ErrorCode::ScopeDenied)
        );
        broker
            .invoke_resource(&bound, "source", ResourceRight::Read, 100.0)
            .unwrap();
        assert_eq!(
            broker.invoke_resource(&bound, "source", ResourceRight::Read, 100.0),
            Err(ErrorCode::BudgetExceeded)
        );
        broker
            .recheck_resource(&bound, "source", ResourceRight::Read, 100.0)
            .unwrap();
        broker.revoke(&identity).unwrap();
        assert_eq!(
            broker.recheck_resource(&bound, "source", ResourceRight::Read, 100.0),
            Err(ErrorCode::GrantRevoked)
        );
        broker.approve_resources(identity, grant).unwrap();
        assert_eq!(
            broker.recheck_resource(&bound, "source", ResourceRight::Read, 100.0),
            Err(ErrorCode::GrantRevoked)
        );
    }
}
