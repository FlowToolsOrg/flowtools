//! Bidirectional bounded child pipes; identities never arrive in child payloads.
use super::*;
use flowtools_runtime_core::{broker::CapabilityOperation, services::ServiceTarget};
use serde::Deserialize;
use std::{collections::HashMap, future::Future, pin::Pin};

pub(super) struct RunnerBackend {
    pub config: Value,
    gates: Mutex<HashMap<String, Arc<Semaphore>>>,
    #[cfg(test)]
    pub handles: std::collections::BTreeMap<String, PathBuf>,
    #[cfg(test)]
    pub reads: std::sync::atomic::AtomicU32,
    #[cfg(test)]
    pub largest_group: std::sync::atomic::AtomicU32,
}
impl RunnerBackend {
    pub fn fixed() -> Result<Self, ErrorCode> {
        Ok(Self::new(
            serde_json::from_str(runner_config()).map_err(|_| ErrorCode::ExecutionFailed)?,
        ))
    }
    pub fn new(config: Value) -> Self {
        Self {
            config,
            gates: Mutex::new(HashMap::new()),
            #[cfg(test)]
            handles: Default::default(),
            #[cfg(test)]
            reads: Default::default(),
            #[cfg(test)]
            largest_group: Default::default(),
        }
    }
    async fn gate(&self, target: &ServiceTarget) -> Result<Arc<Semaphore>, ErrorCode> {
        let mut gates = self.gates.lock().await;
        if gates.len() >= 512 {
            return Err(ErrorCode::BudgetExceeded);
        }
        Ok(gates
            .entry(format!("{}/{}", target.publisher, target.id))
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone())
    }
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum RunnerMessage {
    Service {
        id: u32,
        target: ServiceTarget,
        input: Value,
    },
    Capability {
        id: u32,
        operation: CapabilityOperation,
    },
    Result {
        execution: Value,
        mutations: Vec<DataMutation>,
    },
}
async fn frame_read(stdout: &mut (impl AsyncRead + Unpin)) -> Result<RunnerMessage, ErrorCode> {
    let length = stdout
        .read_u32_le()
        .await
        .map_err(|_| ErrorCode::OutputInvalid)? as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(ErrorCode::OutputInvalid);
    }
    let mut bytes = vec![0; length];
    stdout
        .read_exact(&mut bytes)
        .await
        .map_err(|_| ErrorCode::OutputInvalid)?;
    serde_json::from_slice(&bytes).map_err(|_| ErrorCode::OutputInvalid)
}
async fn frame_write(
    stdin: &mut (impl AsyncWrite + Unpin),
    value: &Value,
) -> Result<(), ErrorCode> {
    let bytes = serde_json::to_vec(value).map_err(|_| ErrorCode::OutputInvalid)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ErrorCode::OutputInvalid);
    }
    stdin
        .write_u32_le(bytes.len() as u32)
        .await
        .map_err(|_| ErrorCode::ExecutionFailed)?;
    stdin
        .write_all(&bytes)
        .await
        .map_err(|_| ErrorCode::ExecutionFailed)
}
fn reply(id: u32, result: Result<Value, ErrorCode>) -> Value {
    match result {
        Ok(data) => json!({"kind":"reply","id":id,"success":true,"data":data}),
        Err(code) => json!({"kind":"reply","id":id,"success":false,"errorCode":code}),
    }
}
async fn monitor(core: &Core, id: &str) -> ErrorCode {
    loop {
        tokio::time::sleep(Duration::from_millis(10)).await;
        if let Err(code) = core.lock().await.check_run(id) {
            return code;
        }
    }
}

pub(super) fn execute(
    spec: RunSpec,
    target: Option<ServiceTarget>,
    core: Core,
    workspace: PathBuf,
    backend: Arc<RunnerBackend>,
) -> Pin<Box<dyn Future<Output = Result<Value, ErrorCode>> + Send>> {
    Box::pin(async move {
        core.lock().await.validate_runner_spec(&spec)?;
        let config = backend.config.clone();
        let (bun, runner) = tokio::task::spawn_blocking(move || {
            #[cfg(feature = "standalone")]
            verify_bundle()?;
            Ok::<_, ErrorCode>((
                verify_artifact(&config["bun"])?,
                verify_artifact(&config["runner"])?,
            ))
        })
        .await
        .map_err(|_| ErrorCode::ExecutionFailed)??;
        core.lock().await.check_run(&spec.run_id)?;
        std::fs::create_dir(&workspace).map_err(|_| ErrorCode::ExecutionFailed)?;
        let mut command = tokio::process::Command::new(bun);
        command
            .arg(runner)
            .current_dir(&workspace)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
        let mut child = command.spawn().map_err(|_| ErrorCode::ExecutionFailed)?;
        let job = match crate::process_job::ProcessJob::bind(&child) {
            Ok(job) => job,
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(ErrorCode::ExecutionFailed);
            }
        };
        let mut stdin = child.stdin.take().ok_or(ErrorCode::ExecutionFailed)?;
        let mut stdout = child.stdout.take().ok_or(ErrorCode::ExecutionFailed)?;
        let mut payload = json!({"pluginId":spec.plugin_id,"commandId":spec.command_id,"input":spec.input,"packageDigest":spec.package_digest,"data":spec.data,"deadline":spec.deadline});
        if let Some(ref target) = target {
            payload["serviceTarget"] =
                serde_json::to_value(target).map_err(|_| ErrorCode::InputInvalid)?;
        }
        let duration = Duration::from_millis((spec.deadline - now()).max(0.0) as u64);
        let run_id = spec.run_id.clone();
        let result = tokio::select! {
            result=tokio::time::timeout(duration,async {
                frame_write(&mut stdin,&json!({"kind":"start","payload":payload})).await?;
                let mut last_id=0; let mut frames=0;
                loop {
                    frames+=1; if frames>256 { return Err(ErrorCode::BudgetExceeded); }
                    core.lock().await.check_run(&run_id)?;
                    match frame_read(&mut stdout).await? {
                        RunnerMessage::Service{id,target,input}=> {
                            if id<=last_id { return Err(ErrorCode::InvalidRequest); } last_id=id;
                            let next=core.lock().await.prepare_service(&run_id,target,input);
                            let response=match next {
                                Err(code)=>Err(code),Ok(next)=> {
                                    let child_core=core.clone(); let child_backend=backend.clone();
                                    let parent=workspace.parent().ok_or(ErrorCode::ExecutionFailed)?.to_owned();
                                    // Detached only from the parent future, never from Host policy/lifetime.
                                    // It retains its lease until the real child has been killed and reaped.
                                    tokio::spawn(async move {
                                        let id=next.spec.run_id.clone();
                                        let mut reservation=None;
                                        let result=async {
                                            let gate=child_backend.gate(&next.target).await?;
                                            let permit=tokio::select! {
                                                permit=gate.acquire_owned()=>permit.map_err(|_|ErrorCode::RuntimeBusy)?,
                                                code=monitor(&child_core,&id)=>return Err(code),
                                            };
                                            child_core.lock().await.start_service(&id)?;
                                            reservation=Some(permit);
                                            execute(next.spec,Some(next.target),child_core.clone(),parent.join(&id),child_backend).await
                                        }.await;
                                        let finished=child_core.lock().await.finish_service(&id,result);
                                        drop(reservation);
                                        finished
                                    }).await.map_err(|_|ErrorCode::ExecutionFailed)?
                                }
                            };
                            frame_write(&mut stdin,&reply(id,response)).await?;
                        }
                        RunnerMessage::Capability{id,operation}=> {
                            if id<=last_id { return Err(ErrorCode::InvalidRequest); } last_id=id;
                            let response=core.lock().await.invoke_service(&run_id,&operation,|_| {
                                #[cfg(test)] if let CapabilityOperation::FileRead{handle}=&operation {
                                    let path=backend.handles.get(handle).ok_or(ErrorCode::ScopeDenied)?;
                                    backend.reads.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
                                    let bytes=std::fs::read(path).map_err(|_|ErrorCode::ExecutionFailed)?;
                                    if bytes.len()>65_536 { return Err(ErrorCode::BudgetExceeded); }
                                    return String::from_utf8(bytes).map(Value::String).map_err(|_|ErrorCode::OutputInvalid);
                                }
                                Err(ErrorCode::CapabilityUndeclared)
                            });
                            frame_write(&mut stdin,&reply(id,response)).await?;
                        }
                        RunnerMessage::Result{execution,mutations}=> {
                            if execution["pluginId"]!=spec.plugin_id || (target.is_some() && !mutations.is_empty()) { return Err(ErrorCode::OutputInvalid); }
                            stdin.shutdown().await.map_err(|_|ErrorCode::ExecutionFailed)?;
                            drop(stdin);
                            if !child.wait().await.map_err(|_|ErrorCode::ExecutionFailed)?.success() { return Err(ErrorCode::ExecutionFailed); }
                            core.lock().await.check_run(&run_id)?;
                            if execution["success"]!=true { return Err(serde_json::from_value(execution["error"]["code"].clone()).unwrap_or(ErrorCode::ExecutionFailed)); }
                            let data=execution["data"].clone();
                            if !core.lock().await.runner_output_valid(&run_id,&data) { return Err(ErrorCode::OutputInvalid); }
                            return Ok((data,mutations));
                        }
                    }
                }
            })=>result.unwrap_or(Err(ErrorCode::Timeout)),
            code=monitor(&core,&run_id)=>Err(code),
        };
        #[cfg(test)]
        if let Ok(count) = job.active_processes() {
            backend
                .largest_group
                .fetch_max(count, std::sync::atomic::Ordering::SeqCst);
        }
        if result.is_err() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        let reaped = tokio::task::spawn_blocking(move || job.drain()).await;
        if !matches!(reaped, Ok(Ok(()))) {
            core.lock().await.fail_closed();
            return Err(ErrorCode::ExecutionInterrupted);
        }
        let (data, mutations) = result?;
        let mut trailing = [0u8; 1];
        if !matches!(
            tokio::time::timeout(Duration::from_secs(1), stdout.read(&mut trailing)).await,
            Ok(Ok(0))
        ) {
            return Err(ErrorCode::OutputInvalid);
        }
        let mut core = core.lock().await;
        core.check_run(&run_id)?;
        if target.is_none() {
            core.commit_mutations(&run_id, &mutations)?;
        }
        Ok(data)
    })
}
