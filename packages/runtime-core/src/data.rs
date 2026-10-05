//! One SQLite writer owned by Runtime; clients receive values and revisions.
use crate::protocol::{DataMutation, DataSnapshot, ErrorCode, LegacyImport};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 2;
pub const MAX_DATA_BYTES: usize = 65_536;
const SCHEMA_V1: &str = "
CREATE TABLE core_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE grants(identity TEXT PRIMARY KEY, epoch INTEGER NOT NULL, state TEXT NOT NULL);
CREATE TABLE jobs(run_id TEXT PRIMARY KEY, metadata TEXT NOT NULL);
PRAGMA user_version=1;";
const SCHEMA_V2: &str = "
CREATE TABLE plugin_data(namespace TEXT NOT NULL, key TEXT NOT NULL, revision INTEGER NOT NULL, value TEXT NOT NULL, PRIMARY KEY(namespace,key));
CREATE TABLE imports(namespace TEXT NOT NULL, source TEXT NOT NULL, digest TEXT NOT NULL, revision INTEGER NOT NULL, PRIMARY KEY(namespace,source,digest));
PRAGMA user_version=2;";

pub struct DataStore {
    pub(crate) connection: Connection,
    path: Option<PathBuf>,
}

fn db_error(error: rusqlite::Error) -> ErrorCode {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
            ErrorCode::StoreBusy
        }
        Some(rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase) => {
            ErrorCode::StoreCorrupt
        }
        _ => ErrorCode::StorageFailed,
    }
}

pub fn safe_path(path: &Path) -> Result<(), ErrorCode> {
    if !path.is_absolute()
        || path.components().any(|p| {
            matches!(
                p,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(ErrorCode::StorageFailed);
    }
    for ancestor in path.ancestors() {
        if let Ok(metadata) = std::fs::symlink_metadata(ancestor) {
            if metadata.file_type().is_symlink() {
                return Err(ErrorCode::StorageFailed);
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(ErrorCode::StorageFailed);
                }
            }
        }
    }
    Ok(())
}

fn healthy(connection: &Connection) -> Result<(), ErrorCode> {
    let result: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(db_error)?;
    if result != "ok" {
        return Err(ErrorCode::StoreCorrupt);
    }
    Ok(())
}

/// Read-only inspection of a legacy Host-owned DB. No migration or deletion.
pub fn verify_legacy_database(path: &Path) -> Result<(), ErrorCode> {
    safe_path(path)?;
    let connection = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(db_error)?;
    healthy(&connection)
}

fn read(connection: &Connection, namespace: &str, key: &str) -> Result<DataSnapshot, ErrorCode> {
    validate_key(key)?;
    let stored: Option<(u32, String)> = connection
        .query_row(
            "SELECT revision,value FROM plugin_data WHERE namespace=?1 AND key=?2",
            params![namespace, key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(db_error)?;
    match stored {
        Some((revision, value)) => Ok(DataSnapshot {
            key: key.into(),
            revision,
            value: serde_json::from_str(&value).map_err(|_| ErrorCode::StoreCorrupt)?,
        }),
        None => Ok(DataSnapshot {
            key: key.into(),
            revision: 0,
            value: Value::Null,
        }),
    }
}

fn validate_key(key: &str) -> Result<(), ErrorCode> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || v == b'-' || v == b'_')
    {
        return Err(ErrorCode::InputInvalid);
    }
    Ok(())
}

impl DataStore {
    pub(crate) fn profile(&self) -> Option<&Path> {
        self.path.as_deref().and_then(|p| p.parent())
    }
    pub(crate) fn save_job(&self, run: &str, metadata: &Value) -> Result<(), ErrorCode> {
        let text = serde_json::to_string(metadata).map_err(|_| ErrorCode::StorageFailed)?;
        self.connection.execute("INSERT INTO jobs(run_id,metadata) VALUES(?1,?2) ON CONFLICT(run_id) DO UPDATE SET metadata=excluded.metadata",params![run,text]).map_err(db_error)?;
        Ok(())
    }
    pub(crate) fn load_jobs(&self) -> Result<Vec<Value>, ErrorCode> {
        let mut statement = self
            .connection
            .prepare("SELECT run_id,metadata FROM jobs ORDER BY run_id LIMIT 1025")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(db_error)?;
        let values = rows
            .map(|row| {
                let (run, text) = row.map_err(db_error)?;
                let value: Value =
                    serde_json::from_str(&text).map_err(|_| ErrorCode::StoreCorrupt)?;
                if value["snapshot"]["runId"] != run || text.len() > 32_768 {
                    return Err(ErrorCode::StoreCorrupt);
                }
                Ok(value)
            })
            .collect::<Result<Vec<Value>, ErrorCode>>()?;
        if values.len() > 1024 {
            return Err(ErrorCode::BudgetExceeded);
        }
        Ok(values)
    }
    pub fn memory() -> Result<Self, ErrorCode> {
        let mut store = Self {
            connection: Connection::open_in_memory().map_err(db_error)?,
            path: None,
        };
        store.migrate(|| Ok(()))?;
        Ok(store)
    }

    pub fn open(path: &Path) -> Result<Self, ErrorCode> {
        Self::open_with_hook(path, || Ok(()))
    }

    fn open_with_hook(
        path: &Path,
        hook: impl FnOnce() -> Result<(), ErrorCode>,
    ) -> Result<Self, ErrorCode> {
        safe_path(path)?;
        if path
            .parent()
            .is_some_and(|parent| parent.join("runtime-recovery-pending").exists())
        {
            return Err(ErrorCode::StorageFailed);
        }
        let mut store = Self {
            connection: Connection::open(path).map_err(db_error)?,
            path: Some(path.into()),
        };
        healthy(&store.connection)?;
        let version: u32 = store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(db_error)?;
        if version > SCHEMA_VERSION {
            return Err(ErrorCode::SchemaUnsupported);
        }
        // This profile cannot acquire a second SQLite writer, even in-process.
        store.connection.execute_batch("PRAGMA busy_timeout=0; PRAGMA locking_mode=EXCLUSIVE; PRAGMA synchronous=FULL; BEGIN EXCLUSIVE; COMMIT;").map_err(db_error)?;
        store.migrate(hook)?;
        Ok(store)
    }

    fn migrate(&mut self, hook: impl FnOnce() -> Result<(), ErrorCode>) -> Result<(), ErrorCode> {
        let version: u32 = self
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(db_error)?;
        if version == SCHEMA_VERSION {
            return Ok(());
        }
        if version > SCHEMA_VERSION {
            return Err(ErrorCode::SchemaUnsupported);
        }
        if version == 0 {
            let count: u32 = self
                .connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table'",
                    [],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            if count != 0 {
                return Err(ErrorCode::SchemaUnsupported);
            }
        }
        if version > 0 && self.path.is_some() {
            self.backup()?;
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(db_error)?;
        if version == 0 {
            transaction.execute_batch(SCHEMA_V1).map_err(db_error)?;
        }
        transaction.execute_batch(SCHEMA_V2).map_err(db_error)?;
        hook()?;
        transaction.commit().map_err(db_error)
    }

    pub fn read(&self, namespace: &str, key: &str) -> Result<DataSnapshot, ErrorCode> {
        read(&self.connection, namespace, key)
    }

    pub fn write(
        &mut self,
        namespace: &str,
        mutation: &DataMutation,
    ) -> Result<DataSnapshot, ErrorCode> {
        Ok(self
            .transaction(namespace, std::slice::from_ref(mutation))?
            .remove(0))
    }

    pub fn transaction(
        &mut self,
        namespace: &str,
        mutations: &[DataMutation],
    ) -> Result<Vec<DataSnapshot>, ErrorCode> {
        if mutations.is_empty()
            || mutations.len() > 16
            || mutations
                .iter()
                .enumerate()
                .any(|(i, m)| mutations[..i].iter().any(|other| other.key == m.key))
        {
            return Err(ErrorCode::InvalidRequest);
        }
        let mut total = 0;
        for mutation in mutations {
            validate_key(&mutation.key)?;
            let bytes = serde_json::to_vec(&mutation.value)
                .map_err(|_| ErrorCode::InputInvalid)?
                .len();
            total += bytes;
            if bytes > MAX_DATA_BYTES || total > 262_144 {
                return Err(ErrorCode::BudgetExceeded);
            }
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut results = Vec::new();
        for mutation in mutations {
            results.push(Self::write_in(&transaction, namespace, mutation)?);
        }
        transaction.commit().map_err(db_error)?;
        Ok(results)
    }

    fn write_in(
        connection: &Connection,
        namespace: &str,
        mutation: &DataMutation,
    ) -> Result<DataSnapshot, ErrorCode> {
        let current = read(connection, namespace, &mutation.key)?;
        if current.revision != mutation.expected_revision {
            return Err(ErrorCode::RevisionConflict);
        }
        let revision = current
            .revision
            .checked_add(1)
            .ok_or(ErrorCode::BudgetExceeded)?;
        let value = serde_json::to_string(&mutation.value).map_err(|_| ErrorCode::InputInvalid)?;
        connection.execute("INSERT INTO plugin_data(namespace,key,revision,value) VALUES(?1,?2,?3,?4) ON CONFLICT(namespace,key) DO UPDATE SET revision=excluded.revision,value=excluded.value", params![namespace,mutation.key,revision,value]).map_err(db_error)?;
        let (keys, bytes): (u32, u32) = connection.query_row(
            "SELECT count(*),COALESCE(sum(length(CAST(value AS BLOB))),0) FROM plugin_data WHERE namespace=?1",
            [namespace], |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(db_error)?;
        if keys > 256 || bytes > 4_194_304 {
            return Err(ErrorCode::BudgetExceeded);
        }
        Ok(DataSnapshot {
            key: mutation.key.clone(),
            revision,
            value: mutation.value.clone(),
        })
    }

    pub fn import_todos(
        &mut self,
        namespace: &str,
        import: &LegacyImport,
    ) -> Result<DataSnapshot, ErrorCode> {
        let items = import.value.as_array().ok_or(ErrorCode::InputInvalid)?;
        if crate::catalog::digest(&import.value) != import.source_digest
            || items.len() > 1000
            || items
                .iter()
                .any(|item| item["todo"].as_str().is_none() || item["deadline"].as_str().is_none())
        {
            return Err(ErrorCode::InputInvalid);
        }
        if serde_json::to_vec(&import.value)
            .map_err(|_| ErrorCode::InputInvalid)?
            .len()
            > MAX_DATA_BYTES
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let source = serde_json::to_string(&import.source).map_err(|_| ErrorCode::InputInvalid)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let previous: Option<u32> = transaction
            .query_row(
                "SELECT revision FROM imports WHERE namespace=?1 AND source=?2 AND digest=?3",
                params![namespace, source, import.source_digest],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if previous.is_some() {
            return read(&transaction, namespace, "todos");
        }
        let imported = Self::write_in(
            &transaction,
            namespace,
            &DataMutation {
                key: "todos".into(),
                expected_revision: 0,
                value: import.value.clone(),
            },
        )?;
        transaction
            .execute(
                "INSERT INTO imports(namespace,source,digest,revision) VALUES(?1,?2,?3,?4)",
                params![namespace, source, import.source_digest, imported.revision],
            )
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)?;
        Ok(imported)
    }

    pub fn backup(&self) -> Result<PathBuf, ErrorCode> {
        let parent = self
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .ok_or(ErrorCode::StorageFailed)?;
        let path = parent.join(format!("runtime-backup-{}.sqlite", uuid::Uuid::new_v4()));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| ErrorCode::StorageFailed)?;
        self.connection
            .backup(rusqlite::MAIN_DB, &path, None)
            .map_err(db_error)?;
        Ok(path)
    }

    /// Host-only recovery: same verified private profile, preserve corrupt original.
    /// Never expose this as an arbitrary-path plugin API.
    pub fn restore_backup(path: &Path, backup: &Path) -> Result<PathBuf, ErrorCode> {
        safe_path(path)?;
        safe_path(backup)?;
        if path.parent() != backup.parent()
            || !backup
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("runtime-backup-"))
        {
            return Err(ErrorCode::StorageFailed);
        }
        let source =
            Connection::open_with_flags(backup, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(db_error)?;
        healthy(&source)?;
        let version: u32 = source
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(db_error)?;
        if version == 0 || version > SCHEMA_VERSION {
            return Err(ErrorCode::SchemaUnsupported);
        }
        let parent = path.parent().ok_or(ErrorCode::StorageFailed)?;
        let stage = parent.join(format!("runtime-recovery-{}.sqlite", uuid::Uuid::new_v4()));
        source
            .backup(rusqlite::MAIN_DB, &stage, None)
            .map_err(db_error)?;
        let quarantine = parent.join(format!("runtime-corrupt-{}.sqlite", uuid::Uuid::new_v4()));
        // Interrupted or failed replacement must never look like a fresh profile.
        let marker = parent.join("runtime-recovery-pending");
        safe_path(&marker)?;
        let marker_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&marker)
            .map_err(|_| ErrorCode::StorageFailed)?;
        marker_file
            .sync_all()
            .map_err(|_| ErrorCode::StorageFailed)?;
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let wide = |p: &Path| {
                p.as_os_str()
                    .encode_wide()
                    .chain(Some(0))
                    .collect::<Vec<_>>()
            };
            let original = wide(path);
            let replacement = wide(&stage);
            let preserved = wide(&quarantine);
            if unsafe {
                windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
                    original.as_ptr(),
                    replacement.as_ptr(),
                    preserved.as_ptr(),
                    0,
                    std::ptr::null(),
                    std::ptr::null(),
                )
            } == 0
            {
                return Err(ErrorCode::StorageFailed);
            }
        }
        #[cfg(not(windows))]
        {
            std::fs::copy(path, &quarantine).map_err(|_| ErrorCode::StorageFailed)?;
            std::fs::rename(stage, path).map_err(|_| ErrorCode::StorageFailed)?;
        }
        std::fs::remove_file(marker).map_err(|_| ErrorCode::StorageFailed)?;
        Ok(quarantine)
    }
}

#[cfg(test)]
mod tests;
