//! Persistent Host policy in the same single-writer database as plugin data.
use crate::{
    broker::{CapabilityBroker, CommandIdentity, Grant},
    catalog::{digest, BuiltinCatalog},
    data::DataStore,
    protocol::*,
};
use rusqlite::{params, OptionalExtension};

pub struct PolicyStore {
    pub records: Vec<PermissionRecord>,
    pub cold_start: bool,
    catalog: BuiltinCatalog,
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
        events.push(serde_json::json!({"sequence":sequence,"action":action,"at":crate::runtime::now(),"decisions":records.iter().map(|r|serde_json::json!({"identityDigest":digest(&serde_json::to_value(&r.identity).expect("identity")),"epoch":r.epoch,"approved":r.grant.is_some()})).collect::<Vec<_>>() }));
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
        if changed.len() > 256 {
            return Err(ErrorCode::BudgetExceeded);
        }
        let tx = store
            .connection
            .transaction()
            .map_err(|_| ErrorCode::StorageFailed)?;
        for record in &changed {
            Self::save(&tx, record)?;
        }
        tx.execute("INSERT INTO core_metadata(key,value) VALUES('cold-start',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[if import.cold_start {"true"} else {"false"}]).map_err(|_|ErrorCode::StorageFailed)?;
        Self::audit(&tx, "import", &changed)?;
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
        let mut records: Vec<PermissionRecord> = store
            .connection
            .prepare("SELECT epoch,state FROM grants ORDER BY identity")
            .map_err(|_| ErrorCode::StorageFailed)?
            .query_map([], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| ErrorCode::StorageFailed)?
            .map(|row| {
                let (epoch, state) = row.map_err(|_| ErrorCode::StoreCorrupt)?;
                let record: PermissionRecord =
                    serde_json::from_str(&state).map_err(|_| ErrorCode::StoreCorrupt)?;
                if epoch == 0 || epoch != record.epoch {
                    return Err(ErrorCode::StoreCorrupt);
                }
                Ok(record)
            })
            .collect::<Result<_, _>>()?;
        if previous.as_deref() != Some(catalog_digest) {
            let tx = store
                .connection
                .transaction()
                .map_err(|_| ErrorCode::StorageFailed)?;
            for record in &mut records {
                record.epoch = record
                    .epoch
                    .checked_add(1)
                    .ok_or(ErrorCode::BudgetExceeded)?;
                record.grant = None;
                Self::save(&tx, record)?;
            }
            Self::audit(&tx, "catalog-change", &records)?;
            tx.execute("INSERT INTO core_metadata(key,value) VALUES('catalog-digest',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[catalog_digest]).map_err(|_|ErrorCode::StorageFailed)?;
            tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
        }
        let mut policy = Self {
            records: vec![],
            cold_start: false,
            catalog,
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
        if self.records.len() >= 256 && !self.records.iter().any(|r| r.identity == identity) {
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
        Self::save(&tx, &record)?;
        Self::audit(&tx, "grant", std::slice::from_ref(&record))?;
        tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
        self.records.retain(|r| r.identity != record.identity);
        self.records.push(record.clone());
        Ok(record)
    }
    pub fn revoke(&mut self, store: &mut DataStore, key: PermissionKey) -> Result<(), ErrorCode> {
        // Revoke all versions for this stable command identity, including rollback.
        let mut changed = self.records.clone();
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
        for record in &changed {
            Self::save(&tx, record)?;
        }
        Self::audit(&tx, "revoke", &changed)?;
        tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
        self.records = changed;
        Ok(())
    }
    pub fn set(&mut self, store: &mut DataStore, policy: BootstrapPolicy) -> Result<(), ErrorCode> {
        let tx = store
            .connection
            .transaction()
            .map_err(|_| ErrorCode::StorageFailed)?;
        tx.execute("INSERT INTO core_metadata(key,value) VALUES('cold-start',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[if policy.cold_start {"true"}else{"false"}]).map_err(|_|ErrorCode::StorageFailed)?;
        Self::audit(&tx, "bootstrap-policy", &[])?;
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

    fn grant() -> PermissionGrant {
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
