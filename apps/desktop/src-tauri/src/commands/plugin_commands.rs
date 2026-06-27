use tauri::State;
use crate::{
    app_state::AppState,
    error::AppError,
    models::Plugin,
    repositories::plugin_repository,
};

#[tauri::command]
#[specta::specta]
pub async fn get_plugins(
    state: State<'_, AppState>,
) -> Result<Vec<Plugin>, AppError> {
    let mut db = state.db.lock().await;
    let plugins = plugin_repository::get_plugins(&mut db).await?;
    Ok(plugins)
}