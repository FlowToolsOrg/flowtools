use std::time::{SystemTime, UNIX_EPOCH};

fn assert_validation_window_starts_at_root(config_json: &str, marker: &str) {
    let config: serde_json::Value = serde_json::from_str(config_json).unwrap();
    let url: tauri::WebviewUrl =
        serde_json::from_value(config["app"]["windows"][0]["url"].clone()).unwrap();
    // The real Tauri URL resolver runs, but no Wry host or user database starts.
    let app = tauri::test::mock_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "validation-root", url)
        .build()
        .unwrap();
    let resolved = window.url().unwrap();
    assert_eq!(
        resolved.path(),
        "/",
        "Validation must enter the actual launcher route"
    );
    assert_eq!(
        resolved
            .query_pairs()
            .find(|(key, _)| key == "execution-validation")
            .map(|(_, value)| value.into_owned()),
        Some(marker.to_string())
    );
}

#[test]
fn automated_validation_window_enters_the_launcher_route() {
    assert_validation_window_starts_at_root(
        include_str!("../../tauri.execution-validation.conf.json"),
        "20261003",
    );
}

#[test]
fn manual_validation_window_enters_the_launcher_route() {
    assert_validation_window_starts_at_root(
        include_str!("../../tauri.manual-validation.conf.json"),
        "20261004-manual",
    );
}

#[test]
fn exports_current_bindings_without_launching_a_host() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "flowtools-binding-test-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("bindings.ts");
    desktop_lib::export_bindings(&path).unwrap();
    let first = std::fs::read_to_string(&path).unwrap();
    for expected in [
        "getPlugins",
        "addPlugin",
        "removePlugin",
        "cliAvailable",
        "createdAt",
        "validationRuntime",
        "validationRuntimeDisconnect",
        "JobReceipt",
        "SessionProof",
    ] {
        assert!(
            first.contains(expected),
            "Missing generated contract: {expected}"
        );
    }
    desktop_lib::export_bindings(&path).unwrap();
    assert_eq!(first, std::fs::read_to_string(&path).unwrap());
    assert!(desktop_lib::export_bindings(&directory).is_err());
    std::fs::remove_dir_all(&directory).unwrap();
}

#[cfg(windows)]
mod ipc_tests {
    #[test]
    fn native_tray_status_refuses_missing_main_foreign_packaged_and_unconfigured_origins() {
        for (label, origin, configured) in [
            ("external", "http://localhost:1420", true),
            ("main", "https://untrusted.invalid", true),
            ("main", "http://tauri.localhost", true),
            ("main", "http://localhost:1420", false),
        ] {
            let mut context = tauri::generate_context!();
            context.config_mut().build.dev_url =
                configured.then(|| "http://localhost:1420".parse().unwrap());
            let app = tauri::test::mock_builder().build(context).unwrap();
            let _window = tauri::WebviewWindowBuilder::new(
                &app,
                label,
                tauri::WebviewUrl::External(origin.parse().unwrap()),
            )
            .build()
            .unwrap();
            let result =
                tauri::async_runtime::block_on(desktop_lib::mock_shell_status(app.handle()));
            assert_eq!(
                result.unwrap_err().code,
                flowtools_runtime_core::protocol::ErrorCode::SessionInvalid
            );
        }
    }

    #[test]
    fn status_control_refuses_untrusted_windows_origins_and_initialization() {
        for (label, origin, initialize) in [
            ("external", "http://localhost:1420", false),
            ("main", "https://untrusted.invalid", false),
            ("main", "http://localhost:1420", true),
        ] {
            let mut context = tauri::generate_context!();
            context.config_mut().build.dev_url = Some("http://localhost:1420".parse().unwrap());
            let app = desktop_lib::mock_command_builder().build(context).unwrap();
            let window = tauri::WebviewWindowBuilder::new(
                &app,
                label,
                tauri::WebviewUrl::External(origin.parse().unwrap()),
            )
            .build()
            .unwrap();
            let response = tauri::test::get_ipc_response(
                &window,
                tauri::webview::InvokeRequest {
                    cmd: "managed_runtime_control".into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    // Reach the native handler; its guard must inspect the real
                    // window URL/label even with a trusted invoke URL.
                    url: "http://tauri.localhost".parse().unwrap(),
                    body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                        "call": {"method":"runtime.status"}, "initialize":initialize
                    })),
                    headers: Default::default(),
                    invoke_key: tauri::test::INVOKE_KEY.into(),
                },
            );
            assert_eq!(
                response.unwrap_err(),
                serde_json::json!({"code":"SESSION_INVALID"})
            );
        }
    }

    #[test]
    fn shell_webview_cannot_create_spoofed_shell_menus_or_change_native_startup_and_shortcuts() {
        let app = tauri::test::mock_builder()
            .build(tauri::generate_context!())
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        for command in [
            "plugin:menu|new",
            "plugin:tray|new",
            "plugin:tray|remove_by_id",
            "plugin:tray|set_visible",
            "plugin:autostart|enable",
            "plugin:autostart|disable",
            "plugin:global-shortcut|register",
        ] {
            let response = tauri::test::get_ipc_response(
                &window,
                tauri::webview::InvokeRequest {
                    cmd: command.into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: "http://tauri.localhost".parse().unwrap(),
                    body: tauri::ipc::InvokeBody::Json(serde_json::json!({})),
                    headers: Default::default(),
                    invoke_key: tauri::test::INVOKE_KEY.into(),
                },
            );
            let Err(error) = response else {
                panic!("native shell IPC must be denied");
            };
            let text = error.to_string();
            assert!(text.contains("not allowed"), "{command}: {text}");
        }
    }
}
