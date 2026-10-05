use flowtools_runtime_core::{
    data::{safe_path, DataStore},
    protocol::ErrorCode,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bootstrap {
    pub format_version: u16,
    pub cli_token: String,
    pub desktop_token: String,
    pub management_token: String,
}

pub fn pipe(path: &Path, sid: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("{sid}|{}", path.display()).as_bytes());
    format!(r"\\.\pipe\flowtools-managed-{digest:x}")
}

pub fn initialize(path: &Path, sid: &str) -> Result<Bootstrap, ErrorCode> {
    safe_path(path)?;
    if path.exists()
        && std::fs::read_dir(path)
            .map_err(|_| ErrorCode::StorageFailed)?
            .next()
            .is_some()
    {
        return Err(ErrorCode::InvalidRequest);
    }
    // Create private directory before creating any credentials or SQLite content.
    crate::security::create_private_directory(path, sid).map_err(|_| ErrorCode::StorageFailed)?;
    let store = DataStore::open(&path.join("runtime.sqlite"))?;
    drop(store);
    let token = || {
        format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        )
    };
    let bootstrap = Bootstrap {
        format_version: 1,
        cli_token: token(),
        desktop_token: token(),
        management_token: token(),
    };
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.join("bootstrap.json"))
        .map_err(|_| ErrorCode::StorageFailed)?;
    file.write_all(&serde_json::to_vec(&bootstrap).map_err(|_| ErrorCode::StorageFailed)?)
        .map_err(|_| ErrorCode::StorageFailed)?;
    file.sync_all().map_err(|_| ErrorCode::StorageFailed)?;
    Ok(bootstrap)
}
pub fn load(path: &Path) -> Result<(PathBuf, Bootstrap), ErrorCode> {
    safe_path(path)?;
    safe_path(&path.join("bootstrap.json"))?;
    let sid = crate::security::current_user_sid().map_err(|_| ErrorCode::StorageFailed)?;
    crate::security::verify_private_path(path, &sid, true).map_err(|_| ErrorCode::StorageFailed)?;
    crate::security::verify_private_path(&path.join("bootstrap.json"), &sid, false)
        .map_err(|_| ErrorCode::StorageFailed)?;
    let bytes =
        std::fs::read(path.join("bootstrap.json")).map_err(|_| ErrorCode::ApprovalRequired)?;
    if bytes.len() > 2048 {
        return Err(ErrorCode::InvalidRequest);
    }
    let bootstrap: Bootstrap =
        serde_json::from_slice(&bytes).map_err(|_| ErrorCode::InvalidRequest)?;
    let tokens = [
        &bootstrap.cli_token,
        &bootstrap.desktop_token,
        &bootstrap.management_token,
    ];
    if bootstrap.format_version != 1
        || tokens
            .iter()
            .any(|t| t.len() != 64 || !t.bytes().all(|b| b.is_ascii_hexdigit()))
        || tokens[0] == tokens[1]
        || tokens[0] == tokens[2]
        || tokens[1] == tokens[2]
    {
        return Err(ErrorCode::InvalidRequest);
    }
    Ok((
        std::fs::canonicalize(path).map_err(|_| ErrorCode::StorageFailed)?,
        bootstrap,
    ))
}
