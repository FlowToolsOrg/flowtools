//! NA0 inert v2 declarations. No ref resolution, grants or IO adapters.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

pub const NATIVE_CAPABILITY_PROFILE: &str = "flowtools-native-v2";
pub const RUNNER_PROTOCOL_VERSION: u16 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum NativeHttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NativeScopeV2 {
    Resource {
        slot: String,
    },
    Endpoint {
        slot: String,
        origin: String,
        methods: Vec<NativeHttpMethod>,
        path_prefix: Vec<String>,
    },
    Url {
        origin: String,
        path_prefix: Vec<String>,
    },
    Credential {
        slot: String,
        endpoint: String,
        method: NativeHttpMethod,
        header: NativeCredentialHeader,
    },
    Data {
        key_prefix: String,
    },
    Clipboard,
    Notification,
    Tool {
        lock: String,
        action: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NativeCredentialHeader {
    Authorization,
    XApiKey,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NativeCapability {
    Fs,
    Network,
    Opener,
    Secrets,
    Storage,
    Clipboard,
    Notification,
    Tool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativePermissionV2 {
    pub capability: NativeCapability,
    pub operations: Vec<String>,
    pub scopes: Vec<NativeScopeV2>,
}
fn key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
fn slot(value: &str) -> bool {
    crate::dependencies::valid_dependency_id(value)
}
pub(crate) fn origin(value: &str) -> bool {
    crate::broker::valid_origin(value)
        && value
            .strip_prefix("https://")
            .and_then(|v| v.rsplit_once(':'))
            .is_some_and(|(host, _)| !host.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
}
fn path(value: &[String]) -> bool {
    value.len() <= 16 && value.iter().all(|segment| key(segment))
}
fn unique<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(i, value)| !values[..i].contains(value))
}
impl NativeScopeV2 {
    fn valid(&self) -> bool {
        match self {
            Self::Resource { slot: value } => slot(value),
            Self::Endpoint {
                slot: value,
                origin: value_origin,
                methods,
                path_prefix,
            } => {
                slot(value)
                    && origin(value_origin)
                    && !methods.is_empty()
                    && methods.len() <= 6
                    && unique(methods)
                    && path(path_prefix)
            }
            Self::Url {
                origin: value,
                path_prefix,
            } => origin(value) && path(path_prefix),
            Self::Credential {
                slot: value,
                endpoint,
                ..
            } => slot(value) && slot(endpoint),
            Self::Data { key_prefix } => key(key_prefix),
            Self::Clipboard | Self::Notification => true,
            Self::Tool { lock, action } => key(lock) && key(action),
        }
    }
}
impl NativePermissionV2 {
    pub(crate) fn effect(&self, operation: &str) -> Option<&'static str> {
        use NativeCapability::*;
        Some(match (&self.capability, operation) {
            (Fs, "read" | "stat" | "list") => "file-read",
            (Fs, "create") => "file-create",
            (Network, "request") => "network-read",
            (Network, "send") => "network-send",
            (Opener, "open-url") => "opener-open-url",
            (Opener, "open-resource") => "opener-open-resource",
            (Opener, "reveal-resource") => "opener-reveal-resource",
            (Secrets, "use" | "describe") => "secret-use",
            (Storage, "get") => "data-read",
            (Storage, "set") => "data-write",
            (Clipboard, "read") => "clipboard-read",
            (Clipboard, "write") => "clipboard-write",
            (Notification, "notify") => "notification",
            (Tool, "execute") => "tool-execute",
            _ => return None,
        })
    }
    fn accepts(&self, operation: &str, scope: &NativeScopeV2) -> bool {
        use NativeCapability as C;
        use NativeScopeV2 as S;
        matches!(
            (&self.capability, operation, scope),
            (C::Fs, _, S::Resource { .. })
                | (C::Opener, "open-url", S::Url { .. })
                | (
                    C::Opener,
                    "open-resource" | "reveal-resource",
                    S::Resource { .. }
                )
                | (C::Secrets, _, S::Credential { .. })
                | (C::Storage, _, S::Data { .. })
                | (C::Clipboard, _, S::Clipboard)
                | (C::Notification, _, S::Notification)
                | (C::Tool, _, S::Tool { .. })
        ) || matches!((&self.capability,scope),(C::Network,S::Endpoint{methods,..}) if methods.iter().any(|method|matches!(method,NativeHttpMethod::Get|NativeHttpMethod::Head)==(operation=="request")))
    }
    pub(crate) fn valid(&self) -> bool {
        !self.operations.is_empty() && self.operations.len()<=32 && unique(&self.operations) &&
        !self.scopes.is_empty() && self.scopes.len()<=64 && unique(&self.scopes) && self.scopes.iter().all(NativeScopeV2::valid) &&
        self.scopes.iter().all(|scope|!matches!(scope,NativeScopeV2::Endpoint{methods,..} if methods.iter().any(|method|!self.operations.iter().any(|op|op==if matches!(method,NativeHttpMethod::Get|NativeHttpMethod::Head){"request"}else{"send"})))) &&
        self.operations.iter().all(|op| self.effect(op).is_some() && self.scopes.iter().any(|scope|self.accepts(op,scope))) &&
        self.scopes.iter().all(|scope|self.operations.iter().any(|op|self.accepts(op,scope)))
    }
}
pub(crate) fn command_valid(command: &Value) -> bool {
    if command["capabilityProfile"] != NATIVE_CAPABILITY_PROFILE {
        return false;
    }
    let Ok(permissions) =
        serde_json::from_value::<Vec<NativePermissionV2>>(command["permissions"].clone())
    else {
        return false;
    };
    let Ok(effects) = serde_json::from_value::<Vec<String>>(command["effects"].clone()) else {
        return false;
    };
    permissions.len() <= 32
        && effects.len() <= 32
        && unique(&effects)
        && permissions.iter().all(NativePermissionV2::valid)
        && permissions.iter().enumerate().all(|(i, p)| {
            !permissions[..i]
                .iter()
                .any(|prior| prior.capability == p.capability)
        })
        && permissions.iter().all(|p| {
            p.operations
                .iter()
                .all(|op| p.effect(op).is_some_and(|e| effects.iter().any(|v| v == e)))
        })
        && effects.iter().all(|e| {
            permissions.iter().any(|p| {
                p.operations
                    .iter()
                    .any(|op| p.effect(op) == Some(e.as_str()))
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn sdk_v2_corpus_matches_native_and_keeps_original_v1_bytes() {
        use sha2::{Digest, Sha256};
        let original = include_bytes!("../manifest-conformance.json");
        assert_eq!(
            format!("{:x}", Sha256::digest(original)),
            "fe6b2c4934c1bec9d4184a1721de52bdd98efd7e9f0605e0384a7cb827ff16fb"
        );
        let corpus: Value =
            serde_json::from_str(include_str!("../manifest-v2-conformance.json")).unwrap();
        let cases = corpus["cases"].as_array().unwrap();
        assert!(cases.len() >= 107);
        for case in cases {
            let result =
                crate::packages::manifest::validate_versioned_value(&case["manifest"], None);
            assert_eq!(
                result.is_ok(),
                case["accepted"] == true,
                "{}: {result:?}",
                case["id"]
            );
            if let Ok(value) = result {
                assert_eq!(value, case["normalized"], "{}", case["id"]);
            }
        }
    }
    #[test]
    fn native_scope_is_typed_but_never_a_legacy_scope_or_grant() {
        let value = json!({"capability":"fs","operations":["read"],"scopes":[{"kind":"resource","slot":"source"}]});
        let permission: NativePermissionV2 = serde_json::from_value(value.clone()).unwrap();
        assert!(permission.valid());
        assert!(
            serde_json::from_value::<crate::broker::Scope>(value["scopes"][0].clone()).is_err()
        );
        for scope in [
            json!("handle:source"),
            json!({"kind":"resource","slot":"source","path":"C:/private"}),
        ] {
            let mut value = value.clone();
            value["scopes"] = json!([scope]);
            assert!(serde_json::from_value::<NativePermissionV2>(value).is_err());
        }
    }
    #[test]
    fn native_origin_and_permission_semantics_are_closed() {
        for value in [
            "https://example.invalid:443",
            "https://a-b.example.invalid:65535",
        ] {
            assert!(origin(value));
        }
        for value in [
            "https://localhost:443",
            "https://127.0.0.1:443",
            "https://EXAMPLE.invalid:443",
            "https://example.invalid:0443",
            "https://a..invalid:443",
            "https://example.invalid:65536",
        ] {
            assert!(!origin(value));
        }
        let mut command = json!({"capabilityProfile":NATIVE_CAPABILITY_PROFILE,"effects":["file-read"],"permissions":[{"capability":"fs","operations":["read"],"scopes":[{"kind":"resource","slot":"source"}]}]});
        assert!(command_valid(&command));
        command["effects"] = json!([]);
        assert!(!command_valid(&command));
        command["effects"] = json!(["file-read"]);
        command["permissions"][0]["operations"] = json!(["read", "read"]);
        assert!(!command_valid(&command));
    }
}
