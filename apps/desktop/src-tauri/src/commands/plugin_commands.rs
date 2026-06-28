use tauri::State;
use crate::{
    app_state::AppState,
    error::AppError,
    repositories::plugin_repository,
};
use crate::dto::PluginDto;

#[tauri::command]
#[specta::specta]
pub async fn get_plugins(
    state: State<'_, AppState>,
) -> Result<Vec<PluginDto>, AppError> {
    let mut db = state.db.lock().await;
    let plugins = plugin_repository::get_plugins(&mut db).await?;
    let plugins = plugins.into_iter().map(PluginDto::from).collect();
    Ok(plugins)
}