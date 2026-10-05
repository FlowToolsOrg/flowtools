use crate::broker::{CommandIdentity, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

pub const PROTOCOL_MAJOR: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 1_048_576;
pub const CLIENT_VERSION: &str = "0.1.0";

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionProof {
    pub session_id: String,
    pub instance_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub request_id: String,
    pub session: Option<SessionProof>,
    pub call: Call,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(tag = "method", content = "payload", deny_unknown_fields)]
pub enum Call {
    #[serde(rename = "session.open")]
    Open(OpenSession),
    #[serde(rename = "runtime.status")]
    Status,
    #[serde(rename = "plugins.list")]
    Plugins,
    #[serde(rename = "jobs.submit")]
    Submit(SubmitJob),
    #[serde(rename = "jobs.lookup")]
    Lookup(IdempotencyKey),
    #[serde(rename = "jobs.status")]
    Job(JobKey),
    #[serde(rename = "jobs.cancel")]
    Cancel(JobKey),
    #[serde(rename = "jobs.events")]
    Events(EventCursor),
    #[serde(rename = "data.read")]
    DataRead(DataRead),
    #[serde(rename = "data.write")]
    DataWrite(DataWrite),
    #[serde(rename = "data.transaction")]
    DataTransaction(DataTransaction),
    #[serde(rename = "data.import-legacy")]
    DataImport(DataImport),
    #[serde(rename = "permissions.list")]
    Permissions,
    #[serde(rename = "permissions.grant")]
    Grant(PermissionGrant),
    #[serde(rename = "permissions.revoke")]
    Revoke(PermissionKey),
    #[serde(rename = "policy.set")]
    Policy(BootstrapPolicy),
    #[serde(rename = "policy.import")]
    PolicyImport(PolicyImport),
    #[serde(rename = "runtime.stop")]
    Stop,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum GrantTarget {
    Cli,
    Desktop,
}
impl GrantTarget {
    pub fn caller(&self) -> &'static str {
        match self {
            Self::Cli => "local-cli",
            Self::Desktop => "local-desktop",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionKey {
    pub plugin_id: String,
    pub command_id: String,
    pub target: GrantTarget,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionGrant {
    pub plugin_id: String,
    pub command_id: String,
    pub target: GrantTarget,
    pub package_digest: String,
    pub effects: Vec<String>,
    pub scopes: Vec<Scope>,
    pub expires_at: f64,
    pub max_calls: u32,
    pub cold_start: bool,
    pub background: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionRecord {
    pub identity: CommandIdentity,
    pub epoch: u32,
    pub grant: Option<PermissionGrant>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BootstrapPolicy {
    pub cold_start: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyImport {
    pub format_version: u16,
    pub cold_start: bool,
    pub grants: Vec<PermissionGrant>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenSession {
    pub token: String,
    pub client_version: String,
    pub expected_instance_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubmitJob {
    pub plugin_id: String,
    pub command_id: String,
    #[specta(type = specta_typescript::Unknown)]
    pub input: Value,
    pub idempotency_key: String,
    pub background: bool,
    pub deadline: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobKey {
    pub run_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdempotencyKey {
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventCursor {
    pub run_id: String,
    pub after_sequence: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataRead {
    pub plugin_id: String,
    pub key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataMutation {
    pub key: String,
    pub expected_revision: u32,
    #[specta(type = specta_typescript::Unknown)]
    pub value: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataWrite {
    pub plugin_id: String,
    pub mutation: DataMutation,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataTransaction {
    pub plugin_id: String,
    pub mutations: Vec<DataMutation>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataSnapshot {
    pub key: String,
    pub revision: u32,
    #[specta(type = specta_typescript::Unknown)]
    pub value: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LegacySource {
    CliV0,
    DesktopLocalstorageV1,
    WebLocalstorageV1,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyImport {
    pub source: LegacySource,
    pub source_digest: String,
    #[specta(type = specta_typescript::Unknown)]
    pub value: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataImport {
    pub plugin_id: String,
    pub import: LegacyImport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    #[serde(rename = "PROTOCOL_MISMATCH")]
    ProtocolMismatch,
    #[serde(rename = "CLIENT_INCOMPATIBLE")]
    ClientIncompatible,
    #[serde(rename = "FRAME_TOO_LARGE")]
    FrameTooLarge,
    #[serde(rename = "INVALID_REQUEST")]
    InvalidRequest,
    #[serde(rename = "SESSION_INVALID")]
    SessionInvalid,
    #[serde(rename = "INSTANCE_MISMATCH")]
    InstanceMismatch,
    #[serde(rename = "APPROVAL_REQUIRED")]
    ApprovalRequired,
    #[serde(rename = "INTERACTION_REQUIRED")]
    InteractionRequired,
    #[serde(rename = "CAPABILITY_UNDECLARED")]
    CapabilityUndeclared,
    #[serde(rename = "SCOPE_DENIED")]
    ScopeDenied,
    #[serde(rename = "GRANT_REVOKED")]
    GrantRevoked,
    #[serde(rename = "BUDGET_EXCEEDED")]
    BudgetExceeded,
    #[serde(rename = "REVISION_CONFLICT")]
    RevisionConflict,
    #[serde(rename = "STORE_BUSY")]
    StoreBusy,
    #[serde(rename = "STORE_CORRUPT")]
    StoreCorrupt,
    #[serde(rename = "SCHEMA_UNSUPPORTED")]
    SchemaUnsupported,
    #[serde(rename = "STORAGE_FAILED")]
    StorageFailed,
    #[serde(rename = "ACCEPTANCE_UNKNOWN")]
    AcceptanceUnknown,
    #[serde(rename = "EXECUTION_INTERRUPTED")]
    ExecutionInterrupted,
    #[serde(rename = "RESULT_EXPIRED")]
    ResultExpired,
    #[serde(rename = "COLD_START_DENIED")]
    ColdStartDenied,
    #[serde(rename = "PLUGIN_NOT_FOUND")]
    PluginNotFound,
    #[serde(rename = "INPUT_INVALID")]
    InputInvalid,
    #[serde(rename = "JOB_NOT_FOUND")]
    JobNotFound,
    #[serde(rename = "IDEMPOTENCY_CONFLICT")]
    IdempotencyConflict,
    #[serde(rename = "TIMEOUT")]
    Timeout,
    #[serde(rename = "ABORTED")]
    Aborted,
    #[serde(rename = "EXECUTION_FAILED")]
    ExecutionFailed,
    #[serde(rename = "OUTPUT_INVALID")]
    OutputInvalid,
    #[serde(rename = "RUNTIME_BUSY")]
    RuntimeBusy,
    #[serde(rename = "RUNTIME_DISCONNECTED")]
    RuntimeDisconnected,
    #[serde(rename = "INVALID_RESPONSE")]
    InvalidResponse,
}

impl ErrorCode {
    pub const ALL: [Self; 32] = [
        Self::AcceptanceUnknown,
        Self::ExecutionInterrupted,
        Self::ResultExpired,
        Self::ColdStartDenied,
        Self::ProtocolMismatch,
        Self::ClientIncompatible,
        Self::FrameTooLarge,
        Self::InvalidRequest,
        Self::SessionInvalid,
        Self::InstanceMismatch,
        Self::ApprovalRequired,
        Self::InteractionRequired,
        Self::CapabilityUndeclared,
        Self::ScopeDenied,
        Self::GrantRevoked,
        Self::BudgetExceeded,
        Self::RevisionConflict,
        Self::StoreBusy,
        Self::StoreCorrupt,
        Self::SchemaUnsupported,
        Self::StorageFailed,
        Self::PluginNotFound,
        Self::InputInvalid,
        Self::JobNotFound,
        Self::IdempotencyConflict,
        Self::Timeout,
        Self::Aborted,
        Self::ExecutionFailed,
        Self::OutputInvalid,
        Self::RuntimeBusy,
        Self::RuntimeDisconnected,
        Self::InvalidResponse,
    ];
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeError {
    pub code: ErrorCode,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    pub version: u16,
    pub request_id: String,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(tag = "type", content = "data", deny_unknown_fields)]
pub enum Outcome {
    #[serde(rename = "session")]
    Session(SessionProof),
    #[serde(rename = "status")]
    Status(RuntimeStatus),
    #[serde(rename = "plugins")]
    Plugins(Vec<PluginStatus>),
    #[serde(rename = "receipt")]
    Receipt(JobReceipt),
    #[serde(rename = "job")]
    Job(Box<JobSnapshot>),
    #[serde(rename = "events")]
    Events(Vec<JobEvent>),
    #[serde(rename = "data")]
    Data(DataSnapshot),
    #[serde(rename = "data-batch")]
    DataBatch(Vec<DataSnapshot>),
    #[serde(rename = "permissions")]
    Permissions(Vec<PermissionRecord>),
    #[serde(rename = "policy")]
    Policy(BootstrapPolicy),
    #[serde(rename = "stopping")]
    Stopping,
    #[serde(rename = "error")]
    Error(RuntimeError),
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeStatus {
    pub instance_id: String,
    pub mode: String,
    pub jobs: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginStatus {
    pub plugin_id: String,
    pub version: String,
    pub installed: bool,
    pub enabled: bool,
    pub running: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Accepted,
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl JobState {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobReceipt {
    pub format_version: u16,
    pub receipt_type: String,
    pub run_id: String,
    pub instance_id: String,
    pub accepted_at: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobSnapshot {
    pub format_version: u16,
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub root_caller: String,
    pub plugin_id: String,
    pub command_id: String,
    pub package_version: String,
    pub package_digest: String,
    pub dependency_lock: String,
    pub deadline: f64,
    pub grant_epoch: u32,
    #[specta(type = specta_typescript::Unknown)]
    pub resources: Value,
    pub state: JobState,
    pub sequence: u32,
    pub result: Option<ExecutionResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputSummary {
    pub kind: String,
    pub size: u32,
}

/// Versioned extension of PluginExecutionResult; a receipt is a different type.
#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ExecutionResult {
    pub format_version: u16,
    pub run_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub started_at: f64,
    pub finished_at: f64,
    pub duration_ms: f64,
    pub input_summary: InputSummary,
    #[serde(flatten)]
    pub outcome: ExecutionOutcome,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(untagged)]
pub enum ExecutionOutcome {
    Success {
        success: bool,
        #[specta(type = specta_typescript::Unknown)]
        data: Value,
    },
    Failure {
        success: bool,
        error: RuntimeError,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobEvent {
    pub run_id: String,
    pub sequence: u32,
    pub state: JobState,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_result_roundtrips_through_native_client() {
        for outcome in [
            ExecutionOutcome::Success {
                success: true,
                data: serde_json::json!({"type":"text","value":"fixture"}),
            },
            ExecutionOutcome::Failure {
                success: false,
                error: RuntimeError {
                    code: ErrorCode::Aborted,
                },
            },
        ] {
            let result = ExecutionResult {
                format_version: 1,
                run_id: "fixture".into(),
                plugin_id: "plugin-base64-encoder".into(),
                plugin_version: "0.1.0".into(),
                started_at: 1.0,
                finished_at: 2.0,
                duration_ms: 1.0,
                input_summary: InputSummary {
                    kind: "object".into(),
                    size: 1,
                },
                outcome,
            };
            let bytes = serde_json::to_vec(&result).unwrap();
            let decoded: ExecutionResult = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(bytes, serde_json::to_vec(&decoded).unwrap());
        }
    }
}
