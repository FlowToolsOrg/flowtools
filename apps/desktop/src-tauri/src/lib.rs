mod app_state;
mod commands;
mod db;
mod dto;
mod error;
mod models;
mod repositories;

use app_state::AppState;
use commands::{
    add_plugin, disable_plugin, enable_plugin, get_plugin, get_plugins, remove_plugin,
    update_plugin,
};
use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};
use tokio::sync::Mutex;

fn command_builder<R: tauri::Runtime>() -> Builder<R> {
    Builder::<R>::new().commands(collect_commands![
        get_plugins,
        get_plugin,
        add_plugin,
        update_plugin,
        enable_plugin,
        disable_plugin,
        remove_plugin
    ])
}

#[cfg(feature = "codegen")]
pub fn export_bindings(
    path: impl AsRef<std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    command_builder::<tauri::test::MockRuntime>()
        .export(Typescript::default(), path)
        .map_err(Into::into)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let commands_builder = command_builder::<tauri::Wry>();

    #[cfg(debug_assertions)]
    commands_builder
        .export(
            Typescript::default(),
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/utils/bindings.ts"),
        )
        .expect("Failed to export typescript bindings");

    #[allow(unused_mut)]
    let mut log_plugin_builder =
        tauri_plugin_log::Builder::new().level(tauri_plugin_log::log::LevelFilter::Info);

    #[cfg(debug_assertions)]
    {
        log_plugin_builder = log_plugin_builder.skip_logger();
    }

    let tauri_builder = tauri::Builder::default()
        .invoke_handler(commands_builder.invoke_handler())
        .setup(move |app| {
            commands_builder.mount_events(app);

            let handle = app.handle().clone();

            let db = tauri::async_runtime::block_on(async { db::init_db(&handle).await })
                .expect("failed to initialize database");

            app.manage(AppState { db: Mutex::new(db) });

            Ok(())
        })
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(log_plugin_builder.build());

    #[cfg(debug_assertions)]
    let tauri_builder = tauri_builder.plugin(tauri_plugin_devtools::init());

    tauri_builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
