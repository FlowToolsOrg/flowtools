//! Host-owned, ephemeral resource references. No paths enter plugin operations.
//! Resource policy uses the existing grants table/writer; v1 is unchanged.
use crate::{broker::CommandIdentity, protocol::*};
use serde::{Deserialize, Serialize};
use specta::Type;

pub const MAX_CHUNK: u32 = 65_536;
pub const MAX_FILE: u64 = 67_108_864;
pub const MAX_OBSERVED: u64 = 268_435_456;
pub const MAX_RUN_REFS: usize = 32;
pub const MAX_PROFILE_REFS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceRight {
    Read,
    Stat,
    List,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    File,
    Directory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceScope {
    pub slot: String,
    pub operations: Vec<ResourceRight>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceGrant {
    pub format_version: u16,
    pub capability_profile: String,
    pub plugin_id: String,
    pub command_id: String,
    pub target: GrantTarget,
    pub package_digest: String,
    pub scopes: Vec<ResourceScope>,
    pub expires_at: f64,
    pub max_calls: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourcePermission {
    pub format_version: u16,
    pub identity: CommandIdentity,
    pub epoch: u32,
    pub grant: Option<ResourceGrant>,
}
impl ResourceGrant {
    pub(crate) fn valid(&self) -> bool {
        self.format_version == 2
            && self.capability_profile == crate::native_contract::NATIVE_CAPABILITY_PROFILE
            && crate::dependencies::valid_dependency_id(&self.plugin_id)
            && crate::dependencies::valid_dependency_id(&self.command_id)
            && self.package_digest.len() == 64
            && self
                .package_digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && self.expires_at.is_finite()
            && self.expires_at > 0.0
            && self.max_calls > 0
            && self.max_calls <= 10_000
            && !self.scopes.is_empty()
            && self.scopes.len() <= 32
            && self.scopes.iter().enumerate().all(|(i, scope)| {
                crate::dependencies::valid_dependency_id(&scope.slot)
                    && !self.scopes[..i]
                        .iter()
                        .any(|other| other.slot == scope.slot)
                    && !scope.operations.is_empty()
                    && scope.operations.len() <= 3
                    && scope
                        .operations
                        .iter()
                        .enumerate()
                        .all(|(n, op)| !scope.operations[..n].contains(op))
            })
    }
    pub(crate) fn allows(&self, slot: &str, right: ResourceRight) -> bool {
        self.scopes
            .iter()
            .any(|scope| scope.slot == slot && scope.operations.contains(&right))
    }
}

/// Closed NA1 subset. Unsupported v2 effects remain unavailable.
pub fn readonly_command(command: &serde_json::Value) -> bool {
    crate::native_contract::command_valid(command)
        && command["effects"] == serde_json::json!(["file-read"])
        && command["permissions"]
            .as_array()
            .is_some_and(|permissions| {
                permissions.len() == 1
                    && permissions[0]["capability"] == "fs"
                    && permissions[0]["operations"].as_array().is_some_and(|ops| {
                        ops.iter()
                            .all(|op| matches!(op.as_str(), Some("read" | "stat" | "list")))
                    })
            })
}
pub fn declared(command: &serde_json::Value, slot: &str, right: ResourceRight) -> bool {
    let op = match right {
        ResourceRight::Read => "read",
        ResourceRight::Stat => "stat",
        ResourceRight::List => "list",
    };
    readonly_command(command)
        && command["permissions"][0]["operations"]
            .as_array()
            .is_some_and(|ops| ops.iter().any(|v| v == op))
        && command["permissions"][0]["scopes"]
            .as_array()
            .is_some_and(|scopes| {
                scopes
                    .iter()
                    .any(|scope| scope["kind"] == "resource" && scope["slot"] == slot)
            })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceInfo {
    #[serde(rename = "ref")]
    pub reference: String,
    pub kind: ResourceKind,
    pub name: String,
    pub size: Option<u32>,
    pub revision: String,
    pub rights: Vec<ResourceRight>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceChunk {
    pub base64: String,
    pub next_offset: u32,
    pub eof: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourcePage {
    pub entries: Vec<ResourceInfo>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(
    tag = "operation",
    content = "parameters",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum ResourceOperation {
    Resolve {
        slot: String,
    },
    Stat {
        #[serde(rename = "ref")]
        reference: String,
    },
    Read {
        #[serde(rename = "ref")]
        reference: String,
        revision: String,
        offset: u32,
        #[serde(rename = "maxBytes")]
        max_bytes: u32,
    },
    List {
        #[serde(rename = "ref")]
        reference: String,
        revision: String,
        cursor: Option<String>,
        limit: u32,
    },
    Release {
        #[serde(rename = "ref")]
        reference: String,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum ResourceResponse {
    Info(ResourceInfo),
    Chunk(ResourceChunk),
    Page(ResourcePage),
    Released,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceOwnerRequest {
    pub key: PermissionKey,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceOwner {
    pub id: String,
    pub identity: CommandIdentity,
    pub expires_at: f64,
    pub scopes: Vec<ResourceScope>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceSelection {
    pub owner: String,
    pub slot: String,
    pub kind: ResourceKind,
    pub path: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceSubmission {
    pub owner: String,
    pub submit: SubmitJob,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceOwnerKey {
    pub owner: String,
}

#[cfg(windows)]
pub mod native;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn resource_contract_rejects_paths_identities_and_unknown_operations_in_plugin_frames() {
        let allowed = json!({"operation":"read","parameters":{"ref":"opaque","revision":"pin","offset":0,"maxBytes":65536}});
        assert!(serde_json::from_value::<ResourceOperation>(allowed.clone()).is_ok());
        for (key, value) in [
            ("path", json!("C:/private")),
            ("caller", json!("local-manager")),
            ("owner", json!("other")),
            ("grant", json!(true)),
        ] {
            let mut injected = allowed.clone();
            injected["parameters"][key] = value;
            assert!(serde_json::from_value::<ResourceOperation>(injected).is_err());
        }
        assert!(serde_json::from_value::<ResourceOperation>(
            json!({"operation":"write","parameters":{}})
        )
        .is_err());
        assert!(serde_json::from_value::<ResourceOperation>(json!({"operation":"read","parameters":{"ref":"opaque","revision":"pin","offset":-1,"maxBytes":1}})).is_err());
        assert!(serde_json::from_value::<crate::broker::Scope>(
            json!({"kind":"resource","slot":"source"})
        )
        .is_err());
    }
    #[test]
    fn resource_grants_have_a_separate_closed_version_and_only_read_rights() {
        let raw = json!({"formatVersion":2,"capabilityProfile":crate::native_contract::NATIVE_CAPABILITY_PROFILE,"pluginId":"fixture-plugin","commandId":"run","target":"cli","packageDigest":"a".repeat(64),"scopes":[{"slot":"source","operations":["read","stat"]}],"expiresAt":1000.0,"maxCalls":64});
        let grant: ResourceGrant = serde_json::from_value(raw.clone()).unwrap();
        assert!(grant.valid());
        assert!(grant.allows("source", ResourceRight::Read));
        assert!(!grant.allows("source", ResourceRight::List));
        let mut duplicate = grant.clone();
        duplicate.scopes.push(duplicate.scopes[0].clone());
        assert!(!duplicate.valid());
        let mut old = grant;
        old.format_version = 1;
        assert!(!old.valid());
        for key in ["coldStart", "background", "path"] {
            let mut bad = raw.clone();
            bad[key] = json!(true);
            assert!(serde_json::from_value::<ResourceGrant>(bad).is_err());
        }
        let mut bad = raw;
        bad["scopes"][0]["operations"] = json!(["write"]);
        assert!(serde_json::from_value::<ResourceGrant>(bad).is_err());
    }
}
