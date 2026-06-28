use crate::models::Plugin;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PluginDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub r#type: String,
    pub permissions: Vec<String>,
    pub created_at: String,
}

impl From<Plugin> for PluginDto {
    fn from(plugin: Plugin) -> Self {
        Self {
            id: plugin.id,
            name: plugin.name,
            version: plugin.version,
            description: plugin.description,
            r#type: plugin.r#type,
            permissions: plugin.permissions,
            created_at: plugin.created_at.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreatePluginDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub r#type: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdatePluginDto {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub r#type: Option<String>,
    pub permissions: Option<Vec<String>>,
}
