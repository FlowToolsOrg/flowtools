//! Persistent Host policy in the same single-writer database as plugin data.
use crate::{
    broker::{CapabilityBroker, CommandIdentity, Grant},
    catalog::{digest, BuiltinCatalog},
    data::DataStore,
    protocol::*,
};
use rusqlite::{params, OptionalExtension};
mod resources;

pub struct PolicyStore {
    pub records: Vec<PermissionRecord>,
    pub cold_start: bool,
    catalog: BuiltinCatalog,
    pub resource_records: Vec<crate::resources::ResourcePermission>,
    resource_predecessors: std::collections::HashMap<CommandIdentity, String>,
}
impl PolicyStore {
    pub fn set_catalog(&mut self, catalog: BuiltinCatalog) {
        self.catalog = catalog;
    }
    fn audit(
        connection: &rusqlite::Connection,
        action: &str,
        records: &[PermissionRecord],
    ) -> Result<(), ErrorCode> {
        Self::audit_decisions(connection,action,records.iter().map(|r|serde_json::json!({"identityDigest":digest(&serde_json::to_value(&r.identity).expect("identity")),"epoch":r.epoch,"approved":r.grant.is_some()})).collect())
    }
    fn audit_decisions(
        connection: &rusqlite::Connection,
        action: &str,
        decisions: Vec<serde_json::Value>,
    ) -> Result<(), ErrorCode> {
        let text: Option<String> = connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='policy-audit'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| ErrorCode::StorageFailed)?;
        let mut events: Vec<serde_json::Value> = text
            .map(|s| serde_json::from_str(&s).map_err(|_| ErrorCode::StoreCorrupt))
            .transpose()?
            .unwrap_or_default();
        if events.len() > 256 {
            return Err(ErrorCode::StoreCorrupt);
        }
        let sequence = events
            .last()
            .and_then(|e| e["sequence"].as_u64())
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ErrorCode::BudgetExceeded)?;
        events.push(serde_json::json!({"sequence":sequence,"action":action,"at":crate::runtime::now(),"decisions":decisions }));
        if events.len() > 256 {
            events.remove(0);
        }
        connection.execute("INSERT INTO core_metadata(key,value) VALUES('policy-audit',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(&events).map_err(|_|ErrorCode::StorageFailed)?]).map_err(|_|ErrorCode::StorageFailed)?;
        Ok(())
    }
    pub fn import(&mut self, store: &mut DataStore, import: PolicyImport) -> Result<(), ErrorCode> {
        if import.format_version != 1 || import.grants.len() > 32 {
            return Err(ErrorCode::InvalidRequest);
        }
        let mut changed = self.records.clone();
        let mut identities = std::collections::HashSet::new();
        for grant in import.grants {
            self.validate(&grant)?;
            if grant.expires_at <= crate::runtime::now()
                || grant.expires_at > crate::runtime::now() + 31_536_000_000.0
            {
                return Err(ErrorCode::InvalidRequest);
            }
            let identity = self.identity(&grant)?;
            if !identities.insert(identity.clone()) {
                return Err(ErrorCode::InvalidRequest);
            }
            let epoch = changed
                .iter()
                .find(|r| r.identity == identity)
                .map_or(Ok(1), |r| {
                    r.epoch.checked_add(1).ok_or(ErrorCode::BudgetExceeded)
                })?;
            changed.retain(|r| r.identity != identity);
            changed.push(PermissionRecord {
                identity,
                epoch,
                grant: Some(grant),
            });
        }
        if changed.len() + self.resource_records.len() > 256 {
            return Err(ErrorCode::BudgetExceeded);
        }
        let tx = store
            .connection
            .transaction()
            .map_err(|_| ErrorCode::StorageFailed)?;
        self.verify_resource_rows(&tx)?;
        let native_before = tx.total_changes();
        for record in &changed {
            Self::save(&tx, record)?;
        }
        tx.execute("INSERT INTO core_metadata(key,value) VALUES('cold-start',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[if import.cold_start {"true"} else {"false"}]).map_err(|_|ErrorCode::StorageFailed)?;
        Self::audit(&tx, "import", &changed)?;
        self.verify_resource_rows(&tx)?;
        if !self.resource_records.is_empty()
            && tx.total_changes().checked_sub(native_before) != Some(changed.len() as u64 + 2)
        {
            return Err(ErrorCode::StorageFailed);
        }
        tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
        self.records = changed;
        self.cold_start = import.cold_start;
        Ok(())
    }
    pub fn load(store: &mut DataStore, catalog_digest: &str) -> Result<Self, ErrorCode> {
        Self::load_snapshot(store, catalog_digest, BuiltinCatalog::embedded())
    }
    pub fn load_snapshot(
        store: &mut DataStore,
        catalog_digest: &str,
        catalog: BuiltinCatalog,
    ) -> Result<Self, ErrorCode> {
        let previous: Option<String> = store
            .connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='catalog-digest'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| ErrorCode::StorageFailed)?;
        let mut resource_records = Vec::new();
        let mut resource_predecessors = std::collections::HashMap::new();
        let mut records: Vec<PermissionRecord> = store
            .connection
            .prepare("SELECT epoch,CASE WHEN typeof(state)='text' AND length(CAST(state AS BLOB))<=32768 THEN state ELSE NULL END,identity FROM grants ORDER BY identity LIMIT 257")
            .map_err(|_| ErrorCode::StorageFailed)?
            .query_map([], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, Option<String>>(1)?,row.get::<_,String>(2)?))
            })
            .map_err(|_| ErrorCode::StorageFailed)?
            .map(|row| {
                let (epoch, state,identity_key) = row.map_err(|_| ErrorCode::StoreCorrupt)?;
                let state=state.ok_or(ErrorCode::StoreCorrupt)?;
                let value:serde_json::Value=serde_json::from_str(&state).map_err(|_|ErrorCode::StoreCorrupt)?;
                if value.get("formatVersion").is_some() {
                    let value=crate::packages::strict_json(state.as_bytes()).map_err(|_|ErrorCode::StoreCorrupt)?;
                    let record:crate::resources::ResourcePermission=serde_json::from_value(value).map_err(|_|ErrorCode::StoreCorrupt)?;
                    if epoch==0||epoch!=record.epoch||record.format_version!=2||serde_json::to_string(&record.identity).map_err(|_|ErrorCode::StoreCorrupt)?!=identity_key {return Err(ErrorCode::StoreCorrupt);}
                    if resource_predecessors.insert(record.identity.clone(),state).is_some() {return Err(ErrorCode::StoreCorrupt);}
                    resource_records.push(record);return Ok(None);
                }
                let record: PermissionRecord =
                    serde_json::from_str(&state).map_err(|_| ErrorCode::StoreCorrupt)?;
                if epoch == 0 || epoch != record.epoch {
                    return Err(ErrorCode::StoreCorrupt);
                }
                Ok(Some(record))
            })
            .collect::<Result<Vec<_>, _>>()?.into_iter().flatten().collect();
        if records.len() + resource_records.len() > 256 {
            return Err(ErrorCode::StoreCorrupt);
        }
        if previous.as_deref() != Some(catalog_digest) {
            let tx = store
                .connection
                .transaction()
                .map_err(|_| ErrorCode::StorageFailed)?;
            let native_before = tx.total_changes();
            for record in &mut records {
                record.epoch = record
                    .epoch
                    .checked_add(1)
                    .ok_or(ErrorCode::BudgetExceeded)?;
                record.grant = None;
                Self::save(&tx, record)?;
            }
            for record in &mut resource_records {
                record.epoch = record
                    .epoch
                    .checked_add(1)
                    .ok_or(ErrorCode::BudgetExceeded)?;
                record.grant = None;
                let text = resources::save_resource(
                    &tx,
                    record,
                    resource_predecessors
                        .get(&record.identity)
                        .map(String::as_str),
                )?;
                resource_predecessors.insert(record.identity.clone(), text);
            }
            if resource_records.is_empty() {
                Self::audit(&tx, "catalog-change", &records)?;
            } else {
                let mut decisions:Vec<_>=records.iter().map(|r|serde_json::json!({"identityDigest":digest(&serde_json::to_value(&r.identity).unwrap()),"epoch":r.epoch,"approved":false})).collect();
                decisions.extend(resource_records.iter().map(|r|serde_json::json!({"identityDigest":digest(&serde_json::to_value(&r.identity).unwrap()),"epoch":r.epoch,"approved":false})));
                Self::audit_decisions(&tx, "catalog-change", decisions)?;
            }
            tx.execute("INSERT INTO core_metadata(key,value) VALUES('catalog-digest',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[catalog_digest]).map_err(|_|ErrorCode::StorageFailed)?;
            if !resource_records.is_empty() {
                if tx.total_changes().checked_sub(native_before)
                    != Some((records.len() + resource_records.len()) as u64 + 2)
                {
                    return Err(ErrorCode::StorageFailed);
                }
                for record in &resource_records {
                    if resources::read(&tx, &record.identity)?
                        != Some((
                            record.epoch,
                            resource_predecessors[&record.identity].clone(),
                        ))
                    {
                        return Err(ErrorCode::StorageFailed);
                    }
                }
            }
            tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
        }
        let mut policy = Self {
            records: vec![],
            cold_start: false,
            catalog,
            resource_records: Vec::new(),
            resource_predecessors: std::collections::HashMap::new(),
        };
        for record in &records {
            if let Some(ref grant) = record.grant {
                let identity = policy.identity(grant)?;
                if identity != record.identity {
                    return Err(ErrorCode::StoreCorrupt);
                }
                policy.validate(grant)?;
            }
        }
        for record in &resource_records {
            if let Some(grant) = &record.grant {
                if policy.resource_identity(grant)? != record.identity {
                    return Err(ErrorCode::StoreCorrupt);
                }
                policy.validate_resource(grant)?;
            }
        }
        let cold_start = store
            .connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='cold-start'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| ErrorCode::StorageFailed)?
            .is_some_and(|v| v == "true");
        policy.records = records;
        policy.cold_start = cold_start;
        policy.resource_records = resource_records;
        policy.resource_predecessors = resource_predecessors;
        Ok(policy)
    }
    fn identity(&self, grant: &PermissionGrant) -> Result<CommandIdentity, ErrorCode> {
        let (manifest, command) = self
            .catalog
            .permission_operation(&grant.plugin_id, &grant.command_id)?;
        let mut identity = CommandIdentity::from_manifest(grant.target.caller(), manifest, command);
        identity.command_id = grant.command_id.clone();
        Ok(identity)
    }
    fn validate(&self, grant: &PermissionGrant) -> Result<(), ErrorCode> {
        let (_, command) = self
            .catalog
            .permission_operation(&grant.plugin_id, &grant.command_id)?;
        if self.identity(grant)?.package_digest != grant.package_digest {
            return Err(ErrorCode::InvalidRequest);
        }
        if command.get("capabilityProfile").is_some() {
            if !crate::native_contract::command_valid(command) {
                return Err(ErrorCode::InvalidRequest);
            }
            if !command["effects"].as_array().is_some_and(Vec::is_empty)
                || !command["permissions"].as_array().is_some_and(Vec::is_empty)
            {
                return Err(ErrorCode::CapabilityUnavailable);
            }
        }
        if !grant.expires_at.is_finite()
            || (grant.cold_start && command["supportsColdStart"] != true)
            || command["headless"] != true
            || command["interaction"] == "required"
            || grant.effects.iter().any(|e| {
                !command["effects"]
                    .as_array()
                    .is_some_and(|v| v.iter().any(|d| d == e))
            })
        {
            return Err(ErrorCode::InvalidRequest);
        }
        CapabilityBroker::default().approve(self.identity(grant)?, Self::capability(grant))?;
        Ok(())
    }
    pub fn capability(grant: &PermissionGrant) -> Grant {
        Grant {
            effects: grant.effects.clone(),
            scopes: grant.scopes.clone(),
            expires_at: grant.expires_at,
            max_calls: grant.max_calls,
        }
    }
    fn save(connection: &rusqlite::Connection, record: &PermissionRecord) -> Result<(), ErrorCode> {
        let key = serde_json::to_string(&record.identity).map_err(|_| ErrorCode::StorageFailed)?;
        let value = serde_json::to_string(record).map_err(|_| ErrorCode::StorageFailed)?;
        connection.execute("INSERT INTO grants(identity,epoch,state) VALUES(?1,?2,?3) ON CONFLICT(identity) DO UPDATE SET epoch=excluded.epoch,state=excluded.state",params![key,record.epoch,value]).map_err(|_|ErrorCode::StorageFailed)?;
        Ok(())
    }
    pub fn grant(
        &mut self,
        store: &mut DataStore,
        grant: PermissionGrant,
    ) -> Result<PermissionRecord, ErrorCode> {
        if grant.expires_at <= crate::runtime::now()
            || grant.expires_at > crate::runtime::now() + 31_536_000_000.0
        {
            return Err(ErrorCode::InvalidRequest);
        }
        self.validate(&grant)?;
        let identity = self.identity(&grant)?;
        if self.records.len() + self.resource_records.len() >= 256
            && !self.records.iter().any(|r| r.identity == identity)
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let epoch = self
            .records
            .iter()
            .find(|r| r.identity == identity)
            .map_or(Ok(1), |r| {
                r.epoch.checked_add(1).ok_or(ErrorCode::BudgetExceeded)
            })?;
        let record = PermissionRecord {
            identity,
            epoch,
            grant: Some(grant),
        };
        let tx = store
            .connection
            .transaction()
            .map_err(|_| ErrorCode::StorageFailed)?;
        self.verify_resource_rows(&tx)?;
        let native_before = tx.total_changes();
        Self::save(&tx, &record)?;
        Self::audit(&tx, "grant", std::slice::from_ref(&record))?;
        self.verify_resource_rows(&tx)?;
        if !self.resource_records.is_empty()
            && tx.total_changes().checked_sub(native_before) != Some(2)
        {
            return Err(ErrorCode::StorageFailed);
        }
        tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
        self.records.retain(|r| r.identity != record.identity);
        self.records.push(record.clone());
        Ok(record)
    }
    pub fn revoke(&mut self, store: &mut DataStore, key: PermissionKey) -> Result<(), ErrorCode> {
        // Revoke all versions for this stable command identity, including rollback.
        let mut changed = self.records.clone();
        let resource_changes = self.resource_revoke_changes(&key)?;
        for record in &mut changed {
            if record.identity.caller == key.target.caller()
                && record.identity.plugin_id == key.plugin_id
                && record.identity.command_id == key.command_id
            {
                record.epoch = record
                    .epoch
                    .checked_add(1)
                    .ok_or(ErrorCode::BudgetExceeded)?;
                record.grant = None;
            }
        }
        let tx = store
            .connection
            .transaction()
            .map_err(|_| ErrorCode::StorageFailed)?;
        self.verify_resource_rows(&tx)?;
        let native_before = tx.total_changes();
        for record in &changed {
            Self::save(&tx, record)?;
        }
        let resource_texts = self.save_resource_changes(&tx, &resource_changes)?;
        if resource_changes.is_empty() {
            Self::audit(&tx, "revoke", &changed)?;
        } else {
            let mut decisions:Vec<_>=changed.iter().map(|r|serde_json::json!({"identityDigest":digest(&serde_json::to_value(&r.identity).unwrap()),"epoch":r.epoch,"approved":r.grant.is_some()})).collect();
            decisions.extend(resource_changes.iter().map(|r|serde_json::json!({"identityDigest":digest(&serde_json::to_value(&r.identity).unwrap()),"epoch":r.epoch,"approved":false})));
            Self::audit_decisions(&tx, "revoke", decisions)?;
        }
        self.verify_unchanged_resource_rows(&tx, &resource_changes)?;
        if !self.resource_records.is_empty()
            && tx.total_changes().checked_sub(native_before)
                != Some((changed.len() + resource_changes.len()) as u64 + 1)
        {
            return Err(ErrorCode::StorageFailed);
        }
        tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
        self.records = changed;
        self.apply_resource_changes(resource_changes, resource_texts);
        Ok(())
    }
    pub fn set(&mut self, store: &mut DataStore, policy: BootstrapPolicy) -> Result<(), ErrorCode> {
        let tx = store
            .connection
            .transaction()
            .map_err(|_| ErrorCode::StorageFailed)?;
        self.verify_resource_rows(&tx)?;
        let native_before = tx.total_changes();
        tx.execute("INSERT INTO core_metadata(key,value) VALUES('cold-start',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[if policy.cold_start {"true"}else{"false"}]).map_err(|_|ErrorCode::StorageFailed)?;
        Self::audit(&tx, "bootstrap-policy", &[])?;
        self.verify_resource_rows(&tx)?;
        if !self.resource_records.is_empty()
            && tx.total_changes().checked_sub(native_before) != Some(2)
        {
            return Err(ErrorCode::StorageFailed);
        }
        tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
        self.cold_start = policy.cold_start;
        Ok(())
    }
    pub fn catalog_digest() -> String {
        digest(&serde_json::json!(BuiltinCatalog::embedded()
            .list()
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DataStore;
    use serde_json::json;
    #[test]
    fn contract_upgrade_preserves_v1_grant_bytes_and_rejects_v2_policy_import() {
        let mut store = DataStore::memory().unwrap();
        let mut policy = PolicyStore::load(&mut store, "same-catalog").unwrap();
        policy.grant(&mut store, grant()).unwrap();
        let read = |store: &DataStore| -> Vec<(String, u32, String)> {
            store
                .connection
                .prepare("SELECT identity,epoch,state FROM grants ORDER BY identity")
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        let before = read(&store);
        assert_eq!(crate::data::SCHEMA_VERSION, 2);
        let mut policy = PolicyStore::load(&mut store, "same-catalog").unwrap();
        assert_eq!(read(&store), before);
        assert!(policy
            .import(
                &mut store,
                PolicyImport {
                    format_version: 2,
                    cold_start: true,
                    grants: vec![grant()]
                }
            )
            .is_err());
        assert_eq!(read(&store), before);
        assert!(!policy.cold_start);
        let typed = json!({"kind":"resource","slot":"source"});
        assert!(serde_json::from_value::<crate::broker::Scope>(typed).is_err());
    }

    pub(super) fn grant() -> PermissionGrant {
        PermissionGrant {
            plugin_id: "plugin-base64-encoder".into(),
            command_id: "run".into(),
            target: GrantTarget::Cli,
            package_digest: CommandIdentity::from_manifest(
                "local-cli",
                BuiltinCatalog::embedded()
                    .command("plugin-base64-encoder", "run")
                    .unwrap()
                    .0,
                BuiltinCatalog::embedded()
                    .command("plugin-base64-encoder", "run")
                    .unwrap()
                    .1,
            )
            .package_digest,
            effects: vec![],
            scopes: vec![],
            expires_at: crate::runtime::now() + 3_600_000.0,
            max_calls: 16,
            cold_start: false,
            background: false,
        }
    }

    #[test]
    fn grants_survive_restart_revoke_and_catalog_rollback_without_resurrection() {
        let path = std::env::temp_dir().join(format!(
            "flowtools-validation-policy-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let mut store = DataStore::open(&path).unwrap();
        let mut policy = PolicyStore::load(&mut store, "catalog-one").unwrap();
        assert!(!policy.cold_start);
        let approved = policy.grant(&mut store, grant()).unwrap();
        assert_eq!(approved.epoch, 1);
        drop(store);
        let mut store = DataStore::open(&path).unwrap();
        let mut policy = PolicyStore::load(&mut store, "catalog-one").unwrap();
        assert_eq!(policy.records[0].epoch, 1);
        policy
            .revoke(
                &mut store,
                PermissionKey {
                    plugin_id: "plugin-base64-encoder".into(),
                    command_id: "run".into(),
                    target: GrantTarget::Cli,
                },
            )
            .unwrap();
        assert!(policy.records[0].grant.is_none());
        assert_eq!(policy.records[0].epoch, 2);
        let policy = PolicyStore::load(&mut store, "catalog-two").unwrap();
        assert!(policy.records[0].grant.is_none());
        let policy = PolicyStore::load(&mut store, "catalog-one").unwrap();
        assert!(policy.records[0].grant.is_none());
    }

    #[test]
    fn rejects_undeclared_effects_and_invalid_budgets_without_policy_writes() {
        let mut store = DataStore::memory().unwrap();
        let mut policy = PolicyStore::load(&mut store, "catalog").unwrap();
        let mut bad = grant();
        bad.effects = vec!["data-write".into()];
        assert!(policy.grant(&mut store, bad).is_err());
        let mut bad = grant();
        bad.max_calls = 0;
        assert!(policy.grant(&mut store, bad).is_err());
        assert!(PolicyStore::load(&mut store, "catalog")
            .unwrap()
            .records
            .is_empty());
    }
    #[test]
    fn bad_policy_import_cannot_partially_grant_or_enable_cold_start() {
        let mut store = DataStore::memory().unwrap();
        let mut policy = PolicyStore::load(&mut store, "catalog").unwrap();
        let mut bad = grant();
        bad.package_digest = "spoof".into();
        assert!(policy
            .import(
                &mut store,
                PolicyImport {
                    format_version: 1,
                    cold_start: true,
                    grants: vec![grant(), bad]
                }
            )
            .is_err());
        let policy = PolicyStore::load(&mut store, "catalog").unwrap();
        assert!(policy.records.is_empty());
        assert!(!policy.cold_start);
    }
}
