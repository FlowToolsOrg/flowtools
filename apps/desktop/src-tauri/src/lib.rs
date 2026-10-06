mod app_state;
mod commands;
mod db;
mod dto;
mod error;
#[cfg(windows)]
mod managed_runtime;
mod models;
mod repositories;
#[cfg(all(windows, any(debug_assertions, feature = "codegen")))]
mod validation_runtime;

use app_state::AppState;
use commands::{
    add_plugin, disable_plugin, enable_plugin, get_plugin, get_plugins, remove_plugin,
    update_plugin,
};
use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};
use tokio::sync::Mutex;

pub(crate) const RUNTIME_VALIDATION_IDENTIFIER: &str = "com.flowtools.g2-validation-20261004";
#[cfg(all(windows, debug_assertions))]
const RUNTIME_VALIDATION_PREFLIGHT: &str = "flowtools-runtime-validation-preflight-v1";

fn validation_startup_allowed(debug: bool, identifier: &str, requested: bool) -> bool {
    !requested || (debug && cfg!(windows) && identifier == RUNTIME_VALIDATION_IDENTIFIER)
}

fn command_builder<R: tauri::Runtime>() -> Builder<R> {
    #[cfg(not(windows))]
    let builder = Builder::<R>::new().commands(collect_commands![
        get_plugins,
        get_plugin,
        add_plugin,
        update_plugin,
        enable_plugin,
        disable_plugin,
        remove_plugin
    ]);
    #[cfg(all(windows, not(any(debug_assertions, feature = "codegen"))))]
    let builder = Builder::<R>::new().commands(collect_commands![
        get_plugins,
        get_plugin,
        add_plugin,
        update_plugin,
        enable_plugin,
        disable_plugin,
        remove_plugin,
        managed_runtime::managed_runtime::<tauri::Wry>,
        managed_runtime::managed_runtime_disconnect::<tauri::Wry>,
        managed_runtime::managed_runtime_control::<tauri::Wry>
    ]);
    #[cfg(all(windows, any(debug_assertions, feature = "codegen")))]
    let builder = Builder::<R>::new().commands(collect_commands![
        get_plugins,
        get_plugin,
        add_plugin,
        update_plugin,
        enable_plugin,
        disable_plugin,
        remove_plugin,
        managed_runtime::managed_runtime::<tauri::Wry>,
        managed_runtime::managed_runtime_disconnect::<tauri::Wry>,
        managed_runtime::managed_runtime_control::<tauri::Wry>,
        validation_runtime::validation_runtime::<tauri::Wry>,
        validation_runtime::validation_runtime_disconnect::<tauri::Wry>
    ]);
    builder
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
    let context = tauri::generate_context!();
    // Metadata-only preflight: no Builder, plugins, bindings writes or DB access.
    if std::env::args().nth(1).as_deref() == Some("--runtime-validation-identity") {
        #[cfg(all(windows, debug_assertions))]
        {
            println!("{RUNTIME_VALIDATION_PREFLIGHT}");
            println!("{}", context.config().identifier);
            println!(
                "{}",
                context
                    .config()
                    .build
                    .dev_url
                    .as_ref()
                    .map_or("", |url| url.as_str())
            );
            println!(
                "{}",
                context
                    .config()
                    .app
                    .windows
                    .first()
                    .map_or("", |window| window.title.as_str())
            );
        }
        return;
    }
    if !validation_startup_allowed(
        cfg!(debug_assertions),
        &context.config().identifier,
        std::env::var("FLOWTOOLS_RUNTIME_VALIDATION").as_deref() == Ok("1"),
    ) {
        eprintln!("VALIDATION_IDENTITY_REQUIRED");
        std::process::exit(1);
    }
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

    #[cfg(windows)]
    let tauri_builder = tauri_builder
        .manage(managed_runtime::ManagedConnections::default())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                managed_runtime::destroyed(window);
            }
        });

    #[cfg(all(windows, debug_assertions))]
    let tauri_builder = tauri_builder
        .manage(validation_runtime::ValidationRuntimeConnection::default())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                validation_runtime::on_destroyed(window);
            }
        });

    #[cfg(debug_assertions)]
    let tauri_builder = tauri_builder.plugin(tauri_plugin_devtools::init());

    tauri_builder
        .run(context)
        .expect("error while running tauri application");
}

#[cfg(test)]
mod startup_tests {
    use super::*;

    #[test]
    fn explicit_validation_refuses_default_identity_before_runtime_initialization() {
        for identifier in ["com.hmsuiji.desktop", RUNTIME_VALIDATION_IDENTIFIER] {
            for debug in [false, true] {
                assert!(validation_startup_allowed(debug, identifier, false));
                assert_eq!(
                    validation_startup_allowed(debug, identifier, true),
                    cfg!(windows) && debug && identifier == RUNTIME_VALIDATION_IDENTIFIER
                );
            }
        }
    }
}
