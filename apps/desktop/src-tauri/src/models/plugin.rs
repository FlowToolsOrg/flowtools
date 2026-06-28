use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, toasty::Model)]
pub struct Plugin {
    #[key]
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

    #[auto]
    pub created_at: jiff::Timestamp,
}
