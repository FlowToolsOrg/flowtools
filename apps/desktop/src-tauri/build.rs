fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Source Desktop prototype: a build-owned locator, never caller input/PATH.
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target")
            });
        let runtime = target.join("debug/flowtools-runtime.exe");
        let runtime =
            std::fs::canonicalize(runtime).expect("Build Runtime before Desktop bindings");
        use sha2::{Digest, Sha256};
        let bytes = std::fs::read(&runtime).expect("Read fixed Runtime artifact");
        println!("cargo:rerun-if-env-changed=CARGO_TARGET_DIR");
        println!("cargo:rerun-if-changed={}", runtime.display());
        println!(
            "cargo:rustc-env=FLOWTOOLS_DESKTOP_RUNTIME_PATH={}",
            runtime.display()
        );
        println!(
            "cargo:rustc-env=FLOWTOOLS_DESKTOP_RUNTIME_SHA256={:x}",
            Sha256::digest(bytes)
        );
    }
    // Defining the application manifest makes custom commands subject to the
    // same window/origin ACL as official plugins. Validation permissions are
    // declared here but granted only by the explicit validation config.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "get_plugins",
        "get_plugin",
        "add_plugin",
        "update_plugin",
        "enable_plugin",
        "disable_plugin",
        "remove_plugin",
        "managed_runtime",
        "managed_runtime_disconnect",
        "managed_runtime_storage",
        "managed_runtime_control",
        "managed_resource_select",
        "validation_runtime",
        "validation_runtime_disconnect",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("Build the explicit application command ACL");

    // Tauri embeds its resources in app binaries, but integration-test harnesses
    // also need Common Controls v6 when linked code imports TaskDialogIndirect.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}
