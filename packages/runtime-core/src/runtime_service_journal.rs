//! One bounded fixed slot, exact predecessor and trigger-aware writer operations.
use super::*;
const MAX_BYTES: usize = 65_536;

pub(super) fn read(connection: &rusqlite::Connection) -> Result<Option<String>, ErrorCode> {
    let value: Option<Option<String>> = connection.query_row(
        "SELECT CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=65536 THEN value ELSE NULL END FROM core_metadata WHERE key='service-catalog-state-v1'",
        [], |row| row.get(0),
    ).optional().map_err(|_| ErrorCode::StorageFailed)?;
    match value {
        None => Ok(None),
        Some(Some(text)) => Ok(Some(text)),
        Some(None) => Err(ErrorCode::StoreCorrupt),
    }
}

pub(super) fn load(
    connection: &rusqlite::Connection,
    catalog: &BuiltinCatalog,
) -> Result<(Option<String>, Vec<crate::dependencies::DependencyIdentity>), ErrorCode> {
    let raw = read(connection)?;
    let disabled = if let Some(text) = &raw {
        let journal: ProviderJournal =
            serde_json::from_str(text).map_err(|_| ErrorCode::StoreCorrupt)?;
        if journal.format_version != 1
            || journal.disabled.len() > 128
            || Uuid::parse_str(&journal.backup_id).is_err()
        {
            return Err(ErrorCode::StoreCorrupt);
        }
        if journal.phase != "committed"
            || journal.pending_catalog.is_some()
            || journal.active_catalog != catalog.snapshot_digest()
        {
            return Err(ErrorCode::RecoveryPending);
        }
        if journal.disabled.iter().any(|identity| {
            !catalog.list().any(|(id, manifest)| {
                id == &identity.id && manifest["publisher"] == identity.publisher
            })
        }) {
            return Err(ErrorCode::StoreCorrupt);
        }
        journal.disabled
    } else {
        Vec::new()
    };
    Ok((raw, disabled))
}

pub(super) fn write_in(
    tx: &rusqlite::Transaction<'_>,
    expected: Option<&str>,
    replacement: Option<&str>,
) -> Result<(), ErrorCode> {
    if tx.is_autocommit() || read(tx)?.as_deref() != expected {
        return Err(ErrorCode::StoreCorrupt);
    }
    if replacement.is_some_and(|value| value.len() > MAX_BYTES) {
        return Err(ErrorCode::BudgetExceeded);
    }
    let before = tx.total_changes();
    let affected = match replacement {
        Some(text) => tx.execute(
            "INSERT INTO core_metadata(key,value) VALUES('service-catalog-state-v1',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [text],
        ),
        None => {
            let expected = expected.ok_or(ErrorCode::StoreCorrupt)?;
            tx.execute("DELETE FROM core_metadata WHERE key='service-catalog-state-v1' AND typeof(value)='text' AND CAST(value AS BLOB)=CAST(?1 AS BLOB)", [expected])
        }
    }.map_err(|_| ErrorCode::StorageFailed)?;
    if affected != 1
        || before.checked_add(1) != Some(tx.total_changes())
        || read(tx)?.as_deref() != replacement
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    Ok(())
}

pub(super) fn replace(
    store: &mut DataStore,
    expected: Option<&str>,
    replacement: Option<&str>,
) -> Result<(), ErrorCode> {
    let tx = store
        .connection
        .transaction()
        .map_err(|_| ErrorCode::StorageFailed)?;
    write_in(&tx, expected, replacement)?;
    tx.commit().map_err(|_| ErrorCode::StorageFailed)
}

pub(super) struct MigrationFailure {
    pub code: ErrorCode,
    pub restored: bool,
}
/// The one provider journal transaction/restore path, shared by single and cohort updates.
pub(super) fn migrate(
    store: &mut DataStore,
    cached: &mut Option<String>,
    prepared: String,
    committed: String,
    migration: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<(), ErrorCode>,
    guard: impl Fn(&rusqlite::Connection) -> Result<(), ErrorCode>,
) -> Result<(), MigrationFailure> {
    let previous = cached.clone();
    let result = (|| {
        guard(&store.connection).map_err(|code| (code, false))?;
        replace(store, previous.as_deref(), Some(&prepared)).map_err(|code| (code, false))?;
        *cached = Some(prepared.clone());
        let tx = store
            .connection
            .transaction()
            .map_err(|_| (ErrorCode::StorageFailed, true))?;
        let result = guard(&tx).and_then(|()| migration(&tx)).and_then(|()| {
            guard(&tx)?;
            write_in(&tx, Some(&prepared), Some(&committed))?;
            guard(&tx)
        });
        match result {
            Ok(()) => tx.commit().map_err(|_| (ErrorCode::StorageFailed, false)),
            Err(code) => match tx.rollback() {
                Ok(()) => Err((code, true)),
                Err(_) => Err((ErrorCode::StorageFailed, false)),
            },
        }
    })();
    match result {
        Ok(()) => {
            *cached = Some(committed);
            Ok(())
        }
        Err((code, true)) => {
            guard(&store.connection)
                .and_then(|()| replace(store, Some(&prepared), previous.as_deref()))
                .map_err(|code| MigrationFailure {
                    code,
                    restored: false,
                })?;
            *cached = previous;
            Err(MigrationFailure {
                code,
                restored: true,
            })
        }
        Err((code, false)) => Err(MigrationFailure {
            code,
            restored: false,
        }),
    }
}
