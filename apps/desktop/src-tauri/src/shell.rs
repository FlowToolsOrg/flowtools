mod policy;

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, Window, WindowEvent, Wry,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const TRAY_ID: &str = "flowtools-desktop";
const SHORTCUT: &str = "Ctrl+Shift+Space";

#[derive(Default)]
pub(crate) struct ShellState {
    tray_ready: AtomicBool,
}

// This plugin MUST precede database setup and every other plugin. Arguments and
// cwd from another instance are untrusted and deliberately never dispatched.
pub(crate) fn single_instance() -> tauri::plugin::TauriPlugin<Wry> {
    tauri_plugin_single_instance::init(|app, _args, _cwd| restore(app))
}

pub(crate) fn restore(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.unminimize();
    if let (Ok(position), Ok(size), Ok(monitors)) = (
        window.outer_position(),
        window.outer_size(),
        window.available_monitors(),
    ) {
        let mut areas: Vec<_> = monitors
            .iter()
            .map(|monitor| {
                let area = monitor.work_area();
                policy::Rect {
                    x: area.position.x,
                    y: area.position.y,
                    width: area.size.width,
                    height: area.size.height,
                }
            })
            .collect();
        if let Ok(Some(primary)) = window.primary_monitor() {
            let position = primary.work_area().position;
            if let Some(index) = areas
                .iter()
                .position(|a| a.x == position.x && a.y == position.y)
            {
                areas.swap(0, index);
            }
        }
        if let Some((x, y)) = policy::restore_position(
            policy::Rect {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            },
            &areas,
        ) {
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
    }
    let _ = window.show();
    let _ = window.set_focus();
}

pub(crate) fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        let ready = window
            .try_state::<ShellState>()
            .is_some_and(|state| state.tray_ready.load(Ordering::Acquire));
        // A failed hide or tray setup must retain the ordinary close/exit path.
        if policy::hide_on_close(window.label(), ready)
            && window.app_handle().tray_by_id(TRAY_ID).is_some()
            && window.hide().is_ok()
        {
            api.prevent_close();
        }
    }
}

fn autostart_text(enabled: bool) -> &'static str {
    if enabled {
        "开机启动：已启用"
    } else {
        "开机启动：关闭"
    }
}

pub(crate) fn setup(app: &mut tauri::App) {
    // Global shortcut/OS errors are recoverable. They never prevent a visible
    // main window or tray menu from opening. No JS permissions are added.
    let shortcut_ready = app
        .handle()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .and_then(|()| {
            app.global_shortcut()
                .on_shortcut(SHORTCUT, |app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        restore(app);
                    }
                })
                .map_err(|error| tauri::Error::Anyhow(error.into()))
        })
        .is_ok();

    let autostart_ready =
        policy::autostart_allowed(cfg!(debug_assertions), &app.config().identifier)
            && app
                .handle()
                .plugin(
                    tauri_plugin_autostart::Builder::new()
                        .app_name("FlowTools")
                        .arg("--background")
                        .build(),
                )
                .is_ok();
    match build_tray(app.handle(), shortcut_ready, autostart_ready) {
        Ok(()) => {
            app.state::<ShellState>()
                .tray_ready
                .store(true, Ordering::Release);
            if policy::start_hidden(autostart_ready, true, std::env::args()) {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
        }
        Err(_) => {
            tauri_plugin_log::log::warn!("DESKTOP_TRAY_UNAVAILABLE");
            restore(app.handle());
        }
    }
}

fn build_tray(app: &AppHandle, shortcut_ready: bool, autostart_ready: bool) -> tauri::Result<()> {
    let status = MenuItem::with_id(
        app,
        "shell-status",
        "桌面已驻留 · 关闭窗口后可从托盘唤起",
        false,
        None::<&str>,
    )?;
    let show = MenuItem::with_id(app, "shell-show", "打开 FlowTools", true, None::<&str>)?;
    let shortcut = MenuItem::with_id(
        app,
        "shell-shortcut",
        if shortcut_ready {
            "唤起快捷键：Ctrl+Shift+Space"
        } else {
            "快捷键不可用 · 请从托盘打开"
        },
        false,
        None::<&str>,
    )?;
    let current = if autostart_ready {
        app.autolaunch().is_enabled().ok()
    } else {
        None
    };
    let autostart = MenuItem::with_id(
        app,
        "shell-autostart-status",
        current.map_or(
            if autostart_ready {
                "自启动状态读取失败"
            } else {
                "开机启动不可用（开发或验证环境）"
            },
            autostart_text,
        ),
        false,
        None::<&str>,
    )?;
    let enable_autostart = MenuItem::with_id(
        app,
        "shell-autostart-enable",
        "启用开机启动",
        autostart_ready,
        None::<&str>,
    )?;
    let disable_autostart = MenuItem::with_id(
        app,
        "shell-autostart-disable",
        "关闭开机启动",
        autostart_ready,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let exit = MenuItem::with_id(
        app,
        "shell-exit",
        "退出桌面（后台任务由 Runtime 管理）",
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &show,
            &shortcut,
            &autostart,
            &enable_autostart,
            &disable_autostart,
            &separator,
            &exit,
        ],
    )?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or(tauri::Error::AssetNotFound("desktop icon".into()))?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("FlowTools · 桌面已驻留")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "shell-show" => restore(app),
            "shell-exit" => app.exit(0),
            "shell-autostart-enable" | "shell-autostart-disable" if autostart_ready => {
                // Only a native, explicit action changes OS state. No automatic
                // preference replay, webview command or caller-selected path.
                let manager = app.autolaunch();
                let result = policy::set_autostart(
                    autostart_ready,
                    event.id.as_ref() == "shell-autostart-enable",
                    |enabled| {
                        if enabled {
                            manager.enable()
                        } else {
                            manager.disable()
                        }
                    },
                    || manager.is_enabled(),
                );
                match result {
                    Some(enabled) => {
                        let _ = autostart.set_text(autostart_text(enabled));
                    }
                    None => {
                        let _ = autostart.set_text("自启动设置失败 · 状态未知，可重试");
                    }
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                restore(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
