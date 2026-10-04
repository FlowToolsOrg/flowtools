use crate::security::{create_pipe, current_user_sid};
use flowtools_runtime_core::{
    protocol::*,
    runtime::{now, RunSpec, RuntimeCore},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::NamedPipeServer,
    sync::{mpsc, Mutex, Semaphore},
};

type Core = Arc<Mutex<RuntimeCore>>;

fn token(name: &str) -> Result<String, &'static str> {
    let value = std::env::var(name).map_err(|_| "VALIDATION_TOKEN_REQUIRED")?;
    if value.len() != 64 || !value.bytes().all(|v| v.is_ascii_hexdigit()) {
        return Err("VALIDATION_TOKEN_REQUIRED");
    }
    Ok(value)
}

fn profile() -> Result<PathBuf, &'static str> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // No default launch, production profile, arbitrary executable or network mode.
    if args.len() != 3 || args[0] != "--validation-profile" || args[2] != "--validation" {
        return Err("SETUP_REQUIRED");
    }
    let path = PathBuf::from(&args[1]);
    if !path.is_absolute()
        || !path
            .file_name()
            .is_some_and(|s| s.to_string_lossy().starts_with("flowtools-validation-"))
    {
        return Err("VALIDATION_PROFILE_REQUIRED");
    }
    // Refuse redirection in every existing ancestor before any write.
    for ancestor in path.ancestors() {
        if ancestor.exists() {
            let metadata =
                std::fs::symlink_metadata(ancestor).map_err(|_| "VALIDATION_PROFILE_REQUIRED")?;
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("VALIDATION_PROFILE_REQUIRED");
            }
        }
    }
    std::fs::create_dir_all(&path).map_err(|_| "VALIDATION_PROFILE_REQUIRED")?;
    let marker = path.join("validation-profile-v1");
    if marker.exists() {
        use std::os::windows::fs::MetadataExt;
        let metadata =
            std::fs::symlink_metadata(&marker).map_err(|_| "VALIDATION_PROFILE_REQUIRED")?;
        if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
            return Err("VALIDATION_PROFILE_REQUIRED");
        }
        if std::fs::read(&marker).map_err(|_| "VALIDATION_PROFILE_REQUIRED")?
            != b"flowtools-disposable-v1"
        {
            return Err("VALIDATION_PROFILE_REQUIRED");
        }
    } else {
        if std::fs::read_dir(&path)
            .map_err(|_| "VALIDATION_PROFILE_REQUIRED")?
            .next()
            .is_some()
        {
            return Err("VALIDATION_PROFILE_REQUIRED");
        }
        std::fs::write(marker, b"flowtools-disposable-v1")
            .map_err(|_| "VALIDATION_PROFILE_REQUIRED")?;
    }
    std::fs::canonicalize(path).map_err(|_| "VALIDATION_PROFILE_REQUIRED")
}

pub async fn run() -> Result<(), &'static str> {
    // Validate explicit mode and bootstrap before touching profile data.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.contains(&"--validation".into()) {
        return Err("SETUP_REQUIRED");
    }
    let cli_token = token("FLOWTOOLS_RUNTIME_VALIDATION_CLI_TOKEN")?;
    let desktop_token = token("FLOWTOOLS_RUNTIME_VALIDATION_DESKTOP_TOKEN")?;
    if cli_token == desktop_token {
        return Err("VALIDATION_TOKEN_REQUIRED");
    }
    let path = profile()?;
    let sid = current_user_sid().map_err(|_| "IPC_ACCESS_FAILED")?;
    let digest = Sha256::digest(format!("{sid}|{}", path.display()).as_bytes());
    let pipe_name = format!(r"\\.\pipe\flowtools-validation-{digest:x}");
    let mut listener =
        create_pipe(&pipe_name, &sid, true).map_err(|_| "RUNTIME_ALREADY_RUNNING")?;
    let core = Arc::new(Mutex::new(RuntimeCore::validation(
        cli_token,
        desktop_token,
    )));
    println!(
        "{}",
        json!({"type":"ready", "pipe":pipe_name, "protocolMajor":1})
    );
    let mut connections = tokio::task::JoinSet::new();
    let mut workers = tokio::task::JoinSet::new();
    let (tasks, mut pending_tasks) = mpsc::channel::<String>(128);
    let mut scheduled = HashSet::new();
    let permits = Arc::new(Semaphore::new(4));
    let stdin_lifetime =
        std::env::var("FLOWTOOLS_RUNTIME_VALIDATION_STDIN_LIFETIME").as_deref() == Ok("1");
    let mut stdin = tokio::io::stdin();
    let mut byte = [0u8; 1];
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = stdin.read(&mut byte), if stdin_lifetime => break,
            result = listener.connect(), if connections.len() < 15 => {
                result.map_err(|_| "IPC_ACCESS_FAILED")?;
                let connected = listener;
                listener = create_pipe(&pipe_name, &sid, false).map_err(|_| "IPC_ACCESS_FAILED")?;
                let core = core.clone();
                let tasks = tasks.clone();
                connections.spawn(async move { connection(connected, core, tasks).await; });
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
            Some(run_id) = pending_tasks.recv() => {
                if !scheduled.insert(run_id.clone()) { continue; }
                let worker_core = core.clone();
                let permits = permits.clone();
                let workspace = path.join(&run_id);
                workers.spawn(async move {
                    let Ok(_permit) = permits.acquire_owned().await else { return; };
                    let spec = worker_core.lock().await.take_run(&run_id);
                    if let Some(spec) = spec {
                        let result = execute(&spec, worker_core.clone(), &workspace).await;
                        worker_core.lock().await.finish(&spec.run_id, result);
                    }
                });
            }
            Some(_) = workers.join_next(), if !workers.is_empty() => {}
        }
    }
    core.lock().await.shutdown();
    connections.abort_all();
    while connections.join_next().await.is_some() {}
    let drained = tokio::time::timeout(Duration::from_secs(5), async {
        while workers.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        workers.abort_all();
        while workers.join_next().await.is_some() {}
        return Err("RUNNER_DRAIN_FAILED");
    }
    Ok(())
}

async fn send(pipe: &mut NamedPipeServer, response: &Response) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(response)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(std::io::Error::other("Response budget"));
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        pipe.write_u32_le(bytes.len() as u32).await?;
        pipe.write_all(&bytes).await
    })
    .await
    .map_err(|_| std::io::Error::other("Response timeout"))?
}

async fn connection(mut pipe: NamedPipeServer, core: Core, tasks: mpsc::Sender<String>) {
    let connection_id = uuid::Uuid::new_v4().to_string();
    let mut request_ids = HashSet::new();
    let mut recent_ids = VecDeque::new();
    loop {
        let frame =
            tokio::time::timeout(
                Duration::from_secs(if recent_ids.is_empty() { 5 } else { 60 }),
                async {
                    let size = pipe.read_u32_le().await? as usize;
                    if size > MAX_FRAME_BYTES {
                        return Ok::<_, std::io::Error>(Err(ErrorCode::FrameTooLarge));
                    }
                    let mut bytes = vec![0u8; size];
                    pipe.read_exact(&mut bytes).await?;
                    Ok(serde_json::from_slice::<Request>(&bytes)
                        .map_err(|_| ErrorCode::InvalidRequest))
                },
            )
            .await;
        let request = match frame {
            Ok(Ok(Ok(request))) => request,
            Ok(Ok(Err(code))) => {
                let _ = send(
                    &mut pipe,
                    &Response {
                        version: 1,
                        request_id: String::new(),
                        outcome: Outcome::Error(RuntimeError { code }),
                    },
                )
                .await;
                break;
            }
            _ => break,
        };
        if !request_ids.insert(request.request_id.clone()) {
            let _ = send(
                &mut pipe,
                &Response {
                    version: 1,
                    request_id: request.request_id,
                    outcome: Outcome::Error(RuntimeError {
                        code: ErrorCode::InvalidRequest,
                    }),
                },
            )
            .await;
            break;
        }
        recent_ids.push_back(request.request_id.clone());
        if recent_ids.len() > 256 {
            request_ids.remove(&recent_ids.pop_front().unwrap());
        }
        let response = core.lock().await.handle(&connection_id, request);
        let run_id = match &response.outcome {
            Outcome::Receipt(receipt) => Some(receipt.run_id.clone()),
            _ => None,
        };
        if let Some(run_id) = run_id {
            if tasks.send(run_id).await.is_err() {
                break;
            }
        }
        if send(&mut pipe, &response).await.is_err() {
            break;
        }
    }
    core.lock().await.disconnect(&connection_id);
}

fn verify_artifact(value: &Value) -> Result<PathBuf, ErrorCode> {
    let path = Path::new(value["path"].as_str().ok_or(ErrorCode::ExecutionFailed)?);
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ErrorCode::ExecutionFailed)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ErrorCode::ExecutionFailed);
    }
    let canonical = std::fs::canonicalize(path).map_err(|_| ErrorCode::ExecutionFailed)?;
    let normalized = canonical
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_lowercase();
    if normalized != path.to_string_lossy().to_lowercase() {
        return Err(ErrorCode::ExecutionFailed);
    }
    let bytes = std::fs::read(path).map_err(|_| ErrorCode::ExecutionFailed)?;
    if format!("{:x}", Sha256::digest(bytes))
        != value["sha256"].as_str().ok_or(ErrorCode::ExecutionFailed)?
    {
        return Err(ErrorCode::ExecutionFailed);
    }
    Ok(path.into())
}

async fn execute(spec: &RunSpec, core: Core, workspace: &Path) -> Result<Value, ErrorCode> {
    let config: Value = serde_json::from_str(include_str!("../.generated/runner.json"))
        .map_err(|_| ErrorCode::ExecutionFailed)?;
    let bun = verify_artifact(&config["bun"])?;
    let runner = verify_artifact(&config["runner"])?;
    let mut command = tokio::process::Command::new(bun);
    std::fs::create_dir(workspace).map_err(|_| ErrorCode::ExecutionFailed)?;
    command
        .arg(runner)
        .current_dir(workspace)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    // Bun needs the Windows system directory, never the parent's credential/token env.
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
    let mut child = command.spawn().map_err(|_| ErrorCode::ExecutionFailed)?;
    let bytes = serde_json::to_vec(
        &json!({"pluginId":spec.plugin_id,"commandId":spec.command_id,"input":spec.input,"packageDigest":spec.package_digest}),
    )
    .map_err(|_| ErrorCode::InputInvalid)?;
    let mut stdin = child.stdin.take().ok_or(ErrorCode::ExecutionFailed)?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or(ErrorCode::ExecutionFailed)?
        .take((MAX_FRAME_BYTES + 1) as u64);
    let duration = Duration::from_millis((spec.deadline - now()).max(0.0) as u64);
    let result = tokio::select! {
        result = tokio::time::timeout(duration, async {
            stdin.write_all(&bytes).await.map_err(|_| ErrorCode::ExecutionFailed)?;
            stdin.shutdown().await.map_err(|_| ErrorCode::ExecutionFailed)?;
            drop(stdin);
            let mut output = Vec::new();
            stdout.read_to_end(&mut output).await.map_err(|_| ErrorCode::ExecutionFailed)?;
            if output.len() > MAX_FRAME_BYTES { return Err(ErrorCode::OutputInvalid); }
            let status = child.wait().await.map_err(|_| ErrorCode::ExecutionFailed)?;
            if !status.success() { return Err(ErrorCode::ExecutionFailed); }
            let envelope: Value = serde_json::from_slice(&output).map_err(|_| ErrorCode::OutputInvalid)?;
            if envelope["pluginId"] != spec.plugin_id { return Err(ErrorCode::OutputInvalid); }
            if envelope["success"] == true { Ok(envelope["data"].clone()) }
            else { Err(serde_json::from_value(envelope["error"]["code"].clone()).unwrap_or(ErrorCode::ExecutionFailed)) }
        }) => result.unwrap_or(Err(ErrorCode::Timeout)),
        _ = async {
            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
                if core.lock().await.state(&spec.run_id) == Some(JobState::Cancelling) { break; }
            }
        } => Err(ErrorCode::Aborted),
    };
    if result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    result
}
