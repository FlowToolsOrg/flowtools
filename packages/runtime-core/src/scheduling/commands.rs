use super::store::ScheduleDefinition;
use crate::protocol::GrantTarget;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSchedule {
    pub request_key: String,
    pub target: GrantTarget,
    pub plugin_id: String,
    pub command_id: String,
    #[specta(type = specta_typescript::Unknown)]
    pub input: serde_json::Value,
    pub definition: ScheduleDefinition,
    pub timeout_ms: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleKey {
    pub id: String,
    pub revision: u32,
}

impl CreateSchedule {
    pub fn validate(&self) -> Result<(), crate::protocol::ErrorCode> {
        if uuid::Uuid::parse_str(&self.request_key).is_err()
            || self.timeout_ms == 0
            || self.timeout_ms > 300_000
        {
            return Err(crate::protocol::ErrorCode::InvalidRequest);
        }
        self.definition.cursor().map(|_| ())
    }
}
