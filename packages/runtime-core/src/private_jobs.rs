//! Private payloads are separate from SQLite metadata and diagnostic events.
use crate::{data::safe_path, protocol::ErrorCode};
use serde_json::Value;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
pub const MAX_BYTES: usize = 1_048_576;
pub struct PrivateJobs {
    root: PathBuf,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Payload {
    format_version: u16,
    run_id: String,
    kind: String,
    digest: String,
    value: Value,
}

#[cfg(windows)]
fn crypt(bytes: &[u8], context: &str, encrypt: bool) -> Result<Vec<u8>, ErrorCode> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: context.len() as u32,
        pbData: context.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    unsafe {
        let success = if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if success == 0 {
            return Err(ErrorCode::StorageFailed);
        }
        let result = if output.cbData as usize > MAX_BYTES + 16384 {
            Err(ErrorCode::BudgetExceeded)
        } else {
            Ok(std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec())
        };
        LocalFree(output.pbData.cast());
        result
    }
}
#[cfg(not(windows))]
fn crypt(_bytes: &[u8], _context: &str, _encrypt: bool) -> Result<Vec<u8>, ErrorCode> {
    Err(ErrorCode::StorageFailed)
}
impl PrivateJobs {
    pub fn open(profile: &Path) -> Result<Self, ErrorCode> {
        let root = profile.join("private-jobs");
        safe_path(&root)?;
        std::fs::create_dir_all(&root).map_err(|_| ErrorCode::StorageFailed)?;
        Ok(Self { root })
    }
    fn path(&self, run: &str, kind: &str) -> Result<PathBuf, ErrorCode> {
        if uuid::Uuid::parse_str(run).is_err() || !matches!(kind, "input" | "output") {
            return Err(ErrorCode::InvalidRequest);
        }
        let path = self.root.join(format!("{run}.{kind}.bin"));
        safe_path(&path)?;
        Ok(path)
    }
    pub fn write(&self, run: &str, kind: &str, value: &Value) -> Result<(), ErrorCode> {
        let bytes = serde_json::to_vec(value).map_err(|_| ErrorCode::InputInvalid)?;
        if bytes.len() > MAX_BYTES {
            return Err(ErrorCode::BudgetExceeded);
        }
        let mut total = 0;
        let mut count = 0;
        for entry in std::fs::read_dir(&self.root).map_err(|_| ErrorCode::StorageFailed)? {
            let path = entry.map_err(|_| ErrorCode::StorageFailed)?.path();
            safe_path(&path)?;
            let metadata = std::fs::symlink_metadata(path).map_err(|_| ErrorCode::StorageFailed)?;
            if !metadata.is_file() {
                return Err(ErrorCode::StorageFailed);
            }
            total += metadata.len();
            count += 1;
        }
        if count >= 256 || total + bytes.len() as u64 + 16384 > 67_108_864 {
            return Err(ErrorCode::BudgetExceeded);
        }
        let payload = Payload {
            format_version: 1,
            run_id: run.into(),
            kind: kind.into(),
            digest: crate::catalog::digest(value),
            value: value.clone(),
        };
        let protected = crypt(
            &serde_json::to_vec(&payload).map_err(|_| ErrorCode::StorageFailed)?,
            &format!("flowtools-job-v1|{run}|{kind}"),
            true,
        )?;
        let path = self.path(run, kind)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| ErrorCode::StorageFailed)?;
        if file
            .write_all(&protected)
            .and_then(|_| file.sync_all())
            .is_err()
        {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(ErrorCode::StorageFailed);
        }
        Ok(())
    }
    pub fn read(&self, run: &str, kind: &str) -> Result<Value, ErrorCode> {
        let path = self.path(run, kind)?;
        if std::fs::metadata(&path)
            .map_err(|_| ErrorCode::StorageFailed)?
            .len()
            > MAX_BYTES as u64 + 16384
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let bytes = std::fs::read(path).map_err(|_| ErrorCode::StorageFailed)?;
        let decrypted = crypt(&bytes, &format!("flowtools-job-v1|{run}|{kind}"), false)?;
        let payload: Payload =
            serde_json::from_slice(&decrypted).map_err(|_| ErrorCode::StoreCorrupt)?;
        if payload.format_version != 1
            || payload.run_id != run
            || payload.kind != kind
            || payload.digest != crate::catalog::digest(&payload.value)
            || serde_json::to_vec(&payload.value)
                .map_err(|_| ErrorCode::StoreCorrupt)?
                .len()
                > MAX_BYTES
        {
            return Err(ErrorCode::StoreCorrupt);
        }
        Ok(payload.value)
    }
    pub fn remove(&self, run: &str, kind: &str) -> Result<(), ErrorCode> {
        let path = self.path(run, kind)?;
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(ErrorCode::StorageFailed),
        }
    }
    pub fn remove_orphans(
        &self,
        runs: &std::collections::HashSet<String>,
    ) -> Result<(), ErrorCode> {
        for entry in std::fs::read_dir(&self.root).map_err(|_| ErrorCode::StorageFailed)? {
            let path = entry.map_err(|_| ErrorCode::StorageFailed)?.path();
            safe_path(&path)?;
            let name = path
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or(ErrorCode::StoreCorrupt)?;
            let parts: Vec<_> = name.split('.').collect();
            if parts.len() != 3
                || uuid::Uuid::parse_str(parts[0]).is_err()
                || !matches!(parts[1], "input" | "output")
                || parts[2] != "bin"
            {
                return Err(ErrorCode::StoreCorrupt);
            }
            if !runs.contains(parts[0]) {
                self.remove(parts[0], parts[1])?;
            }
        }
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn private_payload_is_bound_encrypted_bounded_and_removable() {
        let profile = std::env::temp_dir().join(format!(
            "flowtools-validation-payload-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&profile).unwrap();
        let store = PrivateJobs::open(&profile).unwrap();
        let run = uuid::Uuid::new_v4().to_string();
        let value = serde_json::json!({"text":"private-payload-canary"});
        store.write(&run, "input", &value).unwrap();
        assert_eq!(store.read(&run, "input").unwrap(), value);
        let bytes = std::fs::read(store.path(&run, "input").unwrap()).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("private-payload-canary"));
        let other = uuid::Uuid::new_v4().to_string();
        std::fs::write(store.path(&other, "input").unwrap(), &bytes).unwrap();
        assert!(store.read(&other, "input").is_err());
        assert!(store.write("../escape", "input", &value).is_err());
        assert!(store
            .write(&run, "output", &serde_json::json!("x".repeat(MAX_BYTES)))
            .is_err());
        store.remove(&run, "input").unwrap();
        assert!(store.read(&run, "input").is_err());
    }
}
