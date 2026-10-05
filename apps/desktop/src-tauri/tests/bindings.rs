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
