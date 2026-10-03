use crate::{error::AppError, models::Plugin};
use serde::{Deserialize, Serialize};
use specta::Type;

const PLUGIN_TYPE_APP: &str = "app";
const PLUGIN_TYPE_TOOL: &str = "tool";
const PLUGIN_STATE_REGISTERED: &str = "registered";
const PLUGIN_STATE_LOADING: &str = "loading";
const PLUGIN_STATE_LOADED: &str = "loaded";
const PLUGIN_STATE_ENABLED: &str = "enabled";
const PLUGIN_STATE_DISABLED: &str = "disabled";
const PLUGIN_STATE_ERROR: &str = "error";

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PluginDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub link: Option<String>,
    pub r#type: String,
    pub permissions: Vec<String>,
    pub tags: Vec<String>,
    pub status: Option<String>,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub cli_available: bool,
    pub state: String,
    pub created_at: String,
}

impl From<Plugin> for PluginDto {
    fn from(plugin: Plugin) -> Self {
        Self {
            id: plugin.id,
            name: plugin.name,
            version: plugin.version,
            description: plugin.description,
            author: plugin.author,
            link: plugin.link,
            r#type: plugin.r#type,
            permissions: plugin.permissions,
            tags: plugin.tags,
            status: plugin.status,
            category: plugin.category,
            icon: plugin.icon,
            cli_available: plugin.cli_available,
            state: plugin.state,
            created_at: plugin.created_at.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreatePluginDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub link: Option<String>,
    pub r#type: String,
    pub permissions: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub cli_available: Option<bool>,
    pub state: Option<String>,
}

impl CreatePluginDto {
    pub fn normalize(self) -> Result<PluginWriteDto, AppError> {
        let state = self
            .state
            .unwrap_or_else(|| PLUGIN_STATE_REGISTERED.to_string());

        let plugin = PluginWriteDto {
            id: self.id,
            name: self.name,
            version: self.version,
            description: self.description,
            author: self.author,
            link: self.link,
            r#type: self.r#type,
            permissions: self.permissions.unwrap_or_default(),
            tags: self.tags.unwrap_or_default(),
            status: self.status,
            category: self.category,
            icon: self.icon,
            cli_available: self.cli_available.unwrap_or(false),
            state,
        };

        plugin.validate()?;
        Ok(plugin)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePluginDto {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub link: Option<String>,
    pub r#type: Option<String>,
    pub permissions: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub cli_available: Option<bool>,
    pub state: Option<String>,
}

impl UpdatePluginDto {
    pub fn merge(self, plugin: &Plugin) -> Result<PluginWriteDto, AppError> {
        let plugin = PluginWriteDto {
            id: plugin.id.clone(),
            name: self.name.unwrap_or_else(|| plugin.name.clone()),
            version: self.version.unwrap_or_else(|| plugin.version.clone()),
            description: self.description.or_else(|| plugin.description.clone()),
            author: self.author.or_else(|| plugin.author.clone()),
            link: self.link.or_else(|| plugin.link.clone()),
            r#type: self.r#type.unwrap_or_else(|| plugin.r#type.clone()),
            permissions: self
                .permissions
                .unwrap_or_else(|| plugin.permissions.clone()),
            tags: self.tags.unwrap_or_else(|| plugin.tags.clone()),
            status: self.status.or_else(|| plugin.status.clone()),
            category: self.category.or_else(|| plugin.category.clone()),
            icon: self.icon.or_else(|| plugin.icon.clone()),
            cli_available: self.cli_available.unwrap_or(plugin.cli_available),
            state: self.state.unwrap_or_else(|| plugin.state.clone()),
        };

        plugin.validate()?;
        Ok(plugin)
    }
}

pub struct PluginWriteDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub link: Option<String>,
    pub r#type: String,
    pub permissions: Vec<String>,
    pub tags: Vec<String>,
    pub status: Option<String>,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub cli_available: bool,
    pub state: String,
}

impl PluginWriteDto {
    pub fn validate_id(id: &str) -> Result<(), AppError> {
        validate_non_empty("id", id)?;

        let valid = id.split('-').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        });

        if valid {
            Ok(())
        } else {
            Err(AppError::from(
                "Plugin id must be kebab-case using lowercase letters, numbers, and dashes.",
            ))
        }
    }

    fn validate(&self) -> Result<(), AppError> {
        Self::validate_id(&self.id)?;
        validate_non_empty("name", &self.name)?;
        validate_non_empty("version", &self.version)?;
        validate_plugin_type(&self.r#type)?;
        validate_plugin_state(&self.state)?;
        Ok(())
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        Err(AppError::from(format!("{field} cannot be empty.")))
    } else {
        Ok(())
    }
}

fn validate_plugin_type(plugin_type: &str) -> Result<(), AppError> {
    match plugin_type {
        PLUGIN_TYPE_APP | PLUGIN_TYPE_TOOL => Ok(()),
        _ => Err(AppError::from("Plugin type must be 'app' or 'tool'.")),
    }
}

fn validate_plugin_state(state: &str) -> Result<(), AppError> {
    match state {
        PLUGIN_STATE_REGISTERED
        | PLUGIN_STATE_LOADING
        | PLUGIN_STATE_LOADED
        | PLUGIN_STATE_ENABLED
        | PLUGIN_STATE_DISABLED
        | PLUGIN_STATE_ERROR => Ok(()),
        _ => Err(AppError::from("Plugin state is not supported.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_plugin() -> CreatePluginDto {
        CreatePluginDto {
            id: "plugin-json-formatter".to_string(),
            name: "JSON Formatter".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Formats JSON".to_string()),
            author: Some("FlowTools".to_string()),
            link: Some("https://example.com/plugin-json-formatter".to_string()),
            r#type: PLUGIN_TYPE_APP.to_string(),
            permissions: Some(vec!["clipboard".to_string()]),
            tags: Some(vec!["json".to_string()]),
            status: Some("stable".to_string()),
            category: Some("development".to_string()),
            icon: Some("icon.png".to_string()),
            cli_available: Some(true),
            state: None,
        }
    }

    fn stored_plugin() -> Plugin {
        Plugin {
            id: "plugin-json-formatter".to_string(),
            name: "JSON Formatter".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Formats JSON".to_string()),
            author: Some("FlowTools".to_string()),
            link: None,
            r#type: PLUGIN_TYPE_APP.to_string(),
            permissions: vec!["storage".to_string()],
            tags: vec!["json".to_string()],
            status: Some("stable".to_string()),
            category: Some("development".to_string()),
            icon: None,
            cli_available: false,
            state: PLUGIN_STATE_REGISTERED.to_string(),
            created_at: "2026-07-18T00:00:00Z"
                .parse()
                .expect("timestamp should parse"),
        }
    }

    #[test]
    fn create_plugin_accepts_kebab_case_ids_and_defaults_to_registered() {
        for id in ["plugin", "plugin-json-formatter", "plugin-2"] {
            let mut input = create_plugin();
            input.id = id.to_string();

            let plugin = input.normalize().expect("valid plugin should normalize");
            assert_eq!(plugin.id, id);
            assert_eq!(plugin.state, PLUGIN_STATE_REGISTERED);
        }
    }

    #[test]
    fn create_plugin_applies_safe_optional_defaults() {
        let mut input = create_plugin();
        input.permissions = None;
        input.tags = None;
        input.cli_available = None;

        let plugin = input.normalize().expect("plugin should normalize");

        assert!(plugin.permissions.is_empty());
        assert!(plugin.tags.is_empty());
        assert!(!plugin.cli_available);
        assert_eq!(plugin.state, PLUGIN_STATE_REGISTERED);
    }

    #[test]
    fn create_plugin_rejects_non_kebab_case_ids() {
        for id in [
            "",
            "Plugin",
            "plugin_name",
            "plugin name",
            "-plugin",
            "plugin-",
            "plugin--name",
        ] {
            let mut input = create_plugin();
            input.id = id.to_string();

            assert!(input.normalize().is_err(), "id should be rejected: {id}");
        }
    }

    #[test]
    fn create_plugin_validates_type_state_name_and_version() {
        for plugin_type in [PLUGIN_TYPE_APP, PLUGIN_TYPE_TOOL] {
            let mut input = create_plugin();
            input.r#type = plugin_type.to_string();
            assert!(input.normalize().is_ok());
        }

        let mut invalid_type = create_plugin();
        invalid_type.r#type = "service".to_string();
        assert!(invalid_type.normalize().is_err());

        for state in [
            PLUGIN_STATE_REGISTERED,
            PLUGIN_STATE_LOADING,
            PLUGIN_STATE_LOADED,
            PLUGIN_STATE_ENABLED,
            PLUGIN_STATE_DISABLED,
            PLUGIN_STATE_ERROR,
        ] {
            let mut input = create_plugin();
            input.state = Some(state.to_string());
            assert!(input.normalize().is_ok());
        }

        let mut invalid_state = create_plugin();
        invalid_state.state = Some("unknown".to_string());
        assert!(invalid_state.normalize().is_err());

        let mut blank_name = create_plugin();
        blank_name.name = "   ".to_string();
        assert!(blank_name.normalize().is_err());

        let mut blank_version = create_plugin();
        blank_version.version = "".to_string();
        assert!(blank_version.normalize().is_err());
    }

    #[test]
    fn update_plugin_preserves_omitted_fields_and_replaces_explicit_fields() {
        let plugin = stored_plugin();
        let omitted = UpdatePluginDto {
            name: None,
            version: None,
            description: None,
            author: None,
            link: None,
            r#type: None,
            permissions: None,
            tags: None,
            status: None,
            category: None,
            icon: None,
            cli_available: None,
            state: None,
        };

        let unchanged = omitted.merge(&plugin).expect("update should merge");
        assert_eq!(unchanged.name, plugin.name);
        assert_eq!(unchanged.version, plugin.version);
        assert_eq!(unchanged.permissions, plugin.permissions);
        assert_eq!(unchanged.state, plugin.state);

        let explicit = UpdatePluginDto {
            name: Some("JSON Formatter 2".to_string()),
            version: Some("2.0.0".to_string()),
            description: None,
            author: None,
            link: None,
            r#type: None,
            permissions: Some(vec!["clipboard".to_string()]),
            tags: None,
            status: None,
            category: None,
            icon: None,
            cli_available: Some(true),
            state: Some(PLUGIN_STATE_DISABLED.to_string()),
        };

        let updated = explicit.merge(&plugin).expect("update should merge");
        assert_eq!(updated.name, "JSON Formatter 2");
        assert_eq!(updated.version, "2.0.0");
        assert_eq!(updated.permissions, vec!["clipboard"]);
        assert!(updated.cli_available);
        assert_eq!(updated.state, PLUGIN_STATE_DISABLED);
    }

    #[test]
    fn plugin_dto_serializes_frontend_fields_as_camel_case() {
        let dto = PluginDto {
            id: "plugin-json-formatter".to_string(),
            name: "JSON Formatter".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            link: None,
            r#type: PLUGIN_TYPE_APP.to_string(),
            permissions: Vec::new(),
            tags: Vec::new(),
            status: None,
            category: None,
            icon: None,
            cli_available: true,
            state: PLUGIN_STATE_REGISTERED.to_string(),
            created_at: "2026-07-18T00:00:00Z".to_string(),
        };

        let value = serde_json::to_value(dto).expect("plugin dto should serialize");

        assert_eq!(value["type"], PLUGIN_TYPE_APP);
        assert_eq!(value["cliAvailable"], true);
        assert_eq!(value["createdAt"], "2026-07-18T00:00:00Z");
        assert!(value.get("cli_available").is_none());
        assert!(value.get("created_at").is_none());
    }
}
