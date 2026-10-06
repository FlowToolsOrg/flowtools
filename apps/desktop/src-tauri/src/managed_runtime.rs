//! Current-user native Host client. Never exposes endpoint, tokens or executable selectors.
use flowtools_runtime_core::protocol::*;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Command, Stdio},
};
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient},
    sync::Mutex,
};

#[derive(Default)]
pub struct ManagedConnections(pub std::sync::Arc<Mutex<HashMap<String, NamedPipeClient>>>);
fn error(code: ErrorCode) -> RuntimeError {
    RuntimeError { code }
}
fn guard(debug: bool, label: &str, origin: bool) -> bool {
    debug && label == "main" && origin
}
fn check<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> Result<PathBuf, RuntimeError> {
    let url = window.url().map_err(|_| error(ErrorCode::SessionInvalid))?;
    let config = window.app_handle().config();
    if !guard(
        cfg!(debug_assertions),
        window.label(),
        config
            .build
            .dev_url
            .as_ref()
            .is_some_and(|dev| dev.origin() == url.origin()),
    ) {
        return Err(error(ErrorCode::SessionInvalid));
    }
    if config.identifier == crate::RUNTIME_VALIDATION_IDENTIFIER
        && std::env::var("FLOWTOOLS_RUNTIME_VALIDATION").as_deref() == Ok("1")
    {
        if let Some(path) = std::env::var_os("FLOWTOOLS_MANAGED_VALIDATION_PROFILE") {
            let path = PathBuf::from(path);
            if path.is_absolute()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("flowtools-validation-"))
            {
                return Ok(path);
            }
            return Err(error(ErrorCode::InvalidRequest));
        }
    }
    std::env::var_os("LOCALAPPDATA")
        .map(|root| PathBuf::from(root).join("FlowToolsRuntimeV1"))
        .ok_or_else(|| error(ErrorCode::ApprovalRequired))
}
fn executable() -> Result<PathBuf, RuntimeError> {
    let path = PathBuf::from(env!("FLOWTOOLS_DESKTOP_RUNTIME_PATH"));
    flowtools_runtime_core::data::safe_path(&path).map_err(error)?;
    if !path.is_file()
        || format!(
            "{:x}",
            Sha256::digest(std::fs::read(&path).map_err(|_| error(ErrorCode::StorageFailed))?)
        ) != env!("FLOWTOOLS_DESKTOP_RUNTIME_SHA256")
    {
        return Err(error(ErrorCode::ExecutionFailed));
    }
    Ok(path)
}
fn command(mode: &str, profile: &std::path::Path) -> Result<Command, RuntimeError> {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new(executable()?);
    command
        .args([mode])
        .arg(profile)
        .creation_flags(0x08000000)
        .env_clear();
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    Ok(command)
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Bootstrap {
    pipe: String,
    token: String,
}
async fn bootstrap(profile: PathBuf, manager: bool, cold: bool) -> Result<Bootstrap, RuntimeError> {
    if cold {
        let output = native_output("--ensure-runtime", &profile).await?;
        if !output.status.success() {
            let code = match std::str::from_utf8(&output.stderr).unwrap_or("").trim() {
                "RECOVERY_PENDING" => ErrorCode::RecoveryPending,
                "STORE_CORRUPT" => ErrorCode::StoreCorrupt,
                "STORE_BUSY" => ErrorCode::StoreBusy,
                "SCHEMA_UNSUPPORTED" => ErrorCode::SchemaUnsupported,
                "STORAGE_FAILED" => ErrorCode::StorageFailed,
                _ => ErrorCode::ColdStartDenied,
            };
            return Err(error(code));
        }
    }
    let output = native_output(
        if manager {
            "--manager-bootstrap"
        } else {
            "--desktop-bootstrap"
        },
        &profile,
    )
    .await?;
    if !output.status.success() || output.stdout.len() > 2048 {
        return Err(error(ErrorCode::ApprovalRequired));
    }
    let value: Bootstrap =
        serde_json::from_slice(&output.stdout).map_err(|_| error(ErrorCode::InvalidResponse))?;
    let suffix = value
        .pipe
        .strip_prefix(r"\\.\pipe\flowtools-managed-")
        .ok_or_else(|| error(ErrorCode::InvalidResponse))?;
    if suffix.len() != 64
        || !suffix.bytes().all(|b| b.is_ascii_hexdigit())
        || value.token.len() != 64
    {
        return Err(error(ErrorCode::InvalidResponse));
    }
    Ok(value)
}
async fn native_output(
    mode: &str,
    profile: &std::path::Path,
) -> Result<std::process::Output, RuntimeError> {
    let mut launcher = tokio::process::Command::from(command(mode, profile)?);
    let mut child = launcher
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| error(ErrorCode::RuntimeDisconnected))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| error(ErrorCode::RuntimeDisconnected))?;
    let mut reader = BufReader::new(stdout.take(2048));
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| error(ErrorCode::RuntimeDisconnected))?;
    let mut errors = BufReader::new(stderr.take(256));
    let mut failure = String::new();
    let mut line = String::new();
    tokio::time::timeout(std::time::Duration::from_secs(45), async {
        let (read, status) = tokio::join!(reader.read_line(&mut line), child.wait());
        read.map_err(|_| error(ErrorCode::RuntimeDisconnected))?;
        let status = status.map_err(|_| error(ErrorCode::RuntimeDisconnected))?;
        // A detached Host can retain an inherited stderr handle after the helper
        // exits. Successful helpers have no diagnostic stderr to wait for.
        if !status.success() {
            errors
                .read_line(&mut failure)
                .await
                .map_err(|_| error(ErrorCode::RuntimeDisconnected))?;
        }
        Ok(std::process::Output {
            status,
            stdout: line.into_bytes(),
            stderr: failure.into_bytes(),
        })
    })
    .await
    .map_err(|_| error(ErrorCode::Timeout))?
}

async fn exchange(pipe: &mut NamedPipeClient, request: &Request) -> Result<Response, RuntimeError> {
    let bytes = serde_json::to_vec(request).map_err(|_| error(ErrorCode::InvalidRequest))?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(error(ErrorCode::FrameTooLarge));
    }
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        pipe.write_u32_le(bytes.len() as u32).await?;
        pipe.write_all(&bytes).await?;
        let size = pipe.read_u32_le().await? as usize;
        if size > MAX_FRAME_BYTES {
            return Err(std::io::Error::other("Frame budget"));
        }
        let mut bytes = vec![0; size];
        pipe.read_exact(&mut bytes).await?;
        let response: Response = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
        if response.request_id != request.request_id || response.version != PROTOCOL_MAJOR {
            return Err(std::io::Error::other("Invalid response"));
        }
        Ok(response)
    })
    .await
    .map_err(|_| error(ErrorCode::RuntimeDisconnected))?
    .map_err(|_| error(ErrorCode::RuntimeDisconnected))
}
fn business(call: &Call) -> bool {
    matches!(
        call,
        Call::Open(_)
            | Call::Status
            | Call::Plugins
            | Call::Jobs
            | Call::Submit(_)
            | Call::Lookup(_)
            | Call::Job(_)
            | Call::Diagnose(_)
            | Call::Cancel(_)
            | Call::Events(_)
            | Call::DataRead(_)
            | Call::DataWrite(_)
            | Call::DataTransaction(_)
            | Call::DataImport(_)
    )
}
#[tauri::command]
#[specta::specta]
pub async fn managed_runtime<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    state: tauri::State<'_, ManagedConnections>,
    mut request: Request,
) -> Result<Response, RuntimeError> {
    let profile = check(&window)?;
    if !business(&request.call) {
        return Err(error(ErrorCode::SessionInvalid));
    }
    let mut connections = state.0.lock().await;
    if let Call::Open(open) = &mut request.call {
        if connections.len() >= 8 || request.session.is_some() {
            return Err(error(ErrorCode::RuntimeBusy));
        }
        let native = bootstrap(profile, false, true).await?;
        open.token = native.token;
        let mut pipe = ClientOptions::new()
            .open(native.pipe)
            .map_err(|_| error(ErrorCode::RuntimeDisconnected))?;
        let response = exchange(&mut pipe, &request).await?;
        if let Outcome::Session(proof) = &response.outcome {
            connections.insert(proof.session_id.clone(), pipe);
        }
        return Ok(response);
    }
    let session = request
        .session
        .as_ref()
        .ok_or_else(|| error(ErrorCode::SessionInvalid))?
        .session_id
        .clone();
    let pipe = connections
        .get_mut(&session)
        .ok_or_else(|| error(ErrorCode::SessionInvalid))?;
    let result = exchange(pipe, &request).await;
    if result.is_err() {
        connections.remove(&session);
    }
    result
}
#[tauri::command]
#[specta::specta]
pub async fn managed_runtime_disconnect<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    state: tauri::State<'_, ManagedConnections>,
    session_id: String,
) -> Result<(), RuntimeError> {
    check(&window)?;
    state.0.lock().await.remove(&session_id);
    Ok(())
}
struct TemporaryManager(tokio::process::Child);
impl TemporaryManager {
    async fn close(mut self) {
        self.0.stdin.take();
        if tokio::time::timeout(std::time::Duration::from_secs(3), self.0.wait())
            .await
            .is_err()
        {
            let _ = self.0.kill().await;
            let _ = self.0.wait().await;
        }
    }
}
async fn manager(
    profile: PathBuf,
    initialize: bool,
) -> Result<(NamedPipeClient, SessionProof, Option<TemporaryManager>), RuntimeError> {
    let mut child = None;
    if initialize {
        child = Some(spawn_manager(&profile, true).await?);
    }
    let native = bootstrap(profile.clone(), true, false).await?;
    let mut pipe = match ClientOptions::new().open(&native.pipe) {
        Ok(pipe) => pipe,
        Err(io) if !initialize && io.kind() == std::io::ErrorKind::NotFound => {
            child = Some(spawn_manager(&profile, false).await?);
            ClientOptions::new()
                .open(&native.pipe)
                .map_err(|_| error(ErrorCode::RuntimeDisconnected))?
        }
        Err(_) => return Err(error(ErrorCode::RuntimeDisconnected)),
    };
    let response = exchange(
        &mut pipe,
        &Request {
            version: 1,
            request_id: uuid(),
            session: None,
            call: Call::Open(OpenSession {
                token: native.token,
                client_version: CLIENT_VERSION.into(),
                expected_instance_id: None,
            }),
        },
    )
    .await?;
    match response.outcome {
        Outcome::Session(proof) => Ok((pipe, proof, child)),
        Outcome::Error(failure) => Err(failure),
        _ => Err(error(ErrorCode::InvalidResponse)),
    }
}
fn uuid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("desktop-native-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}
async fn spawn_manager(
    profile: &std::path::Path,
    initialize: bool,
) -> Result<TemporaryManager, RuntimeError> {
    let profile = profile.to_owned();
    let mut launcher = tokio::process::Command::from(command(
        if initialize {
            "--initialize-profile"
        } else {
            "--profile"
        },
        &profile,
    )?);
    let mut child = launcher
        .arg("--management")
        .env("FLOWTOOLS_RUNTIME_VALIDATION_STDIN_LIFETIME", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| error(ErrorCode::StorageFailed))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| error(ErrorCode::StorageFailed))?;
    let mut reader = BufReader::new(stdout.take(2048));
    let mut line = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        reader.read_line(&mut line),
    )
    .await
    .map_err(|_| error(ErrorCode::Timeout))?
    .map_err(|_| error(ErrorCode::RuntimeDisconnected))?;
    let ready: serde_json::Value =
        serde_json::from_str(&line).map_err(|_| error(ErrorCode::RuntimeDisconnected))?;
    if ready["type"] != "ready" || ready["protocolMajor"] != 1 {
        return Err(error(ErrorCode::InvalidResponse));
    }
    Ok(TemporaryManager(child))
}

#[tauri::command]
#[specta::specta]
pub async fn managed_runtime_control<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    call: Call,
    initialize: bool,
) -> Result<Outcome, RuntimeError> {
    let profile = check(&window)?;
    if !matches!(
        call,
        Call::Permissions
            | Call::Plugins
            | Call::Grant(_)
            | Call::Revoke(_)
            | Call::Policy(_)
            | Call::PolicyImport(_)
            | Call::Stop
    ) || (initialize && !matches!(call, Call::PolicyImport(_)))
    {
        return Err(error(ErrorCode::SessionInvalid));
    }
    if !matches!(call, Call::Permissions | Call::Plugins | Call::Revoke(_)) {
        let encoded = serde_json::to_string(&call).map_err(|_| error(ErrorCode::InvalidRequest))?;
        if encoded.len() > 8192 {
            return Err(error(ErrorCode::FrameTooLarge));
        }
        let app = window.app_handle().clone();
        let approved = tokio::task::spawn_blocking(move || {
            app.dialog()
                .message(format!(
                    "批准此 Runtime 操作？\n{}\n停止 Runtime 会取消所有活动任务。",
                    encoded
                ))
                .title("FlowTools Runtime approval")
                .buttons(MessageDialogButtons::OkCancel)
                .blocking_show()
        })
        .await
        .map_err(|_| error(ErrorCode::InteractionRequired))?;
        if !approved {
            return Err(error(ErrorCode::ApprovalRequired));
        }
    }
    let (mut pipe, proof, child) = manager(profile, initialize).await?;
    let result = exchange(
        &mut pipe,
        &Request {
            version: 1,
            request_id: uuid(),
            session: Some(proof),
            call,
        },
    )
    .await?;
    drop(pipe);
    if let Some(child) = child {
        child.close().await;
    }
    match result.outcome {
        Outcome::Error(failure) => Err(failure),
        outcome => Ok(outcome),
    }
}
#[tauri::command]
#[specta::specta]
pub async fn managed_runtime_storage<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    action: StorageAction,
) -> Result<StorageReport, RuntimeError> {
    let profile = check(&window)?;
    flowtools_runtime_core::recovery::validate_action(&action).map_err(error)?;
    if !matches!(action, StorageAction::List) {
        let encoded =
            serde_json::to_string(&action).map_err(|_| error(ErrorCode::InvalidRequest))?;
        let app = window.app_handle().clone();
        let approved = tokio::task::spawn_blocking(move || app.dialog().message(format!("批准离线数据操作？\n{encoded}\n恢复会回滚数据、撤销全部授权并中断未完成任务。原数据将保留；不会自动重放任务。请先停止 Runtime。")) .title("FlowTools storage approval").buttons(MessageDialogButtons::OkCancel).blocking_show()).await.map_err(|_|error(ErrorCode::InteractionRequired))?;
        if !approved {
            return Err(error(ErrorCode::ApprovalRequired));
        }
    }
    let mut launcher = tokio::process::Command::from(command("--storage", &profile)?);
    let mut child = launcher
        .arg(serde_json::to_string(&action).map_err(|_| error(ErrorCode::InvalidRequest))?)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| error(ErrorCode::StorageFailed))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| error(ErrorCode::StorageFailed))?;
    let mut reader = BufReader::new(stdout.take(MAX_FRAME_BYTES as u64));
    let mut line = String::new();
    let (read, status) = tokio::time::timeout(std::time::Duration::from_secs(45), async {
        tokio::join!(reader.read_line(&mut line), child.wait())
    })
    .await
    .map_err(|_| error(ErrorCode::Timeout))?;
    read.map_err(|_| error(ErrorCode::StorageFailed))?;
    if !status
        .map_err(|_| error(ErrorCode::StorageFailed))?
        .success()
    {
        #[derive(serde::Deserialize)]
        struct Failure {
            error: RuntimeError,
        }
        return Err(serde_json::from_str::<Failure>(&line)
            .map(|f| f.error)
            .unwrap_or_else(|_| error(ErrorCode::StorageFailed)));
    }
    serde_json::from_str(&line).map_err(|_| error(ErrorCode::InvalidResponse))
}
pub fn destroyed<R: tauri::Runtime>(window: &tauri::Window<R>) {
    if window.label() == "main" {
        if let Some(state) = window.try_state::<ManagedConnections>() {
            let connections = state.0.clone();
            tauri::async_runtime::spawn(async move {
                connections.lock().await.clear();
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cold_helper_finishes_while_the_started_host_is_still_running() {
        let profile = std::env::temp_dir().join(format!(
            "flowtools-validation-desktop-cold-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (pipe, _, temporary) = manager(profile.clone(), true).await.unwrap();
        drop(pipe);
        temporary.unwrap().close().await;
        let denied = native_output("--ensure-runtime", &profile).await.unwrap();
        assert!(!denied.status.success());
        assert_eq!(
            String::from_utf8(denied.stderr).unwrap().trim(),
            "COLD_START_DENIED"
        );
        let (mut pipe, proof, temporary) = manager(profile.clone(), false).await.unwrap();
        let response = exchange(
            &mut pipe,
            &Request {
                version: PROTOCOL_MAJOR,
                request_id: uuid(),
                session: Some(proof),
                call: Call::Policy(BootstrapPolicy { cold_start: true }),
            },
        )
        .await
        .unwrap();
        assert!(matches!(response.outcome, Outcome::Policy(_)));
        drop(pipe);
        temporary.unwrap().close().await;

        let cold = tokio::time::timeout(
            std::time::Duration::from_secs(12),
            native_output("--ensure-runtime", &profile),
        )
        .await;
        // Always stop the exact disposable Host, including when the regression fails.
        let (mut pipe, proof, temporary) = manager(profile.clone(), false).await.unwrap();
        assert!(
            temporary.is_none(),
            "cold helper must have started the Host"
        );
        let status = exchange(
            &mut pipe,
            &Request {
                version: PROTOCOL_MAJOR,
                request_id: uuid(),
                session: Some(proof.clone()),
                call: Call::Stop,
            },
        )
        .await
        .unwrap();
        assert!(matches!(status.outcome, Outcome::Stopping));
        drop(pipe);
        assert!(
            cold.is_ok(),
            "native output waited for background Host stderr EOF"
        );
        let output = cold.unwrap().unwrap();
        assert!(output.status.success());
        let endpoint: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(endpoint["type"], "endpoint");
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn managed_bridge_is_native_origin_bound_and_manager_calls_never_use_business_transport() {
        for debug in [false, true] {
            for label in ["main", "external"] {
                for origin in [false, true] {
                    assert_eq!(
                        guard(debug, label, origin),
                        debug && label == "main" && origin
                    );
                }
            }
        }
        assert!(!business(&Call::Permissions));
        assert!(!business(&Call::Stop));
        assert!(!business(&Call::Policy(BootstrapPolicy {
            cold_start: true
        })));
        assert!(business(&Call::Jobs));
    }
}
