use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, toasty::Model,Type)]
pub struct Plugin {
    #[key]
    pub id: String,

    pub name: String,

    pub version: String,

    pub description: String,

    // pub type: String,

    pub permissions: Vec<String>,
}