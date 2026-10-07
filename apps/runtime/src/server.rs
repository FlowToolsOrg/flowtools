use crate::security::{create_pipe, current_user_sid};
use flowtools_runtime_core::{
    data::DataStore,
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
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
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
    if !matches!(args.len(), 3 | 4)
        || args[0] != "--validation-profile"
        || args[2] != "--validation"
        || (args.len() == 4 && args[3] != "--validation-data")
    {
        return Err("SETUP_REQUIRED");
    }
    let path = PathBuf::from(&args[1]);
    if !path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
        || !matches!(path.components().next(), Some(std::path::Component::Prefix(prefix)) if matches!(prefix.kind(),std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
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
        if !metadata.is_file()
            || metadata.file_attributes() & 0x400 != 0
            || metadata.len() != b"flowtools-disposable-v1".len() as u64
        {
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
    #[cfg(feature = "standalone")]
    verify_bundle().map_err(|_| "BUNDLE_INTEGRITY_FAILED")?;
    // Validate explicit mode and bootstrap before touching profile data.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sid = current_user_sid().map_err(|_| "IPC_ACCESS_FAILED")?;
    let validation = args.contains(&"--validation".into());
    let (path, pipe_name, credentials) = if validation {
        let cli = token("FLOWTOOLS_RUNTIME_VALIDATION_CLI_TOKEN")?;
        let desktop = token("FLOWTOOLS_RUNTIME_VALIDATION_DESKTOP_TOKEN")?;
        if cli == desktop {
            return Err("VALIDATION_TOKEN_REQUIRED");
        }
        let path = profile()?;
        let digest = Sha256::digest(format!("{sid}|{}", path.display()).as_bytes());
        (
            path,
            format!(r"\\.\pipe\flowtools-validation-{digest:x}"),
            Some((cli, desktop)),
        )
    } else {
        if args.len() != 3
            || !matches!(args[0].as_str(), "--initialize-profile" | "--profile")
            || !matches!(
                args[2].as_str(),
                "--management" | "--cold-start" | "--serve"
            )
            || (args[0] == "--initialize-profile" && args[2] != "--management")
        {
            return Err("SETUP_REQUIRED");
        }
        let path = PathBuf::from(&args[1]);
        if args[0] == "--initialize-profile" {
            crate::profile::initialize(&path, &sid).map_err(|_| "PROFILE_INITIALIZATION_FAILED")?;
        }
        let (path, _) = crate::profile::load(&path).map_err(|_| "SETUP_REQUIRED")?;
        let pipe_name = crate::profile::pipe(&path, &sid);
        (path, pipe_name, None)
    };
    let mut listener =
        create_pipe(&pipe_name, &sid, true).map_err(|_| "RUNTIME_ALREADY_RUNNING")?;
    let store = DataStore::open(&path.join("runtime.sqlite")).map_err(|code| match code {
        ErrorCode::RecoveryPending => "RECOVERY_PENDING",
        ErrorCode::StoreCorrupt => "STORE_CORRUPT",
        ErrorCode::StoreBusy => "STORE_BUSY",
        ErrorCode::SchemaUnsupported => "SCHEMA_UNSUPPORTED",
        _ => "STORAGE_FAILED",
    })?;
    let runtime = if let Some((cli_token, desktop_token)) = credentials {
        if args.contains(&"--validation-data".into()) {
            RuntimeCore::validation_data(cli_token, desktop_token, store)
        } else {
            RuntimeCore::validation(cli_token, desktop_token).with_data_store(store)
        }
    } else {
        let (_, bootstrap) = crate::profile::load(&path).map_err(|_| "SETUP_REQUIRED")?;
        RuntimeCore::managed(
            std::collections::HashMap::from([
                (bootstrap.cli_token, "local-cli".into()),
                (bootstrap.desktop_token, "local-desktop".into()),
                (bootstrap.management_token, "local-manager".into()),
            ]),
            store,
            args[2] == "--management",
            args[2] == "--cold-start",
        )
        .map_err(|code| match code {
            ErrorCode::ColdStartDenied => "COLD_START_DENIED",
            _ => "RUNTIME_DATA_UNAVAILABLE",
        })?
    };
    let core = Arc::new(Mutex::new(runtime));
    println!(
        "{}",
        json!({"type":"ready", "pipe":pipe_name, "protocolMajor":1})
    );
    let mut connections = tokio::task::JoinSet::new();
    let mut workers = tokio::task::JoinSet::new();
    let (tasks, mut pending_tasks) = mpsc::channel::<String>(128);
    for run in core.lock().await.pending_runs() {
        tasks.try_send(run).map_err(|_| "RUNNER_QUEUE_FAILED")?;
    }
    let mut scheduled = HashSet::new();
    let permits = Arc::new(Semaphore::new(4));
    let stdin_lifetime =
        std::env::var("FLOWTOOLS_RUNTIME_VALIDATION_STDIN_LIFETIME").as_deref() == Ok("1");
    let mut stdin = tokio::io::stdin();
    let mut byte = [0u8; 1];
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    let stop_reply = Arc::new(Semaphore::new(1));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = tick.tick() => {if stop_ready(&core, &stop_reply).await {break;}}
            _ = stdin.read(&mut byte), if stdin_lifetime => break,
            result = listener.connect(), if connections.len() < 15 => {
                let Some(connected) = accept_connection(&mut listener, result, &pipe_name, &sid).map_err(|_| "IPC_ACCESS_FAILED")? else { continue; };
                let core = core.clone();
                let tasks = tasks.clone();
                let stop_reply = stop_reply.clone();
                connections.spawn(async move { connection(connected, core, tasks, stop_reply).await; });
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

fn accept_connection(
    listener: &mut NamedPipeServer,
    result: std::io::Result<()>,
    name: &str,
    sid: &str,
) -> std::io::Result<Option<NamedPipeServer>> {
    let accepted = match result {
        Ok(()) => true,
        // A client may disappear before the server observes its connection.
        // That peer's failure must not terminate the shared Host.
        Err(error) if matches!(error.raw_os_error(), Some(109 | 232 | 233)) => false,
        Err(error) => return Err(error),
    };
    // Create before dropping the old handle, preserving exclusive pipe ownership.
    let previous = std::mem::replace(listener, create_pipe(name, sid, false)?);
    Ok(accepted.then_some(previous))
}

async fn stop_ready(core: &Core, stop_reply: &Semaphore) -> bool {
    core.lock().await.stopping() && stop_reply.available_permits() != 0
}

async fn send(pipe: &mut (impl AsyncWrite + Unpin), response: &Response) -> std::io::Result<()> {
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

async fn connection(
    mut pipe: impl AsyncRead + AsyncWrite + Unpin,
    core: Core,
    tasks: mpsc::Sender<String>,
    stop_reply: Arc<Semaphore>,
) {
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
        if request.request_id.len() > 128 || !request_ids.insert(request.request_id.clone()) {
            let _ = send(
                &mut pipe,
                &Response {
                    version: 1,
                    request_id: String::new(),
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
        let (response, stop_receipt) = {
            let mut core = core.lock().await;
            let response = core.handle(&connection_id, request);
            // Claim the receipt barrier under the same lock that marks stopping.
            // The shutdown tick cannot abort this connection between those steps.
            let receipt = matches!(response.outcome, Outcome::Stopping)
                .then(|| stop_reply.clone().try_acquire_owned().expect("Single stop"));
            (response, receipt)
        };
        let run_id = match &response.outcome {
            Outcome::Receipt(receipt) => Some(receipt.run_id.clone()),
            _ => None,
        };
        if let Some(run_id) = run_id {
            if tasks.send(run_id).await.is_err() {
                break;
            }
        }
        let sent = send(&mut pipe, &response).await;
        if stop_receipt.is_some() {
            // Windows may discard unread pipe data when the server handle closes.
            // Our clients close after reading the receipt. A disappeared or idle
            // manager cannot prevent shutdown indefinitely; keep the 5s IO bound.
            if sent.is_ok() {
                let _ = tokio::time::timeout(Duration::from_secs(5), pipe.read_u8()).await;
            }
            drop(stop_receipt);
            break;
        }
        if sent.is_err() {
            break;
        }
    }
    core.lock().await.disconnect(&connection_id);
}

fn verify_artifact(value: &Value) -> Result<PathBuf, ErrorCode> {
    #[cfg(feature = "standalone")]
    {
        let root = std::env::current_exe().map_err(|_| ErrorCode::ExecutionFailed)?;
        let root = root
            .parent()
            .and_then(Path::parent)
            .ok_or(ErrorCode::ExecutionFailed)?;
        let artifact =
            serde_json::from_value(value.clone()).map_err(|_| ErrorCode::ExecutionFailed)?;
        return crate::bundle::verify(root, &artifact).map_err(|_| ErrorCode::ExecutionFailed);
    }
    #[cfg(not(feature = "standalone"))]
    {
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
}

#[cfg(feature = "standalone")]
fn verify_bundle() -> Result<(), ErrorCode> {
    let config: Value =
        serde_json::from_str(runner_config()).map_err(|_| ErrorCode::ExecutionFailed)?;
    for artifact in config["files"]
        .as_array()
        .ok_or(ErrorCode::ExecutionFailed)?
    {
        verify_artifact(artifact)?;
    }
    Ok(())
}
fn runner_config() -> &'static str {
    #[cfg(feature = "standalone")]
    {
        include_str!(concat!(env!("FLOWTOOLS_BUNDLE_BUILD_DIR"), "/runner.json"))
    }
    #[cfg(not(feature = "standalone"))]
    {
        include_str!("../.generated/runner.json")
    }
}

async fn execute(spec: &RunSpec, core: Core, workspace: &Path) -> Result<Value, ErrorCode> {
    // Hashing build-owned binaries must not block the IPC scheduler.
    let (bun, runner) = tokio::task::spawn_blocking(|| {
        #[cfg(feature = "standalone")]
        verify_bundle()?;
        let config: Value =
            serde_json::from_str(runner_config()).map_err(|_| ErrorCode::ExecutionFailed)?;
        Ok::<_, ErrorCode>((
            verify_artifact(&config["bun"])?,
            verify_artifact(&config["runner"])?,
        ))
    })
    .await
    .map_err(|_| ErrorCode::ExecutionFailed)??;
    core.lock().await.check_run(&spec.run_id)?;
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
    let _job = match crate::process_job::ProcessJob::bind(&child) {
        Ok(job) => job,
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(ErrorCode::ExecutionFailed);
        }
    };
    let mut payload = json!({"pluginId":spec.plugin_id,"commandId":spec.command_id,"input":spec.input,"packageDigest":spec.package_digest});
    if spec.data.is_some() {
        payload["data"] = serde_json::to_value(&spec.data).map_err(|_| ErrorCode::InputInvalid)?;
    }
    let bytes = serde_json::to_vec(&payload).map_err(|_| ErrorCode::InputInvalid)?;
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
            if envelope.as_object().is_none_or(|v| v.len()!=2) {return Err(ErrorCode::OutputInvalid);}
            let mutations:Vec<flowtools_runtime_core::protocol::DataMutation>=serde_json::from_value(envelope["mutations"].clone()).map_err(|_|ErrorCode::OutputInvalid)?;
            let execution=&envelope["execution"];
            if execution["pluginId"] != spec.plugin_id { return Err(ErrorCode::OutputInvalid); }
            if execution["success"] == true {
                let data=execution["data"].clone();
                if !flowtools_runtime_core::catalog::BuiltinCatalog::embedded().output_valid(&spec.plugin_id,&spec.command_id,&data) {return Err(ErrorCode::OutputInvalid);}
                core.lock().await.commit_mutations(&spec.run_id,&mutations)?;
                Ok(data)
            }
            else { Err(serde_json::from_value(execution["error"]["code"].clone()).unwrap_or(ErrorCode::ExecutionFailed)) }
        }) => result.unwrap_or(Err(ErrorCode::Timeout)),
        code = async {
            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
                if let Err(code) = core.lock().await.check_run(&spec.run_id) { break code; }
            }
        } => Err(code),
    };
    if result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowtools_runtime_core::protocol::{Call, OpenSession, SubmitJob, CLIENT_VERSION};

    #[tokio::test]
    async fn aborted_pipe_peers_preserve_owner_and_next_real_handshake() {
        use tokio::net::windows::named_pipe::ClientOptions;
        let name = format!(r"\\.\pipe\flowtools-aborted-test-{}", uuid::Uuid::new_v4());
        let sid = current_user_sid().unwrap();
        let mut listener = create_pipe(&name, &sid, true).unwrap();
        drop(ClientOptions::new().open(&name).unwrap());
        let early = listener.connect().await;
        drop(accept_connection(&mut listener, early, &name, &sid).unwrap());
        for code in [109, 232, 233] {
            assert!(accept_connection(
                &mut listener,
                Err(std::io::Error::from_raw_os_error(code)),
                &name,
                &sid
            )
            .unwrap()
            .is_none());
            assert!(create_pipe(&name, &sid, true).is_err());
        }
        assert!(accept_connection(
            &mut listener,
            Err(std::io::Error::from_raw_os_error(5)),
            &name,
            &sid
        )
        .is_err());
        let mut peer = ClientOptions::new().open(&name).unwrap();
        listener.connect().await.unwrap();
        let connected = accept_connection(&mut listener, Ok(()), &name, &sid)
            .unwrap()
            .unwrap();
        let core = Arc::new(Mutex::new(RuntimeCore::validation(
            "cli".into(),
            "desktop".into(),
        )));
        let (tasks, _receiver) = mpsc::channel(128);
        let worker = tokio::spawn(connection(
            connected,
            core,
            tasks,
            Arc::new(Semaphore::new(1)),
        ));
        let request = serde_json::to_vec(&Request {
            version: 1,
            request_id: "after-aborted-peer".into(),
            session: None,
            call: Call::Open(OpenSession {
                token: "cli".into(),
                client_version: CLIENT_VERSION.into(),
                expected_instance_id: None,
            }),
        })
        .unwrap();
        peer.write_u32_le(request.len() as u32).await.unwrap();
        peer.write_all(&request).await.unwrap();
        let length = peer.read_u32_le().await.unwrap();
        let mut response = vec![0; length as usize];
        peer.read_exact(&mut response).await.unwrap();
        assert!(matches!(
            serde_json::from_slice::<Response>(&response)
                .unwrap()
                .outcome,
            Outcome::Session(_)
        ));
        drop(peer);
        tokio::time::timeout(Duration::from_secs(1), worker)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn stop_waits_for_the_complete_receipt_and_client_close() {
        for read_receipt in [true, false] {
            let profile = std::env::temp_dir().join(format!(
                "flowtools-validation-stop-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir(&profile).unwrap();
            let store = DataStore::open(&profile.join("runtime.sqlite")).unwrap();
            let core = Arc::new(Mutex::new(
                RuntimeCore::managed(
                    std::collections::HashMap::from([("manager".into(), "local-manager".into())]),
                    store,
                    false,
                    false,
                )
                .unwrap(),
            ));
            let stop_reply = Arc::new(Semaphore::new(1));
            // A one-byte transport forces the receipt write to wait for the reader.
            // No timing luck or retries are needed to reproduce the shutdown race.
            let (server, mut peer) = tokio::io::duplex(1);
            let (tasks, _receiver) = mpsc::channel(128);
            let worker = tokio::spawn(connection(server, core.clone(), tasks, stop_reply.clone()));
            let request = Request {
                version: 1,
                request_id: "open".into(),
                session: None,
                call: Call::Open(OpenSession {
                    token: "manager".into(),
                    client_version: CLIENT_VERSION.into(),
                    expected_instance_id: None,
                }),
            };
            async fn write(peer: &mut tokio::io::DuplexStream, request: &Request) {
                let bytes = serde_json::to_vec(request).unwrap();
                peer.write_u32_le(bytes.len() as u32).await.unwrap();
                peer.write_all(&bytes).await.unwrap();
            }
            async fn read(peer: &mut tokio::io::DuplexStream) -> Response {
                let size = peer.read_u32_le().await.unwrap();
                let mut bytes = vec![0; size as usize];
                peer.read_exact(&mut bytes).await.unwrap();
                serde_json::from_slice(&bytes).unwrap()
            }
            write(&mut peer, &request).await;
            let Outcome::Session(session) = read(&mut peer).await.outcome else {
                panic!("Authenticated manager");
            };
            write(
                &mut peer,
                &Request {
                    request_id: "stop".into(),
                    session: Some(session),
                    call: Call::Stop,
                    ..request
                },
            )
            .await;
            tokio::time::timeout(Duration::from_secs(1), async {
                while !core.lock().await.stopping() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(
                !stop_ready(&core, &stop_reply).await,
                "Receipt is still blocked"
            );
            if read_receipt {
                let response = read(&mut peer).await;
                assert_eq!(response.request_id, "stop");
                assert!(matches!(response.outcome, Outcome::Stopping));
                assert!(
                    !stop_ready(&core, &stop_reply).await,
                    "Client still owns its pipe"
                );
            }
            drop(peer);
            tokio::time::timeout(Duration::from_secs(1), worker)
                .await
                .unwrap()
                .unwrap();
            assert!(stop_ready(&core, &stop_reply).await);
        }
    }

    async fn accepted_run(core: &Core) -> RunSpec {
        let mut locked = core.lock().await;
        let Outcome::Session(session) = locked
            .handle(
                "fixture",
                Request {
                    version: 1,
                    request_id: uuid::Uuid::new_v4().to_string(),
                    session: None,
                    call: Call::Open(OpenSession {
                        token: "cli".into(),
                        client_version: CLIENT_VERSION.into(),
                        expected_instance_id: None,
                    }),
                },
            )
            .outcome
        else {
            panic!("Session");
        };
        let Outcome::Receipt(receipt) = locked
            .handle(
                "fixture",
                Request {
                    version: 1,
                    request_id: uuid::Uuid::new_v4().to_string(),
                    session: Some(session),
                    call: Call::Submit(SubmitJob {
                        plugin_id: "plugin-base64-encoder".into(),
                        command_id: "run".into(),
                        input: json!({"text":"private-canary"}),
                        idempotency_key: "fixture".into(),
                        background: false,
                        deadline: now() + 10000.0,
                    }),
                },
            )
            .outcome
        else {
            panic!("Receipt");
        };
        locked.take_run(&receipt.run_id).unwrap()
    }
    #[tokio::test]
    async fn real_managed_child_refuses_changed_package_and_reaps_on_deadline() {
        let profile = std::env::temp_dir().join(format!(
            "flowtools-validation-runner-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&profile).unwrap();
        let (manifest, _) = flowtools_runtime_core::catalog::BuiltinCatalog::embedded()
            .command("plugin-base64-encoder", "run")
            .map(|(a, b)| (a.clone(), b.clone()))
            .unwrap();
        let digest = flowtools_runtime_core::catalog::digest(&manifest);
        let core = Arc::new(Mutex::new(RuntimeCore::validation(
            "cli".into(),
            "desktop".into(),
        )));
        let mut invalid = accepted_run(&core).await;
        invalid.package_digest = "changed".into();
        assert_eq!(
            execute(&invalid, core.clone(), &profile.join(&invalid.run_id))
                .await
                .unwrap_err(),
            ErrorCode::ExecutionFailed
        );
        let expired = RunSpec {
            package_digest: digest,
            deadline: now() + 1.0,
            ..invalid
        };
        assert_eq!(
            execute(&expired, core, &profile.join("expired"))
                .await
                .unwrap_err(),
            ErrorCode::Timeout
        );
    }
}
