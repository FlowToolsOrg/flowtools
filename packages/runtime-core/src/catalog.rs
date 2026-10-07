use crate::protocol::ErrorCode;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct BuiltinCatalog {
    manifests: BTreeMap<String, Value>,
}

impl BuiltinCatalog {
    pub fn embedded() -> Self {
        let catalog: Value = serde_json::from_str(include_str!("../.generated/catalog.json"))
            .expect("Build-owned Manifest v1 artifact");
        let manifests = catalog["plugins"]
            .as_array()
            .expect("Plugin inventory")
            .iter()
            .map(|manifest| {
                (
                    manifest["id"].as_str().expect("Plugin ID").to_owned(),
                    manifest.clone(),
                )
            })
            .collect();
        Self { manifests }
    }

    pub fn list(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.manifests.iter()
    }

    // Fixed T1 only: full metadata must match the TS-validated build artifact.
    pub fn validate_manifest(&self, manifest: &Value) -> Result<(), ErrorCode> {
        let id = manifest["id"].as_str().ok_or(ErrorCode::PluginNotFound)?;
        if self.manifests.get(id) == Some(manifest) {
            Ok(())
        } else {
            Err(ErrorCode::PluginNotFound)
        }
    }

    pub fn command(&self, id: &str, command_id: &str) -> Result<(&Value, &Value), ErrorCode> {
        let manifest = self.manifests.get(id).ok_or(ErrorCode::PluginNotFound)?;
        let command = manifest["commands"]
            .as_array()
            .and_then(|commands| commands.iter().find(|command| command["id"] == command_id))
            .ok_or(ErrorCode::PluginNotFound)?;
        Ok((manifest, command))
    }

    pub fn prepare_input(
        &self,
        id: &str,
        command_id: &str,
        input: Value,
    ) -> Result<Value, ErrorCode> {
        self.prepare_input_mode(id, command_id, input, false)
    }

    pub fn prepare_input_mode(
        &self,
        id: &str,
        command_id: &str,
        mut input: Value,
        managed: bool,
    ) -> Result<Value, ErrorCode> {
        let (_, command) = self.command(id, command_id)?;
        // No side effects/capabilities before G3 broker/grants. Pure T1 evaluation only.
        if !(managed && id == "plugin-todo-list")
            && (command["effects"].as_array().is_none_or(|v| !v.is_empty())
                || command["permissions"]
                    .as_array()
                    .is_none_or(|v| !v.is_empty()))
        {
            return Err(ErrorCode::ApprovalRequired);
        }
        apply_defaults(&command["inputSchema"], &mut input);
        let budget = command["resources"]["maxInputBytes"].as_u64().unwrap_or(0);
        if serde_json::to_vec(&input)
            .map_err(|_| ErrorCode::InputInvalid)?
            .len() as u64
            > budget
            || !jsonschema::is_valid(&command["inputSchema"], &input)
        {
            return Err(ErrorCode::InputInvalid);
        }
        Ok(input)
    }

    pub fn output_valid(&self, id: &str, command_id: &str, output: &Value) -> bool {
        self.command(id, command_id).is_ok_and(|(_, command)| {
            serde_json::to_vec(output).is_ok_and(|bytes| {
                bytes.len() <= crate::protocol::MAX_FRAME_BYTES - 16_384
                    && bytes.len() as u64
                        <= command["resources"]["maxOutputBytes"].as_u64().unwrap_or(0)
            }) && jsonschema::is_valid(&command["outputSchema"], output)
        })
    }
}

pub fn digest(value: &Value) -> String {
    // serde_json's default BTreeMap gives canonical key order for this finite JSON contract.
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("JSON value"))
    )
}

fn apply_defaults(schema: &Value, input: &mut Value) {
    if let (Some(properties), Some(object)) =
        (schema["properties"].as_object(), input.as_object_mut())
    {
        for (key, property) in properties {
            if !object.contains_key(key) {
                if let Some(default) = property.get("default") {
                    object.insert(key.clone(), default.clone());
                }
            }
            if let Some(value) = object.get_mut(key) {
                apply_defaults(property, value);
            }
        }
    }
    if let Some(values) = input.as_array_mut() {
        for value in values {
            apply_defaults(&schema["items"], value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn complete_t1_metadata_and_schemas_share_ts_artifact() {
        let catalog = BuiltinCatalog::embedded();
        assert_eq!(catalog.list().count(), 12);
        for (_, manifest) in catalog.list() {
            assert!(catalog.validate_manifest(manifest).is_ok());
            let mut changed = manifest.clone();
            changed["entries"]["executor"] = json!("../escape.js");
            assert!(catalog.validate_manifest(&changed).is_err());
            changed = manifest.clone();
            changed["commands"][0]["headless"] = json!(false);
            assert!(catalog.validate_manifest(&changed).is_err());
        }
        let input = catalog
            .prepare_input("plugin-base64-encoder", "run", json!({"text":"hello"}))
            .unwrap();
        assert_eq!(input["mode"], "encode");
        assert_eq!(
            catalog
                .prepare_input("plugin-base64-encoder", "run", json!({"text":3}))
                .unwrap_err(),
            ErrorCode::InputInvalid
        );
        assert_eq!(
            catalog
                .prepare_input(
                    "plugin-base64-encoder",
                    "run",
                    json!({"text":"hello","pluginId":"spoof"})
                )
                .unwrap_err(),
            ErrorCode::InputInvalid
        );
        assert_eq!(
            catalog
                .prepare_input("plugin-todo-list", "run", json!({"todo":"private"}))
                .unwrap_err(),
            ErrorCode::ApprovalRequired
        );
    }
}
