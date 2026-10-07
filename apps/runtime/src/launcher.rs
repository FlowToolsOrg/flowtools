mod bundle;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Lock {
    node: bundle::Artifact,
    cli: bundle::Artifact,
}

fn run() -> Result<i32, &'static str> {
    let root = std::env::current_exe()
        .map_err(|_| "BUNDLE_INVALID")?
        .parent()
        .ok_or("BUNDLE_INVALID")?
        .to_path_buf();
    let lock: Lock = serde_json::from_str(include_str!(concat!(
        env!("FLOWTOOLS_BUNDLE_BUILD_DIR"),
        "/launcher.json"
    )))
    .map_err(|_| "BUNDLE_INVALID")?;
    let node = bundle::verify(&root, &lock.node)?;
    let cli = bundle::verify(&root, &lock.cli)?;
    let mut command = std::process::Command::new(node);
    command
        .arg(cli)
        .args(std::env::args_os().skip(1))
        .env_clear();
    for key in ["SystemRoot", "LOCALAPPDATA"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .status()
        .map(|s| s.code().unwrap_or(1))
        .map_err(|_| "BUNDLE_START_FAILED")
}
fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(code) => {
            println!(
                "{}",
                serde_json::json!({"formatVersion":1,"success":false,"error":{"code":code}})
            );
            std::process::exit(1);
        }
    }
}
