//! T0-owned policy for fixed T1 commands. This is not a process sandbox.
//! Approval is an in-memory Host API until P2.6a/P2.4a supply durable storage
//! and management sessions. Neither wire requests nor plugin metadata grant it.
use crate::{catalog::digest, protocol::ErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandIdentity {
    pub caller: String,
    pub publisher: String,
    pub plugin_id: String,
    pub version: String,
    pub package_digest: String,
    pub command_id: String,
}

impl CommandIdentity {
    pub fn from_manifest(caller: &str, manifest: &Value, command: &Value) -> Self {
        Self {
            caller: caller.into(),
            publisher: manifest["publisher"]
                .as_str()
                .expect("Validated publisher")
                .into(),
            plugin_id: manifest["id"].as_str().expect("Validated ID").into(),
            version: manifest["version"]
                .as_str()
                .expect("Validated version")
                .into(),
            package_digest: digest(manifest),
            command_id: command["id"].as_str().expect("Validated command").into(),
        }
    }
}

/// Narrow operations: no caller/plugin/namespace, raw paths, SQL, argv or env.
/// File handles and tool locks must be issued by a future Host adapter.
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(
    tag = "operation",
    content = "parameters",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum CapabilityOperation {
    FileRead { handle: String },
    FileCreate { handle: String },
    FileReplace { handle: String },
    FileDelete { handle: String },
    NetworkRead { origin: String, method: ReadMethod },
    NetworkSend { origin: String, method: SendMethod },
    DataRead { key: String },
    DataWrite { key: String },
    ClipboardRead,
    ClipboardWrite,
    ToolExecute { lock: String, action: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
pub enum ReadMethod {
    GET,
    HEAD,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
pub enum SendMethod {
    POST,
    PUT,
    PATCH,
    DELETE,
}

/// Exact, Host-approved scopes; there are no wildcard grants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(
    tag = "kind",
    content = "scope",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Scope {
    FileHandle(String),
    NetworkRead { origin: String, method: ReadMethod },
    NetworkSend { origin: String, method: SendMethod },
    PluginData { key_prefix: String },
    Clipboard,
    ToolLock { lock: String, action: String },
}

#[derive(Clone, Debug)]
pub struct Grant {
    pub effects: Vec<String>,
    pub scopes: Vec<Scope>,
    pub expires_at: f64,
    pub max_calls: u32,
}

/// Opaque and non-serializable. Only Host code can bind a runner to an identity.
#[derive(Clone)]
pub struct BrokerSession {
    id: String,
}

struct Policy {
    epoch: u32,
    grant: Option<Grant>,
}
struct Binding {
    identity: CommandIdentity,
    command: Value,
    epoch: u32,
    deadline: f64,
    calls: u32,
}

#[derive(Default)]
pub struct CapabilityBroker {
    policies: HashMap<CommandIdentity, Policy>,
    bindings: HashMap<String, Binding>,
}

impl CapabilityBroker {
    /// T0 management API, deliberately absent from the wire protocol.
    /// Every replacement invalidates all previously issued sessions.
    pub fn approve(&mut self, identity: CommandIdentity, grant: Grant) -> Result<u32, ErrorCode> {
        if !grant.expires_at.is_finite()
            || grant.max_calls == 0
            || grant.max_calls > 10_000
            || grant.effects.len() > 32
            || (grant.effects.is_empty() != grant.scopes.is_empty())
            || grant.scopes.len() > 64
            || grant.effects.iter().any(|effect| {
                !matches!(
                    effect.as_str(),
                    "file-read"
                        | "file-create"
                        | "file-replace"
                        | "file-delete"
                        | "network-read"
                        | "network-send"
                        | "data-read"
                        | "data-write"
                        | "clipboard-read"
                        | "clipboard-write"
                        | "tool-execute"
                )
            })
            || grant
                .effects
                .iter()
                .enumerate()
                .any(|(i, effect)| grant.effects[..i].contains(effect))
            || grant.scopes.iter().any(|scope| !scope.valid())
            || grant
                .scopes
                .iter()
                .enumerate()
                .any(|(i, scope)| grant.scopes[..i].contains(scope))
        {
            return Err(ErrorCode::InvalidRequest);
        }
        let policy = self.policies.entry(identity).or_insert(Policy {
            epoch: 0,
            grant: None,
        });
        policy.epoch = policy.epoch.checked_add(1).ok_or(ErrorCode::RuntimeBusy)?;
        policy.grant = Some(grant);
        Ok(policy.epoch)
    }

    pub fn restore_policy(
        &mut self,
        identity: CommandIdentity,
        epoch: u32,
        grant: Option<Grant>,
    ) -> Result<(), ErrorCode> {
        if epoch == 0 {
            return Err(ErrorCode::StoreCorrupt);
        }
        if let Some(ref approved) = grant {
            let mut validator = Self::default();
            validator.approve(identity.clone(), approved.clone())?;
        }
        self.policies.insert(identity, Policy { epoch, grant });
        Ok(())
    }

    pub fn require_command_grant(
        &self,
        identity: &CommandIdentity,
        at: f64,
    ) -> Result<u32, ErrorCode> {
        let policy = self
            .policies
            .get(identity)
            .ok_or(ErrorCode::ApprovalRequired)?;
        if policy
            .grant
            .as_ref()
            .is_none_or(|grant| grant.expires_at <= at)
        {
            return Err(ErrorCode::ApprovalRequired);
        }
        Ok(policy.epoch)
    }

    pub fn revoke(&mut self, identity: &CommandIdentity) -> Result<(), ErrorCode> {
        let policy = self.policies.entry(identity.clone()).or_insert(Policy {
            epoch: 0,
            grant: None,
        });
        policy.epoch = policy.epoch.checked_add(1).ok_or(ErrorCode::RuntimeBusy)?;
        policy.grant = None;
        Ok(())
    }

    pub fn authorize_command(
        &self,
        identity: &CommandIdentity,
        command: &Value,
        at: f64,
    ) -> Result<u32, ErrorCode> {
        if command["headless"] != true || command["interaction"] == "required" {
            return Err(ErrorCode::InteractionRequired);
        }
        let effects = command["effects"]
            .as_array()
            .ok_or(ErrorCode::CapabilityUndeclared)?;
        let permissions = command["permissions"]
            .as_array()
            .ok_or(ErrorCode::CapabilityUndeclared)?;
        // Preserve G2's explicit validation-only pure T1 evaluation.
        if effects.is_empty() && permissions.is_empty() {
            return Ok(self
                .policies
                .get(identity)
                .filter(|p| p.grant.is_some())
                .map_or(0, |p| p.epoch));
        }
        let policy = self
            .policies
            .get(identity)
            .ok_or(ErrorCode::ApprovalRequired)?;
        let grant = policy.grant.as_ref().ok_or(ErrorCode::ApprovalRequired)?;
        if !at.is_finite()
            || grant.expires_at <= at
            || effects
                .iter()
                .any(|effect| !grant.effects.iter().any(|v| effect == v))
        {
            return Err(ErrorCode::ApprovalRequired);
        }
        Ok(policy.epoch)
    }

    pub fn bind(
        &mut self,
        identity: CommandIdentity,
        command: Value,
        deadline: f64,
        at: f64,
    ) -> Result<BrokerSession, ErrorCode> {
        if !at.is_finite() || !deadline.is_finite() || deadline <= at {
            return Err(ErrorCode::Timeout);
        }
        if deadline - at > command["resources"]["timeoutMs"].as_f64().unwrap_or(0.0) {
            return Err(ErrorCode::BudgetExceeded);
        }
        let epoch = self.authorize_command(&identity, &command, at)?;
        let id = Uuid::new_v4().to_string();
        self.bindings.insert(
            id.clone(),
            Binding {
                identity,
                command,
                epoch,
                deadline,
                calls: 0,
            },
        );
        Ok(BrokerSession { id })
    }

    pub fn check_session(&self, session: &BrokerSession, at: f64) -> Result<(), ErrorCode> {
        let binding = self
            .bindings
            .get(&session.id)
            .ok_or(ErrorCode::SessionInvalid)?;
        if !at.is_finite() || at >= binding.deadline {
            return Err(ErrorCode::Timeout);
        }
        if binding.epoch != 0 {
            let policy = self
                .policies
                .get(&binding.identity)
                .ok_or(ErrorCode::GrantRevoked)?;
            if policy.epoch != binding.epoch
                || policy.grant.as_ref().is_none_or(|g| g.expires_at <= at)
            {
                return Err(ErrorCode::GrantRevoked);
            }
        }
        Ok(())
    }

    /// Authorization and adapter entry share the Host's mutable broker lock.
    /// Do not return a reusable permission ticket or defer a side effect outside
    /// this callback. Async adapters need a fresh check at their commit point.
    pub fn invoke<T>(
        &mut self,
        session: &BrokerSession,
        operation: &CapabilityOperation,
        at: f64,
        adapter: impl FnOnce(&CommandIdentity) -> Result<T, ErrorCode>,
    ) -> Result<T, ErrorCode> {
        self.check_session(session, at)?;
        let binding = &self.bindings[&session.id];
        let (effect, capability, method) = operation.declaration();
        if !binding.command["effects"]
            .as_array()
            .is_some_and(|effects| effects.iter().any(|v| v == effect))
            || !binding.command["permissions"]
                .as_array()
                .is_some_and(|permissions| {
                    permissions.iter().any(|p| {
                        p["capability"] == capability
                            && p["operations"]
                                .as_array()
                                .is_some_and(|ops| ops.iter().any(|v| v == method))
                    })
                })
        {
            return Err(ErrorCode::CapabilityUndeclared);
        }
        if !binding.command["permissions"]
            .as_array()
            .is_some_and(|permissions| {
                permissions.iter().any(|p| {
                    p["capability"] == capability
                        && p["scopes"].as_array().is_some_and(|scopes| {
                            scopes.iter().any(|scope| {
                                scope
                                    .as_str()
                                    .is_some_and(|scope| operation.in_manifest_scope(scope))
                            })
                        })
                })
            })
        {
            return Err(ErrorCode::ScopeDenied);
        }
        let grant = self
            .policies
            .get(&binding.identity)
            .and_then(|p| p.grant.as_ref())
            .ok_or(ErrorCode::ApprovalRequired)?;
        if !grant.effects.iter().any(|v| v == effect) {
            return Err(ErrorCode::ApprovalRequired);
        }
        if !grant.scopes.iter().any(|scope| scope.contains(operation)) {
            return Err(ErrorCode::ScopeDenied);
        }
        if binding.calls >= grant.max_calls {
            return Err(ErrorCode::BudgetExceeded);
        }
        let binding = self.bindings.get_mut(&session.id).unwrap();
        binding.calls += 1;
        adapter(&binding.identity)
    }

    pub fn close(&mut self, session: &BrokerSession) {
        self.bindings.remove(&session.id);
    }
}

impl CapabilityOperation {
    fn in_manifest_scope(&self, scope: &str) -> bool {
        match self {
            Self::FileRead { handle }
            | Self::FileCreate { handle }
            | Self::FileReplace { handle }
            | Self::FileDelete { handle } => scope == format!("handle:{handle}"),
            Self::NetworkRead { origin, .. } | Self::NetworkSend { origin, .. } => scope == origin,
            Self::DataRead { .. } | Self::DataWrite { .. } => scope == "plugin-data",
            Self::ClipboardRead | Self::ClipboardWrite => scope == "text",
            Self::ToolExecute { lock, action } => scope == format!("lock:{lock}:{action}"),
        }
    }
    fn declaration(&self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::FileRead { .. } => ("file-read", "fs", "read"),
            Self::FileCreate { .. } => ("file-create", "fs", "create"),
            Self::FileReplace { .. } => ("file-replace", "fs", "replace"),
            Self::FileDelete { .. } => ("file-delete", "fs", "delete"),
            Self::NetworkRead { .. } => ("network-read", "network", "request"),
            Self::NetworkSend { .. } => ("network-send", "network", "request"),
            Self::DataRead { .. } => ("data-read", "storage", "get"),
            Self::DataWrite { .. } => ("data-write", "storage", "set"),
            Self::ClipboardRead => ("clipboard-read", "clipboard", "read"),
            Self::ClipboardWrite => ("clipboard-write", "clipboard", "write"),
            Self::ToolExecute { .. } => ("tool-execute", "tool", "execute"),
        }
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && key
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || v == b'-' || v == b'_')
}

impl Scope {
    fn valid(&self) -> bool {
        match self {
            Self::FileHandle(handle) => valid_key(handle),
            Self::NetworkRead { origin, .. } | Self::NetworkSend { origin, .. } => {
                valid_origin(origin)
            }
            Self::PluginData { key_prefix } => valid_key(key_prefix),
            Self::Clipboard => true,
            Self::ToolLock { lock, action } => valid_key(lock) && valid_key(action),
        }
    }
    fn contains(&self, operation: &CapabilityOperation) -> bool {
        match (self, operation) {
            (
                Self::FileHandle(approved),
                CapabilityOperation::FileRead { handle }
                | CapabilityOperation::FileCreate { handle }
                | CapabilityOperation::FileReplace { handle }
                | CapabilityOperation::FileDelete { handle },
            ) => approved == handle && valid_key(handle),
            (
                Self::NetworkRead {
                    origin: approved,
                    method: allowed,
                },
                CapabilityOperation::NetworkRead { origin, method },
            ) => approved == origin && allowed == method,
            (
                Self::NetworkSend {
                    origin: approved,
                    method: allowed,
                },
                CapabilityOperation::NetworkSend { origin, method },
            ) => approved == origin && allowed == method,
            (
                Self::PluginData { key_prefix },
                CapabilityOperation::DataRead { key } | CapabilityOperation::DataWrite { key },
            ) => {
                valid_key(key)
                    && (key == key_prefix
                        || key
                            .strip_prefix(key_prefix)
                            .is_some_and(|suffix| suffix.starts_with('-')))
            }
            (
                Self::Clipboard,
                CapabilityOperation::ClipboardRead | CapabilityOperation::ClipboardWrite,
            ) => true,
            (
                Self::ToolLock {
                    lock: approved,
                    action: allowed,
                },
                CapabilityOperation::ToolExecute { lock, action },
            ) => approved == lock && allowed == action,
            _ => false,
        }
    }
}

// This is an exact-origin policy contract, not DNS/redirect enforcement. The
// network adapter remains unavailable until it enforces those constraints.
fn valid_origin(origin: &str) -> bool {
    let Some(authority) = origin.strip_prefix("https://") else {
        return false;
    };
    let Some((host, port)) = authority.rsplit_once(':') else {
        return false;
    };
    !host.is_empty()
        && host.len() <= 253
        && host.contains('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|v| v.is_ascii_lowercase() || v.is_ascii_digit() || v == b'-')
        })
        && host.parse::<std::net::IpAddr>().is_err()
        && port
            .parse::<u16>()
            .is_ok_and(|value| value > 0 && value.to_string() == port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::BuiltinCatalog;
    use serde_json::json;

    fn fixture() -> (CapabilityBroker, CommandIdentity, Value, Grant) {
        let catalog = BuiltinCatalog::embedded();
        let (manifest, command) = catalog.command("plugin-todo-list", "run").unwrap();
        let identity = CommandIdentity::from_manifest("host-cli", manifest, command);
        (
            CapabilityBroker::default(),
            identity,
            command.clone(),
            Grant {
                effects: vec!["data-read".into(), "data-write".into()],
                scopes: vec![Scope::PluginData {
                    key_prefix: "todos".into(),
                }],
                expires_at: 200.0,
                max_calls: 2,
            },
        )
    }

    #[test]
    fn actual_todo_contract_requires_host_approval_and_binds_the_namespace() {
        let (mut broker, identity, command, grant) = fixture();
        assert_eq!(
            broker
                .bind(identity.clone(), command.clone(), 100.0, 10.0)
                .err(),
            Some(ErrorCode::ApprovalRequired)
        );
        broker.approve(identity.clone(), grant).unwrap();
        let session = broker.bind(identity.clone(), command, 100.0, 10.0).unwrap();
        let mut namespaces = Vec::new();
        for key in ["todos", "todos-active"] {
            broker
                .invoke(
                    &session,
                    &CapabilityOperation::DataWrite { key: key.into() },
                    11.0,
                    |bound| {
                        namespaces.push(bound.plugin_id.clone());
                        Ok(())
                    },
                )
                .unwrap();
        }
        assert_eq!(namespaces, ["plugin-todo-list", "plugin-todo-list"]);
        let mut calls = 0;
        for (operation, expected) in [
            (
                CapabilityOperation::DataRead {
                    key: "todos".into(),
                },
                ErrorCode::BudgetExceeded,
            ),
            (
                CapabilityOperation::DataWrite {
                    key: "todos-other/../../grants".into(),
                },
                ErrorCode::ScopeDenied,
            ),
            (
                CapabilityOperation::DataRead {
                    key: "todosother".into(),
                },
                ErrorCode::ScopeDenied,
            ),
            (
                CapabilityOperation::ClipboardRead,
                ErrorCode::CapabilityUndeclared,
            ),
            (
                CapabilityOperation::FileDelete {
                    handle: "todos".into(),
                },
                ErrorCode::CapabilityUndeclared,
            ),
        ] {
            assert_eq!(
                broker.invoke(&session, &operation, 12.0, |_| {
                    calls += 1;
                    Ok(())
                }),
                Err(expected)
            );
        }
        assert_eq!(calls, 0);
        broker.close(&session);
        assert_eq!(
            broker.invoke(
                &session,
                &CapabilityOperation::DataRead {
                    key: "todos".into()
                },
                12.0,
                |_| {
                    calls += 1;
                    Ok(())
                }
            ),
            Err(ErrorCode::SessionInvalid)
        );
        assert_eq!(calls, 0);
    }

    #[test]
    fn caller_publisher_version_digest_and_command_do_not_inherit_grants() {
        let (mut broker, identity, command, grant) = fixture();
        broker.approve(identity.clone(), grant).unwrap();
        for field in 0..6 {
            let mut changed = identity.clone();
            match field {
                0 => changed.caller = "other-client".into(),
                1 => changed.publisher = "other-publisher".into(),
                2 => changed.version = "0.2.0".into(),
                3 => changed.package_digest = "changed-artifact".into(),
                4 => changed.command_id = "other-command".into(),
                _ => changed.plugin_id = "other-plugin".into(),
            }
            assert_eq!(
                broker.bind(changed, command.clone(), 100.0, 10.0).err(),
                Some(ErrorCode::ApprovalRequired)
            );
        }
        // A new Runtime never restores transient approvals from saved metadata.
        let mut restarted = CapabilityBroker::default();
        assert_eq!(
            restarted.bind(identity, command, 100.0, 10.0).err(),
            Some(ErrorCode::ApprovalRequired)
        );
    }

    #[test]
    fn revocation_and_reapproval_never_revalidate_an_old_runner_session() {
        let (mut broker, identity, command, grant) = fixture();
        broker.approve(identity.clone(), grant.clone()).unwrap();
        let old = broker
            .bind(identity.clone(), command.clone(), 100.0, 10.0)
            .unwrap();
        broker.revoke(&identity).unwrap();
        assert_eq!(
            broker.authorize_command(&identity, &command, 11.0),
            Err(ErrorCode::ApprovalRequired)
        );
        broker.approve(identity.clone(), grant).unwrap();
        let new = broker.bind(identity, command, 100.0, 12.0).unwrap();
        let mut calls = 0;
        assert_eq!(
            broker.invoke(
                &old,
                &CapabilityOperation::DataWrite {
                    key: "todos".into()
                },
                12.0,
                |_| {
                    calls += 1;
                    Ok(())
                }
            ),
            Err(ErrorCode::GrantRevoked)
        );
        broker
            .invoke(
                &new,
                &CapabilityOperation::DataWrite {
                    key: "todos".into(),
                },
                12.0,
                |_| {
                    calls += 1;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(broker.check_session(&new, 100.0), Err(ErrorCode::Timeout));
        assert_eq!(
            broker.check_session(&new, f64::NAN),
            Err(ErrorCode::Timeout)
        );
    }

    #[test]
    fn expired_grants_and_interactive_commands_fail_before_adapter_entry() {
        let (mut broker, identity, mut command, mut grant) = fixture();
        grant.expires_at = 20.0;
        broker.approve(identity.clone(), grant).unwrap();
        let session = broker
            .bind(identity.clone(), command.clone(), 100.0, 10.0)
            .unwrap();
        let mut calls = 0;
        assert_eq!(
            broker.invoke(
                &session,
                &CapabilityOperation::DataWrite {
                    key: "todos".into()
                },
                20.0,
                |_| {
                    calls += 1;
                    Ok(())
                }
            ),
            Err(ErrorCode::GrantRevoked)
        );
        command["interaction"] = json!("required");
        assert_eq!(
            broker.authorize_command(&identity, &command, 10.0),
            Err(ErrorCode::InteractionRequired)
        );
        assert_eq!(calls, 0);
    }

    #[test]
    fn destructive_effects_methods_and_scopes_require_independent_approval() {
        let cases = [
            (
                "file-read",
                "fs",
                "read",
                "handle:file-one",
                Scope::FileHandle("file-one".into()),
                CapabilityOperation::FileRead {
                    handle: "file-one".into(),
                },
                CapabilityOperation::FileDelete {
                    handle: "file-one".into(),
                },
            ),
            (
                "network-read",
                "network",
                "request",
                "https://example.invalid:443",
                Scope::NetworkRead {
                    origin: "https://example.invalid:443".into(),
                    method: ReadMethod::HEAD,
                },
                CapabilityOperation::NetworkRead {
                    origin: "https://example.invalid:443".into(),
                    method: ReadMethod::HEAD,
                },
                CapabilityOperation::NetworkSend {
                    origin: "https://example.invalid:443".into(),
                    method: SendMethod::POST,
                },
            ),
            (
                "clipboard-read",
                "clipboard",
                "read",
                "text",
                Scope::Clipboard,
                CapabilityOperation::ClipboardRead,
                CapabilityOperation::ClipboardWrite,
            ),
            (
                "tool-execute",
                "tool",
                "execute",
                "lock:host-lock:probe",
                Scope::ToolLock {
                    lock: "host-lock".into(),
                    action: "probe".into(),
                },
                CapabilityOperation::ToolExecute {
                    lock: "host-lock".into(),
                    action: "probe".into(),
                },
                CapabilityOperation::ToolExecute {
                    lock: "foreign-lock".into(),
                    action: "probe".into(),
                },
            ),
        ];
        for (effect, capability, method, scope, approved, allowed, denied) in cases {
            let (mut broker, identity, mut command, mut grant) = fixture();
            command["effects"] = json!([effect]);
            command["permissions"] =
                json!([{"capability": capability, "operations": [method], "scopes": [scope]}]);
            grant.effects = vec![effect.into()];
            grant.scopes = vec![approved];
            broker.approve(identity.clone(), grant).unwrap();
            let session = broker.bind(identity, command, 100.0, 10.0).unwrap();
            let mut calls = 0;
            broker
                .invoke(&session, &allowed, 11.0, |_| {
                    calls += 1;
                    Ok(())
                })
                .unwrap();
            assert!(broker
                .invoke(&session, &denied, 11.0, |_| {
                    calls += 1;
                    Ok(())
                })
                .is_err());
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn manifest_scope_and_grant_scope_are_both_required() {
        let (mut broker, identity, mut command, grant) = fixture();
        command["permissions"][0]["scopes"] = json!(["host-metadata"]);
        broker.approve(identity.clone(), grant).unwrap();
        let session = broker.bind(identity, command, 100.0, 10.0).unwrap();
        let mut calls = 0;
        assert_eq!(
            broker.invoke(
                &session,
                &CapabilityOperation::DataWrite {
                    key: "todos".into()
                },
                11.0,
                |_| {
                    calls += 1;
                    Ok(())
                }
            ),
            Err(ErrorCode::ScopeDenied)
        );
        assert_eq!(calls, 0);
    }

    #[test]
    fn wire_operations_refuse_identity_raw_paths_sql_and_executable_injection() {
        for value in [
            json!({"operation":"data-read","parameters":{"key":"todos","namespace":"host"}}),
            json!({"operation":"data-write","parameters":{"key":"todos","pluginId":"other"}}),
            json!({"operation":"file-read","parameters":{"handle":"one","path":"C:/private"}}),
            json!({"operation":"tool-execute","parameters":{"lock":"one","action":"probe","argv":["--evil"]}}),
            json!({"operation":"native-invoke","parameters":{"command":"delete"}}),
            json!({"operation":"db-query","parameters":{"sql":"DROP TABLE grants"}}),
        ] {
            assert!(serde_json::from_value::<CapabilityOperation>(value).is_err());
        }
        for origin in [
            "http://example.invalid:80",
            "https://127.0.0.1:443",
            "https://example.invalid:0443",
            "https://example.invalid:0",
            "https://example.invalid:443?token=secret",
            "https://user@example.invalid:443",
            "https://example.invalid.:443",
            "https://*.invalid:443",
        ] {
            assert!(!valid_origin(origin));
        }
    }

    #[test]
    fn invalid_approvals_and_deadline_budgets_leave_the_current_policy_unchanged() {
        let (mut broker, identity, command, grant) = fixture();
        broker.approve(identity.clone(), grant.clone()).unwrap();
        let session = broker
            .bind(identity.clone(), command.clone(), 100.0, 10.0)
            .unwrap();
        for invalid in [
            Grant {
                max_calls: 0,
                ..grant.clone()
            },
            Grant {
                max_calls: 10_001,
                ..grant.clone()
            },
            Grant {
                expires_at: f64::INFINITY,
                ..grant.clone()
            },
            Grant {
                effects: vec!["raw-invoke".into()],
                ..grant.clone()
            },
            Grant {
                scopes: vec![Scope::FileHandle("C:/private".into())],
                ..grant.clone()
            },
        ] {
            assert_eq!(
                broker.approve(identity.clone(), invalid),
                Err(ErrorCode::InvalidRequest)
            );
            assert!(broker.check_session(&session, 11.0).is_ok());
        }
        assert_eq!(
            broker
                .bind(identity.clone(), command.clone(), 30_011.0, 10.0)
                .err(),
            Some(ErrorCode::BudgetExceeded)
        );
        assert_eq!(
            broker
                .bind(identity.clone(), command.clone(), 10.0, 10.0)
                .err(),
            Some(ErrorCode::Timeout)
        );
        assert_eq!(
            broker.bind(identity, command, f64::INFINITY, 10.0).err(),
            Some(ErrorCode::Timeout)
        );
    }
}
