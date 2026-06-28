use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, toasty::Model)]
pub struct Plugin {
    #[key]
    pub id: String,

    pub name: String,

    pub version: String,

    pub description: String,

    pub r#type: String,

    pub permissions: Vec<String>,

    #[auto]
    pub created_at: jiff::Timestamp,
}
