//! Detached supervision retains IO ownership through cancellation and actual drain.
use flowtools_runtime_core::{
    protocol::ErrorCode,
    resources::native::{ResourceIo, ResourceIoResult},
    runtime::RuntimeCore,
};
use serde_json::Value;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
pub fn execute(
    core: Arc<Mutex<RuntimeCore>>,
    work: ResourceIo,
    selection: bool,
) -> tokio::task::JoinHandle<Result<Value, ErrorCode>> {
    tokio::spawn(async move {
        let id = work.id.clone();
        let cancellation = work.cancellation.clone();
        let started = Instant::now();
        let worker_core = core.clone();
        let worker_id = id.clone();
        let mut io = tokio::task::spawn_blocking(move || {
            work.perform_authorized(|| worker_core.blocking_lock().check_resource_io(&worker_id))
        });
        let result = loop {
            tokio::select! {
                result=&mut io=>break result.unwrap_or_else(|_|ResourceIoResult::interrupted()),
                _=tokio::time::sleep(Duration::from_millis(10))=>{
                    let cancel=started.elapsed()>=Duration::from_secs(5)||core.lock().await.check_resource_io(&id).is_err()||cancellation.check().is_err();
                    if cancel&&cancellation.cancel().is_err() {core.lock().await.fail_closed();}
                }
            }
        };
        let mut core = core.lock().await;
        if selection {
            core.finish_resource_selection(&id, result)
                .and_then(|v| serde_json::to_value(v).map_err(|_| ErrorCode::OutputInvalid))
        } else {
            core.finish_resource_call(&id, result)
                .and_then(|v| serde_json::to_value(v).map_err(|_| ErrorCode::OutputInvalid))
        }
    })
}
