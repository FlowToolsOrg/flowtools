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
            .unwrap_or_else(|| PLUGIN_STATE_ENABLED.to_string());

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

        let valid = id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-');

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
