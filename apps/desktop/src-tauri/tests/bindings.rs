use std::time::{SystemTime, UNIX_EPOCH};

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
