use flowtools_runtime_core::protocol::{
    Call, ErrorCode, Request, Response, RuntimeError, MAX_FRAME_BYTES,
};
use tauri::Manager;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient},
    sync::Mutex,
};

#[derive(Default)]
pub struct ValidationRuntimeConnection(pub std::sync::Arc<Mutex<Option<NamedPipeClient>>>);

fn denied() -> RuntimeError {
    RuntimeError {
        code: ErrorCode::SessionInvalid,
    }
}

fn allowed(debug: bool, identifier: &str, label: &str, host: &str, opt_in: Option<&str>) -> bool {
    debug
        && identifier == "com.flowtools.g2-validation-20261004"
        && label == "main"
        && matches!(host, "localhost" | "127.0.0.1" | "tauri.localhost")
        && opt_in == Some("1")
}

fn check_window<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> Result<(), RuntimeError> {
    let url = window.url().map_err(|_| denied())?;
    if !matches!(url.scheme(), "http" | "tauri")
        || !allowed(
            cfg!(debug_assertions),
            &window.app_handle().config().identifier,
            window.label(),
            url.host_str().unwrap_or_default(),
            std::env::var("FLOWTOOLS_RUNTIME_VALIDATION")
                .ok()
                .as_deref(),
        )
    {
        return Err(denied());
    }
    Ok(())
}

async fn exchange(
    state: &ValidationRuntimeConnection,
    mut request: Request,
) -> Result<Response, RuntimeError> {
    let mut guard = state.0.lock().await;
    if guard.is_none() {
        let path = std::env::var("FLOWTOOLS_RUNTIME_VALIDATION_PIPE").map_err(|_| denied())?;
        let suffix = path
            .strip_prefix(r"\\.\pipe\flowtools-validation-")
            .ok_or_else(denied)?;
        if suffix.len() != 64 || !suffix.bytes().all(|v| v.is_ascii_hexdigit()) {
            return Err(denied());
        }
        *guard = Some(ClientOptions::new().open(path).map_err(|_| RuntimeError {
            code: ErrorCode::RuntimeDisconnected,
        })?);
    }
    // JS cannot choose caller identity or read the native-only desktop bootstrap token.
    if let Call::Open(open) = &mut request.call {
        open.token =
            std::env::var("FLOWTOOLS_RUNTIME_VALIDATION_DESKTOP_TOKEN").map_err(|_| denied())?;
    }
    let bytes = serde_json::to_vec(&request).map_err(|_| RuntimeError {
        code: ErrorCode::InvalidRequest,
    })?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(RuntimeError {
            code: ErrorCode::FrameTooLarge,
        });
    }
    let pipe = guard.as_mut().unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        pipe.write_u32_le(bytes.len() as u32).await?;
        pipe.write_all(&bytes).await?;
        let size = pipe.read_u32_le().await? as usize;
        if size > MAX_FRAME_BYTES {
            return Err(std::io::Error::other("Response budget"));
        }
        let mut output = vec![0u8; size];
        pipe.read_exact(&mut output).await?;
        serde_json::from_slice::<Response>(&output).map_err(std::io::Error::other)
    })
    .await;
    match result {
        Ok(Ok(response)) => Ok(response),
        _ => {
            *guard = None;
            Err(RuntimeError {
                code: ErrorCode::RuntimeDisconnected,
            })
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn validation_runtime<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    state: tauri::State<'_, ValidationRuntimeConnection>,
    request: Request,
) -> Result<Response, RuntimeError> {
    check_window(&window)?;
    exchange(&state, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn validation_runtime_disconnect<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    state: tauri::State<'_, ValidationRuntimeConnection>,
) -> Result<(), RuntimeError> {
    check_window(&window)?;
    state.0.lock().await.take();
    Ok(())
}

pub fn on_destroyed<R: tauri::Runtime>(window: &tauri::Window<R>) {
    if let Some(state) = window.try_state::<ValidationRuntimeConnection>() {
        let connection = state.0.clone();
        tauri::async_runtime::spawn(async move {
            connection.lock().await.take();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_bridge_requires_native_identity_origin_mode_and_opt_in() {
        for debug in [false, true] {
            for identifier in ["com.flowtools.g2-validation-20261004", "com.tauri.dev"] {
                for label in ["main", "plugin-spoof"] {
                    for host in ["localhost", "evil.invalid"] {
                        for opt_in in [None, Some("1"), Some("true")] {
                            assert_eq!(
                                allowed(debug, identifier, label, host, opt_in),
                                debug
                                    && identifier == "com.flowtools.g2-validation-20261004"
                                    && label == "main"
                                    && host == "localhost"
                                    && opt_in == Some("1")
                            );
                        }
                    }
                }
            }
        }
    }
}
