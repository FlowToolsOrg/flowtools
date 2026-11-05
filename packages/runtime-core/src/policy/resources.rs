//! Format-2 resource records in the existing grants table and DataStore writer.
use super::*;
use crate::resources::{ResourceGrant, ResourcePermission};
use rusqlite::Connection;

fn key(identity: &CommandIdentity) -> Result<String, ErrorCode> {
    serde_json::to_string(identity).map_err(|_| ErrorCode::StorageFailed)
}
pub(super) fn read(
    connection: &Connection,
    identity: &CommandIdentity,
) -> Result<Option<(u32, String)>, ErrorCode> {
    let row:Option<(u32,Option<String>)>=connection.query_row("SELECT epoch,CASE WHEN typeof(state)='text' AND length(CAST(state AS BLOB))<=32768 THEN state ELSE NULL END FROM grants WHERE identity=?1",[key(identity)?],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|ErrorCode::StorageFailed)?;
    row.map(|(epoch, text)| Ok((epoch, text.ok_or(ErrorCode::StoreCorrupt)?)))
        .transpose()
}
pub(super) fn save_resource(
    connection: &Connection,
    record: &ResourcePermission,
    expected: Option<&str>,
) -> Result<String, ErrorCode> {
    let previous = read(connection, &record.identity)?;
    if previous.as_ref().map(|(_, text)| text.as_str()) != expected {
        return Err(ErrorCode::RevisionConflict);
    }
    if let Some((epoch, text)) = &previous {
        let old: ResourcePermission =
            serde_json::from_str(text).map_err(|_| ErrorCode::StoreCorrupt)?;
        if old.epoch != *epoch || old.identity != record.identity || old.format_version != 2 {
            return Err(ErrorCode::StoreCorrupt);
        }
    }
    let text = serde_json::to_string(record).map_err(|_| ErrorCode::StorageFailed)?;
    if text.len() > 32768 {
        return Err(ErrorCode::BudgetExceeded);
    }
    let before = connection.total_changes();
    let rows=connection.execute("INSERT INTO grants(identity,epoch,state) VALUES(?1,?2,?3) ON CONFLICT(identity) DO UPDATE SET epoch=excluded.epoch,state=excluded.state",params![key(&record.identity)?,record.epoch,&text]).map_err(|_|ErrorCode::StorageFailed)?;
    if rows != 1
        || connection.total_changes().checked_sub(before) != Some(1)
        || read(connection, &record.identity)? != Some((record.epoch, text.clone()))
    {
        return Err(ErrorCode::StorageFailed);
    }
    Ok(text)
}
impl PolicyStore {
    pub(super) fn verify_resource_rows(&self, connection: &Connection) -> Result<(), ErrorCode> {
        self.verify_unchanged_resource_rows(connection, &[])
    }
    pub(super) fn verify_unchanged_resource_rows(
        &self,
        connection: &Connection,
        changed: &[ResourcePermission],
    ) -> Result<(), ErrorCode> {
        for record in &self.resource_records {
            if changed.iter().any(|r| r.identity == record.identity) {
                continue;
            }
            let expected = self
                .resource_predecessors
                .get(&record.identity)
                .ok_or(ErrorCode::StoreCorrupt)?;
            if read(connection, &record.identity)? != Some((record.epoch, expected.clone())) {
                return Err(ErrorCode::RevisionConflict);
            }
        }
        Ok(())
    }
    pub(super) fn resource_identity(
        &self,
        grant: &ResourceGrant,
    ) -> Result<CommandIdentity, ErrorCode> {
        let (manifest, command) = self.catalog.command(&grant.plugin_id, &grant.command_id)?;
        Ok(CommandIdentity::from_manifest(
            grant.target.caller(),
            manifest,
            command,
        ))
    }
    pub(super) fn validate_resource(&self, grant: &ResourceGrant) -> Result<(), ErrorCode> {
        if !grant.valid() {
            return Err(ErrorCode::InvalidRequest);
        }
        let (manifest, command) = self.catalog.command(&grant.plugin_id, &grant.command_id)?;
        if manifest["formatVersion"] != 2
            || !crate::resources::readonly_command(command)
            || command["headless"] != true
            || command["interaction"] == "required"
            || command["supportsColdStart"] != false
        {
            return Err(ErrorCode::CapabilityUnavailable);
        }
        let identity = self.resource_identity(grant)?;
        if identity.package_digest != grant.package_digest
            || !grant.scopes.iter().all(|scope| {
                scope
                    .operations
                    .iter()
                    .all(|right| crate::resources::declared(command, &scope.slot, *right))
            })
        {
            return Err(ErrorCode::ScopeDenied);
        }
        CapabilityBroker::default().approve_resources(identity, grant.clone())?;
        Ok(())
    }
    pub fn verify_resource_record(
        &self,
        store: &DataStore,
        identity: &CommandIdentity,
    ) -> Result<(), ErrorCode> {
        let record = self
            .resource_records
            .iter()
            .find(|record| &record.identity == identity)
            .ok_or(ErrorCode::ApprovalRequired)?;
        let expected = self
            .resource_predecessors
            .get(identity)
            .ok_or(ErrorCode::StoreCorrupt)?;
        if read(&store.connection, identity)? != Some((record.epoch, expected.clone())) {
            return Err(ErrorCode::RevisionConflict);
        }
        Ok(())
    }
    pub fn grant_resources(
        &mut self,
        store: &mut DataStore,
        grant: ResourceGrant,
    ) -> Result<ResourcePermission, ErrorCode> {
        self.validate_resource(&grant)?;
        let at = crate::runtime::now();
        if grant.expires_at <= at || grant.expires_at > at + 86_400_000.0 {
            return Err(ErrorCode::InvalidRequest);
        }
        let identity = self.resource_identity(&grant)?;
        if self.records.len() + self.resource_records.len() >= 256
            && !self.resource_records.iter().any(|r| r.identity == identity)
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let epoch = self
            .resource_records
            .iter()
            .find(|r| r.identity == identity)
            .map_or(Ok(1), |r| {
                r.epoch.checked_add(1).ok_or(ErrorCode::BudgetExceeded)
            })?;
        let record = ResourcePermission {
            format_version: 2,
            identity,
            epoch,
            grant: Some(grant),
        };
        let tx = store
            .connection
            .transaction()
            .map_err(|_| ErrorCode::StorageFailed)?;
        self.verify_resource_rows(&tx)?;
        let before = tx.total_changes();
        let text = save_resource(
            &tx,
            &record,
            self.resource_predecessors
                .get(&record.identity)
                .map(String::as_str),
        )?;
        Self::audit_decisions(
            &tx,
            "resource-grant",
            vec![
                serde_json::json!({"identityDigest":digest(&serde_json::to_value(&record.identity).unwrap()),"epoch":record.epoch,"approved":true}),
            ],
        )?;
        if tx.total_changes().checked_sub(before) != Some(2)
            || read(&tx, &record.identity)? != Some((record.epoch, text.clone()))
        {
            return Err(ErrorCode::StorageFailed);
        }
        self.verify_unchanged_resource_rows(&tx, std::slice::from_ref(&record))?;
        tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
        self.resource_records
            .retain(|r| r.identity != record.identity);
        self.resource_records.push(record.clone());
        self.resource_predecessors
            .insert(record.identity.clone(), text);
        Ok(record)
    }
    pub(super) fn resource_revoke_changes(
        &self,
        key: &PermissionKey,
    ) -> Result<Vec<ResourcePermission>, ErrorCode> {
        self.resource_records
            .iter()
            .filter(|r| {
                r.identity.caller == key.target.caller()
                    && r.identity.plugin_id == key.plugin_id
                    && r.identity.command_id == key.command_id
            })
            .cloned()
            .map(|mut r| {
                r.epoch = r.epoch.checked_add(1).ok_or(ErrorCode::BudgetExceeded)?;
                r.grant = None;
                Ok(r)
            })
            .collect()
    }
    pub(super) fn save_resource_changes(
        &self,
        connection: &Connection,
        records: &[ResourcePermission],
    ) -> Result<Vec<(CommandIdentity, String)>, ErrorCode> {
        records
            .iter()
            .map(|r| {
                save_resource(
                    connection,
                    r,
                    self.resource_predecessors
                        .get(&r.identity)
                        .map(String::as_str),
                )
                .map(|text| (r.identity.clone(), text))
            })
            .collect()
    }
    pub(super) fn apply_resource_changes(
        &mut self,
        records: Vec<ResourcePermission>,
        texts: Vec<(CommandIdentity, String)>,
    ) {
        for record in records {
            self.resource_records
                .retain(|r| r.identity != record.identity);
            self.resource_records.push(record);
        }
        self.resource_predecessors.extend(texts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> (DataStore, PolicyStore, ResourceGrant) {
        let mut manifests: Vec<_> = BuiltinCatalog::embedded()
            .list()
            .map(|(_, m)| m.clone())
            .collect();
        let mut resource = manifests[0].clone();
        resource["id"] = json!("fixture-resource");
        resource["formatVersion"] = json!(2);
        resource["capabilityProfile"] = json!(crate::native_contract::NATIVE_CAPABILITY_PROFILE);
        resource["commands"][0]["capabilityProfile"] =
            json!(crate::native_contract::NATIVE_CAPABILITY_PROFILE);
        resource["commands"][0]["effects"] = json!(["file-read"]);
        resource["commands"][0]["permissions"] = json!([{"capability":"fs","operations":["read","stat"],"scopes":[{"kind":"resource","slot":"source"}]}]);
        resource["commands"][0]["supportsColdStart"] = json!(false);
        let grant = ResourceGrant {
            format_version: 2,
            capability_profile: crate::native_contract::NATIVE_CAPABILITY_PROFILE.into(),
            plugin_id: "fixture-resource".into(),
            command_id: "run".into(),
            target: GrantTarget::Cli,
            package_digest: digest(&resource),
            scopes: vec![crate::resources::ResourceScope {
                slot: "source".into(),
                operations: vec![crate::resources::ResourceRight::Read],
            }],
            expires_at: crate::runtime::now() + 60_000.0,
            max_calls: 64,
        };
        manifests.push(resource);
        let catalog = BuiltinCatalog::from_host_manifests(manifests).unwrap();
        let mut store = DataStore::memory().unwrap();
        let policy =
            PolicyStore::load_snapshot(&mut store, &catalog.snapshot_digest(), catalog).unwrap();
        (store, policy, grant)
    }
    #[test]
    fn resource_policy_shares_writer_but_never_rewrites_v1_or_survives_revoke_epoch() {
        let (mut store, mut policy, grant) = fixture();
        let old = super::super::tests::grant();
        policy.grant(&mut store, old).unwrap();
        let original: Vec<(String, String)> = store
            .connection
            .prepare("SELECT identity,state FROM grants ORDER BY identity")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let resource = policy.grant_resources(&mut store, grant.clone()).unwrap();
        policy
            .verify_resource_record(&store, &resource.identity)
            .unwrap();
        for (id, text) in &original {
            assert_eq!(
                store
                    .connection
                    .query_row("SELECT state FROM grants WHERE identity=?1", [id], |r| {
                        r.get::<_, String>(0)
                    })
                    .unwrap(),
                *text
            );
        }
        let catalog = policy.catalog.clone();
        let loaded =
            PolicyStore::load_snapshot(&mut store, &catalog.snapshot_digest(), catalog).unwrap();
        assert_eq!(loaded.resource_records.len(), 1);
        assert_eq!(loaded.resource_records[0].epoch, resource.epoch);
        policy
            .revoke(
                &mut store,
                PermissionKey {
                    plugin_id: grant.plugin_id.clone(),
                    command_id: grant.command_id.clone(),
                    target: grant.target.clone(),
                },
            )
            .unwrap();
        let revoked = policy
            .resource_records
            .iter()
            .find(|r| r.identity == resource.identity)
            .unwrap();
        assert!(revoked.grant.is_none());
        assert!(revoked.epoch > resource.epoch);
    }
    #[test]
    fn resource_catalog_commit_refuses_metadata_trigger_substitution_and_rolls_back_old_bytes() {
        let (mut store, mut policy, grant) = fixture();
        let record = policy.grant_resources(&mut store, grant).unwrap();
        let before = read(&store.connection, &record.identity).unwrap();
        store.connection.execute_batch("CREATE TRIGGER substitute_catalog AFTER UPDATE OF value ON core_metadata WHEN NEW.key='catalog-digest' BEGIN UPDATE grants SET state='{}'; END;").unwrap();
        let catalog = policy.catalog.clone();
        assert!(PolicyStore::load_snapshot(&mut store, "new-catalog", catalog).is_err());
        assert_eq!(read(&store.connection, &record.identity).unwrap(), before);
    }
    #[test]
    fn resource_policy_refuses_trigger_substitution_and_cached_predecessor_drift() {
        let (mut store, mut policy, grant) = fixture();
        let record = policy.grant_resources(&mut store, grant.clone()).unwrap();
        let before = read(&store.connection, &record.identity).unwrap();
        store.connection.execute_batch("CREATE TRIGGER substitute_resource AFTER UPDATE ON grants BEGIN UPDATE grants SET state='{}' WHERE identity=NEW.identity; END;").unwrap();
        assert!(policy.grant_resources(&mut store, grant.clone()).is_err());
        assert_eq!(read(&store.connection, &record.identity).unwrap(), before);
        store
            .connection
            .execute_batch("DROP TRIGGER substitute_resource;")
            .unwrap();
        store
            .connection
            .execute("UPDATE grants SET state=state||' '", [])
            .unwrap();
        assert_eq!(
            policy.verify_resource_record(&store, &record.identity),
            Err(ErrorCode::RevisionConflict)
        );
        assert!(policy.grant_resources(&mut store, grant).is_err());
    }
}
