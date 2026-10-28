//! One durable attempt per runner. Receipts never claim OS display/delivery.
use crate::{catalog::digest, data::DataStore, protocol::ErrorCode};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

const KEY: &str = "notification-attempts-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationStatus {
    Requested,
    Unsupported,
    Failed,
    Unknown,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Attempt {
    run_id: String,
    input_digest: String,
    status: NotificationStatus,
}

fn write(store: &mut DataStore, attempts: &[Attempt]) -> Result<(), ErrorCode> {
    let value = serde_json::to_string(attempts).map_err(|_| ErrorCode::StorageFailed)?;
    let tx = store
        .connection
        .transaction()
        .map_err(|_| ErrorCode::StorageFailed)?;
    tx.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", rusqlite::params![KEY,value]).map_err(|_| ErrorCode::StorageFailed)?;
    tx.commit().map_err(|_| ErrorCode::StorageFailed)
}

pub(crate) fn attempt(
    store: &mut DataStore,
    run_id: &str,
    title: &str,
    message: &str,
    adapter: impl FnOnce() -> NotificationStatus,
) -> Result<NotificationStatus, ErrorCode> {
    let raw: Option<String> = store
        .connection
        .query_row("SELECT value FROM core_metadata WHERE key=?1", [KEY], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|_| ErrorCode::StorageFailed)?;
    if raw.as_ref().is_some_and(|v| v.len() > 262_144) {
        return Err(ErrorCode::StoreCorrupt);
    }
    let mut records: Vec<Attempt> = raw
        .map(|v| serde_json::from_str(&v).map_err(|_| ErrorCode::StoreCorrupt))
        .transpose()?
        .unwrap_or_default();
    if records.len() > 1024 {
        return Err(ErrorCode::StoreCorrupt);
    }
    let mut ids = std::collections::HashSet::new();
    for record in &records {
        if uuid::Uuid::parse_str(&record.run_id).is_err()
            || !ids.insert(&record.run_id)
            || record.input_digest.len() != 64
            || !record.input_digest.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(ErrorCode::StoreCorrupt);
        }
    }
    let input_digest = digest(&serde_json::json!([title, message]));
    if let Some(record) = records.iter().find(|record| record.run_id == run_id) {
        return if record.input_digest == input_digest {
            Ok(record.status)
        } else {
            Err(ErrorCode::IdempotencyConflict)
        };
    }
    if records.len() >= 1024 {
        return Err(ErrorCode::BudgetExceeded);
    }
    if uuid::Uuid::parse_str(run_id).is_err() {
        return Err(ErrorCode::InvalidRequest);
    }
    records.push(Attempt {
        run_id: run_id.into(),
        input_digest,
        status: NotificationStatus::Unknown,
    });
    write(store, &records)?; // Commit uncertainty before entering the OS.
    let result = adapter();
    records.last_mut().unwrap().status = result;
    write(store, &records)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_retry_never_sends_twice_and_different_content_is_rejected() {
        let mut store = DataStore::memory().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        assert_eq!(
            attempt(&mut store, &id, "private title", "private body", || {
                NotificationStatus::Requested
            })
            .unwrap(),
            NotificationStatus::Requested
        );
        assert_eq!(
            attempt(&mut store, &id, "private title", "private body", || panic!(
                "must not resend"
            ))
            .unwrap(),
            NotificationStatus::Requested
        );
        assert_eq!(
            attempt(&mut store, &id, "different", "private body", || panic!(
                "must not send"
            ))
            .unwrap_err(),
            ErrorCode::IdempotencyConflict
        );
        let text: String = store
            .connection
            .query_row("SELECT value FROM core_metadata WHERE key=?1", [KEY], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(!text.contains("private"));
    }
    #[test]
    fn unknown_attempt_is_not_replayed_and_failed_commit_has_no_effect() {
        let mut store = DataStore::memory().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let record = Attempt {
            run_id: id.clone(),
            input_digest: digest(&serde_json::json!(["title", "body"])),
            status: NotificationStatus::Unknown,
        };
        write(&mut store, &[record]).unwrap();
        assert_eq!(
            attempt(&mut store, &id, "title", "body", || panic!(
                "crash uncertainty never replays"
            ))
            .unwrap(),
            NotificationStatus::Unknown
        );
        store
            .connection
            .execute_batch("PRAGMA query_only=ON")
            .unwrap();
        assert_eq!(
            attempt(
                &mut store,
                &uuid::Uuid::new_v4().to_string(),
                "title",
                "body",
                || panic!("commit before effect")
            )
            .unwrap_err(),
            ErrorCode::StorageFailed
        );
    }
}
