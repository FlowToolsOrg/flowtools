//! Offline T0 recovery. The native Host must reserve the profile before calling.
use crate::{
    data::{healthy, safe_path, DataStore, SCHEMA_VERSION},
    protocol::*,
};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

const PENDING: &str = "runtime-recovery-pending";
const LAST: &str = "runtime-recovery-last.json";
const LIMIT: u64 = 67_108_864;
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    format_version: u16,
    recovery_id: String,
    backup_id: String,
    original_digest: String,
    stage_digest: String,
}

fn id(value: &str) -> Result<(), ErrorCode> {
    if uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value) {
        Ok(())
    } else {
        Err(ErrorCode::InvalidRequest)
    }
}
fn regular(path: &Path, limit: u64) -> Result<(), ErrorCode> {
    safe_path(path)?;
    let meta = std::fs::symlink_metadata(path).map_err(|_| ErrorCode::StorageFailed)?;
    if !meta.is_file() || meta.len() > limit {
        return Err(ErrorCode::StorageFailed);
    }
    Ok(())
}
fn hash(path: &Path) -> Result<String, ErrorCode> {
    regular(path, LIMIT)?;
    let mut file = std::fs::File::open(path).map_err(|_| ErrorCode::StorageFailed)?;
    let mut digest = Sha256::new();
    let mut bytes = [0; 8192];
    loop {
        let n = file
            .read(&mut bytes)
            .map_err(|_| ErrorCode::StorageFailed)?;
        if n == 0 {
            break;
        }
        digest.update(&bytes[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn write_new<T: Serialize>(path: &Path, value: &T) -> Result<(), ErrorCode> {
    safe_path(path)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| ErrorCode::StorageFailed)?;
    file.write_all(&serde_json::to_vec(value).map_err(|_| ErrorCode::StorageFailed)?)
        .map_err(|_| ErrorCode::StorageFailed)?;
    file.sync_all().map_err(|_| ErrorCode::StorageFailed)
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ErrorCode> {
    regular(path, 8192)?;
    serde_json::from_slice(&std::fs::read(path).map_err(|_| ErrorCode::StorageFailed)?)
        .map_err(|_| ErrorCode::StoreCorrupt)
}
fn inspect(path: &Path) -> Result<u32, ErrorCode> {
    regular(path, LIMIT)?;
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| ErrorCode::StoreCorrupt)?;
    healthy(&db)?;
    let version: u32 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| ErrorCode::StoreCorrupt)?;
    if version == 0 || version > SCHEMA_VERSION {
        return Err(ErrorCode::SchemaUnsupported);
    }
    Ok(version)
}
fn generation(path: &Path, expected: &str) -> bool {
    if regular(path, LIMIT).is_err() {
        return false;
    }
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .ok()
        .is_some_and(|db| {
            healthy(&db).is_ok()
                && db
                    .query_row(
                        "SELECT value FROM core_metadata WHERE key='recovery-generation'",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .is_ok_and(|value| value == expected)
        })
}
fn report(profile: &Path) -> Result<StorageReport, ErrorCode> {
    let mut backups = Vec::new();
    for entry in std::fs::read_dir(profile).map_err(|_| ErrorCode::StorageFailed)? {
        let entry = entry.map_err(|_| ErrorCode::StorageFailed)?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(value) = name
            .strip_prefix("runtime-backup-")
            .and_then(|n| n.strip_suffix(".sqlite"))
        else {
            continue;
        };
        if id(value).is_err() {
            continue;
        }
        if backups.len() >= 64 {
            return Err(ErrorCode::BudgetExceeded);
        }
        let inspected = inspect(&entry.path());
        backups.push(BackupInfo {
            id: value.into(),
            bytes: entry.metadata().map(|m| m.len() as f64).unwrap_or(0.0),
            schema_version: inspected.as_ref().ok().copied(),
            usable: inspected.is_ok(),
            error_code: inspected.err(),
        });
    }
    backups.sort_by(|a, b| a.id.cmp(&b.id));
    let recovery = if profile.join(PENDING).exists() {
        let journal: Journal = read_json(&profile.join(PENDING))?;
        validate_journal(&journal)?;
        Some(RecoveryReceipt {
            recovery_id: journal.recovery_id.clone(),
            backup_id: journal.backup_id,
            phase: if generation(&profile.join("runtime.sqlite"), &journal.recovery_id) {
                RecoveryPhase::Replaced
            } else {
                RecoveryPhase::Prepared
            },
            grants_revoked: generation(&profile.join("runtime.sqlite"), &journal.recovery_id),
            payloads_quarantined: profile
                .join(format!("private-jobs-quarantine-{}", journal.recovery_id))
                .exists(),
        })
    } else if profile.join(LAST).exists() {
        let receipt: RecoveryReceipt = read_json(&profile.join(LAST))?;
        id(&receipt.recovery_id).map_err(|_| ErrorCode::StoreCorrupt)?;
        id(&receipt.backup_id).map_err(|_| ErrorCode::StoreCorrupt)?;
        if receipt.phase != RecoveryPhase::Complete {
            return Err(ErrorCode::StoreCorrupt);
        }
        Some(receipt)
    } else {
        None
    };
    Ok(StorageReport {
        format_version: 1,
        backups,
        recovery,
    })
}
fn validate_journal(journal: &Journal) -> Result<(), ErrorCode> {
    id(&journal.recovery_id).map_err(|_| ErrorCode::StoreCorrupt)?;
    id(&journal.backup_id).map_err(|_| ErrorCode::StoreCorrupt)?;
    if journal.format_version != 1
        || [&journal.original_digest, &journal.stage_digest]
            .iter()
            .any(|v| v.len() != 64 || !v.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    Ok(())
}
fn sanitize(stage: &Path, recovery: &str) -> Result<(), ErrorCode> {
    let mut store = DataStore::open(stage)?;
    let tx = store
        .connection
        .transaction()
        .map_err(|_| ErrorCode::StorageFailed)?;
    let rows = tx
        .prepare("SELECT identity,epoch,state FROM grants")
        .map_err(|_| ErrorCode::StoreCorrupt)?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| ErrorCode::StoreCorrupt)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ErrorCode::StoreCorrupt)?;
    if rows.len() > 256 {
        return Err(ErrorCode::BudgetExceeded);
    }
    for (identity, epoch, state) in rows {
        let mut record: PermissionRecord =
            serde_json::from_str(&state).map_err(|_| ErrorCode::StoreCorrupt)?;
        if record.epoch != epoch
            || epoch == 0
            || serde_json::to_string(&record.identity).map_err(|_| ErrorCode::StoreCorrupt)?
                != identity
        {
            return Err(ErrorCode::StoreCorrupt);
        }
        record.epoch = epoch.checked_add(1).ok_or(ErrorCode::BudgetExceeded)?;
        record.grant = None;
        tx.execute(
            "UPDATE grants SET epoch=?1,state=?2 WHERE identity=?3",
            rusqlite::params![
                record.epoch,
                serde_json::to_string(&record).map_err(|_| ErrorCode::StorageFailed)?,
                identity
            ],
        )
        .map_err(|_| ErrorCode::StorageFailed)?;
    }
    crate::runtime::interrupt_restored_jobs(&tx)?;
    for (key, value) in [("cold-start", "false"), ("recovery-generation", recovery)] {
        tx.execute(
            "INSERT OR REPLACE INTO core_metadata(key,value) VALUES(?1,?2)",
            [key, value],
        )
        .map_err(|_| ErrorCode::StorageFailed)?;
    }
    tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
    drop(store);
    std::fs::OpenOptions::new()
        .write(true)
        .open(stage)
        .and_then(|f| f.sync_all())
        .map_err(|_| ErrorCode::StorageFailed)
}
fn replace(original: &Path, stage: &Path, preserved: &Path) -> Result<(), ErrorCode> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide = |p: &Path| {
            p.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        if unsafe {
            windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
                wide(original).as_ptr(),
                wide(stage).as_ptr(),
                wide(preserved).as_ptr(),
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
        std::fs::rename(original, preserved).map_err(|_| ErrorCode::StorageFailed)?;
        std::fs::rename(stage, original).map_err(|_| ErrorCode::StorageFailed)?;
    }
    Ok(())
}
fn resume(
    profile: &Path,
    hook: &mut impl FnMut(RecoveryPhase) -> Result<(), ErrorCode>,
) -> Result<(), ErrorCode> {
    let journal: Journal = read_json(&profile.join(PENDING))?;
    validate_journal(&journal)?;
    let original = profile.join("runtime.sqlite");
    let stage = profile.join(format!("runtime-recovery-{}.sqlite", journal.recovery_id));
    let preserved = profile.join(format!("runtime-corrupt-{}.sqlite", journal.recovery_id));
    let payloads = profile.join("private-jobs");
    let quarantine = profile.join(format!("private-jobs-quarantine-{}", journal.recovery_id));
    for path in [&original, &stage, &preserved, &payloads, &quarantine] {
        safe_path(path)?
    }
    if !generation(&original, &journal.recovery_id) {
        if hash(&stage)? != journal.stage_digest || !generation(&stage, &journal.recovery_id) {
            return Err(ErrorCode::StoreCorrupt);
        }
        if payloads.exists() {
            if !payloads.is_dir() || quarantine.exists() {
                return Err(ErrorCode::StorageFailed);
            }
            std::fs::rename(&payloads, &quarantine).map_err(|_| ErrorCode::StorageFailed)?;
        }
        if original.exists() {
            if preserved.exists() || hash(&original)? != journal.original_digest {
                return Err(ErrorCode::RevisionConflict);
            }
            replace(&original, &stage, &preserved)?;
        } else {
            // ReplaceFile may have renamed the original before failing. Keep it.
            if hash(&preserved)? != journal.original_digest {
                return Err(ErrorCode::RevisionConflict);
            }
            std::fs::rename(&stage, &original).map_err(|_| ErrorCode::StorageFailed)?;
        }
    }
    hook(RecoveryPhase::Replaced)?;
    inspect(&original)?;
    let receipt = RecoveryReceipt {
        recovery_id: journal.recovery_id.clone(),
        backup_id: journal.backup_id,
        phase: RecoveryPhase::Complete,
        grants_revoked: true,
        payloads_quarantined: quarantine.exists(),
    };
    // A generation-specific immutable receipt makes finalization retryable too.
    let durable = profile.join(format!("runtime-receipt-{}.json", journal.recovery_id));
    if !durable.exists() {
        write_new(&durable, &receipt)?
    }
    // Retry must not copy a redirected, truncated or unrelated receipt.
    let committed: RecoveryReceipt = read_json(&durable)?;
    if committed != receipt {
        return Err(ErrorCode::StoreCorrupt);
    }
    let last = profile.join(LAST);
    safe_path(&last)?;
    if last.exists() {
        regular(&last, 8192)?;
        std::fs::remove_file(&last).map_err(|_| ErrorCode::StorageFailed)?;
    }
    std::fs::copy(&durable, &last).map_err(|_| ErrorCode::StorageFailed)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&last)
        .and_then(|f| f.sync_all())
        .map_err(|_| ErrorCode::StorageFailed)?;
    std::fs::remove_file(profile.join(PENDING)).map_err(|_| ErrorCode::StorageFailed)?;
    Ok(())
}
pub fn manage(profile: &Path, action: StorageAction) -> Result<StorageReport, ErrorCode> {
    manage_with_hook(profile, action, |_| Ok(()))
}

pub fn validate_action(action: &StorageAction) -> Result<(), ErrorCode> {
    if let StorageAction::Restore { backup_id } = action {
        id(backup_id)?;
    }
    Ok(())
}
fn manage_with_hook(
    profile: &Path,
    action: StorageAction,
    mut hook: impl FnMut(RecoveryPhase) -> Result<(), ErrorCode>,
) -> Result<StorageReport, ErrorCode> {
    validate_action(&action)?;
    safe_path(profile)?;
    if !profile.is_dir() {
        return Err(ErrorCode::StorageFailed);
    }
    let pending = profile.join(PENDING);
    safe_path(&pending)?;
    match action {
        StorageAction::List => {}
        StorageAction::Create => {
            if pending.exists() {
                return Err(ErrorCode::RecoveryPending);
            }
            if report(profile)?.backups.len() >= 64 {
                return Err(ErrorCode::BudgetExceeded);
            }
            regular(&profile.join("runtime.sqlite"), LIMIT)?;
            let store = DataStore::open(&profile.join("runtime.sqlite"))?;
            let path = store.backup()?;
            std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .and_then(|f| f.sync_all())
                .map_err(|_| ErrorCode::StorageFailed)?;
        }
        StorageAction::Restore { backup_id } => {
            id(&backup_id)?;
            if pending.exists() {
                return Err(ErrorCode::RecoveryPending);
            }
            let backup = profile.join(format!("runtime-backup-{backup_id}.sqlite"));
            inspect(&backup)?;
            let original_digest = hash(&profile.join("runtime.sqlite"))?;
            let recovery_id = uuid::Uuid::new_v4().to_string();
            let stage: PathBuf = profile.join(format!("runtime-recovery-{recovery_id}.sqlite"));
            safe_path(&stage)?;
            let source = Connection::open_with_flags(&backup, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|_| ErrorCode::StoreCorrupt)?;
            source
                .backup(rusqlite::MAIN_DB, &stage, None)
                .map_err(|_| ErrorCode::StorageFailed)?;
            drop(source);
            sanitize(&stage, &recovery_id)?;
            write_new(
                &pending,
                &Journal {
                    format_version: 1,
                    recovery_id,
                    backup_id,
                    original_digest,
                    stage_digest: hash(&stage)?,
                },
            )?;
            hook(RecoveryPhase::Prepared)?;
            resume(profile, &mut hook)?;
        }
        StorageAction::Retry => {
            if !pending.exists() {
                return Err(ErrorCode::InvalidRequest);
            }
            resume(profile, &mut hook)?;
        }
    }
    report(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, String) {
        let profile = std::env::temp_dir().join(format!(
            "flowtools-validation-recovery-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&profile).unwrap();
        let store = DataStore::open(&profile.join("runtime.sqlite")).unwrap();
        store
            .connection
            .execute(
                "INSERT INTO core_metadata(key,value) VALUES('cold-start','true')",
                [],
            )
            .unwrap();
        drop(store);
        let report = manage(&profile, StorageAction::Create).unwrap();
        (profile, report.backups[0].id.clone())
    }
    #[test]
    fn corrupt_original_survives_and_retry_does_not_restore_authorization() {
        let (profile, backup_id) = fixture();
        std::fs::write(profile.join("runtime.sqlite"), b"private corrupt original").unwrap();
        std::fs::create_dir(profile.join("private-jobs")).unwrap();
        std::fs::write(profile.join("private-jobs/fixture"), b"private payload").unwrap();
        assert_eq!(
            manage_with_hook(&profile, StorageAction::Restore { backup_id }, |phase| {
                if matches!(phase, RecoveryPhase::Replaced) {
                    Err(ErrorCode::StorageFailed)
                } else {
                    Ok(())
                }
            })
            .err(),
            Some(ErrorCode::StorageFailed)
        );
        assert!(matches!(
            DataStore::open(&profile.join("runtime.sqlite")),
            Err(ErrorCode::RecoveryPending)
        ));
        let report = manage(&profile, StorageAction::Retry).unwrap();
        let receipt = report.recovery.unwrap();
        assert!(matches!(receipt.phase, RecoveryPhase::Complete));
        assert!(receipt.grants_revoked && receipt.payloads_quarantined);
        assert_eq!(
            std::fs::read(profile.join(format!("runtime-corrupt-{}.sqlite", receipt.recovery_id)))
                .unwrap(),
            b"private corrupt original"
        );
        assert_eq!(
            std::fs::read(profile.join(format!(
                "private-jobs-quarantine-{}/fixture",
                receipt.recovery_id
            )))
            .unwrap(),
            b"private payload"
        );
        let store = DataStore::open(&profile.join("runtime.sqlite")).unwrap();
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT value FROM core_metadata WHERE key='cold-start'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "false"
        );
        assert!(manage(
            &profile,
            StorageAction::Restore {
                backup_id: "../../private".into()
            }
        )
        .is_err());
    }
    #[test]
    fn prepared_retry_rejects_tampered_stage_without_touching_original() {
        let (profile, backup_id) = fixture();
        let before = hash(&profile.join("runtime.sqlite")).unwrap();
        assert!(
            manage_with_hook(&profile, StorageAction::Restore { backup_id }, |_| Err(
                ErrorCode::StorageFailed
            ))
            .is_err()
        );
        let journal: Journal = read_json(&profile.join(PENDING)).unwrap();
        std::fs::write(
            profile.join(format!("runtime-recovery-{}.sqlite", journal.recovery_id)),
            b"tampered",
        )
        .unwrap();
        assert_eq!(
            manage(&profile, StorageAction::Retry).err(),
            Some(ErrorCode::StoreCorrupt)
        );
        assert_eq!(hash(&profile.join("runtime.sqlite")).unwrap(), before);
        assert!(profile.join(PENDING).exists());
    }

    #[test]
    fn retry_completes_original_renamed_state_and_preserves_its_bytes() {
        let (profile, backup_id) = fixture();
        let before = hash(&profile.join("runtime.sqlite")).unwrap();
        assert!(
            manage_with_hook(&profile, StorageAction::Restore { backup_id }, |_| Err(
                ErrorCode::StorageFailed
            ))
            .is_err()
        );
        let journal: Journal = read_json(&profile.join(PENDING)).unwrap();
        let preserved = profile.join(format!("runtime-corrupt-{}.sqlite", journal.recovery_id));
        std::fs::rename(profile.join("runtime.sqlite"), &preserved).unwrap();
        assert!(matches!(
            DataStore::open(&profile.join("runtime.sqlite")),
            Err(ErrorCode::RecoveryPending)
        ));
        let report = manage(&profile, StorageAction::Retry).unwrap();
        assert!(matches!(
            report.recovery.unwrap().phase,
            RecoveryPhase::Complete
        ));
        assert_eq!(hash(&preserved).unwrap(), before);
    }

    #[test]
    fn malformed_job_metadata_refuses_restore_before_journal_or_original_changes() {
        let (profile, backup_id) = fixture();
        let before = hash(&profile.join("runtime.sqlite")).unwrap();
        let backup = profile.join(format!("runtime-backup-{backup_id}.sqlite"));
        let db = Connection::open(&backup).unwrap();
        db.execute(
            "INSERT INTO jobs(run_id,metadata) VALUES('invalid','{}')",
            [],
        )
        .unwrap();
        drop(db);
        assert_eq!(
            manage(&profile, StorageAction::Restore { backup_id }).err(),
            Some(ErrorCode::StoreCorrupt)
        );
        assert_eq!(hash(&profile.join("runtime.sqlite")).unwrap(), before);
        assert!(!profile.join(PENDING).exists());
    }

    #[test]
    fn altered_existing_receipt_keeps_recovery_pending_and_original_preserved() {
        let (profile, backup_id) = fixture();
        let before = hash(&profile.join("runtime.sqlite")).unwrap();
        assert!(
            manage_with_hook(&profile, StorageAction::Restore { backup_id }, |_| Err(
                ErrorCode::StorageFailed
            ))
            .is_err()
        );
        let journal: Journal = read_json(&profile.join(PENDING)).unwrap();
        let receipt = profile.join(format!("runtime-receipt-{}.json", journal.recovery_id));
        std::fs::write(receipt, b"truncated receipt").unwrap();
        assert_eq!(
            manage(&profile, StorageAction::Retry).err(),
            Some(ErrorCode::StoreCorrupt)
        );
        assert!(profile.join(PENDING).exists());
        assert!(!profile.join(LAST).exists());
        assert_eq!(
            hash(&profile.join(format!("runtime-corrupt-{}.sqlite", journal.recovery_id))).unwrap(),
            before
        );
        assert!(matches!(
            DataStore::open(&profile.join("runtime.sqlite")),
            Err(ErrorCode::RecoveryPending)
        ));
    }
}
