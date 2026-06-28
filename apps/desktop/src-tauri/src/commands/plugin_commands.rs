use tauri::State;

use crate::{
    app_state::AppState,
    dto::{CreatePluginDto, PluginDto, PluginWriteDto, UpdatePluginDto},
    error::AppError,
    repositories::plugin_repository,
};

#[tauri::command]
#[specta::specta]
pub async fn get_plugins(state: State<'_, AppState>) -> Result<Vec<PluginDto>, AppError> {
    let mut db = state.db.lock().await;
    let plugins = plugin_repository::get_plugins(&mut db).await?;
    let plugins = plugins.into_iter().map(PluginDto::from).collect();
    Ok(plugins)
}

#[tauri::command]
#[specta::specta]
pub async fn get_plugin(state: State<'_, AppState>, id: String) -> Result<PluginDto, AppError> {
    PluginWriteDto::validate_id(&id)?;

    let mut db = state.db.lock().await;
    let plugin = plugin_repository::get_plugin(&mut db, &id).await?;
    Ok(plugin.into())
}

#[tauri::command]
#[specta::specta]
pub async fn add_plugin(
    state: State<'_, AppState>,
    plugin: CreatePluginDto,
) -> Result<PluginDto, AppError> {
    let plugin = plugin.normalize()?;

    let mut db = state.db.lock().await;
    let plugin = plugin_repository::add_plugin(&mut db, plugin).await?;
    Ok(plugin.into())
}

#[tauri::command]
#[specta::specta]
pub async fn update_plugin(
    state: State<'_, AppState>,
    id: String,
    plugin: UpdatePluginDto,
) -> Result<PluginDto, AppError> {
    PluginWriteDto::validate_id(&id)?;

    let mut db = state.db.lock().await;
    let existing = plugin_repository::get_plugin(&mut db, &id).await?;
    let update = plugin.merge(&existing)?;
    let plugin = plugin_repository::update_plugin(&mut db, existing, update).await?;
    Ok(plugin.into())
}

#[tauri::command]
#[specta::specta]
pub async fn enable_plugin(state: State<'_, AppState>, id: String) -> Result<PluginDto, AppError> {
    PluginWriteDto::validate_id(&id)?;

    let mut db = state.db.lock().await;
    let plugin = plugin_repository::enable_plugin(&mut db, id).await?;
    Ok(plugin.into())
}

#[tauri::command]
#[specta::specta]
pub async fn disable_plugin(state: State<'_, AppState>, id: String) -> Result<PluginDto, AppError> {
    PluginWriteDto::validate_id(&id)?;

    let mut db = state.db.lock().await;
    let plugin = plugin_repository::disable_plugin(&mut db, id).await?;
    Ok(plugin.into())
}

#[tauri::command]
#[specta::specta]
pub async fn remove_plugin(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    PluginWriteDto::validate_id(&id)?;

    let mut db = state.db.lock().await;
    plugin_repository::remove_plugin(&mut db, id).await?;
    Ok(())
}
