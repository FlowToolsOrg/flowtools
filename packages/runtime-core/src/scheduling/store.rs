//! Durable finite schedule positions and claims in the existing DataStore.
//! A committed claim permits one dispatch attempt, not an external effect.
use super::{MissedRunPolicy, ScheduleCursor, MAX_TIMESTAMP_MS};
use crate::{broker::CommandIdentity, catalog::digest, data::DataStore, protocol::ErrorCode};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

const KEY: &str = "schedules-v1";
const MAX_SCHEDULES: usize = 128;
const MAX_BYTES: usize = 262_144;

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleDefinition {
    pub first_due_ms: u64,
    pub interval_ms: Option<u64>,
    pub occurrences: u32,
    pub grace_ms: u64,
    pub missed: MissedRunPolicy,
}
impl ScheduleDefinition {
    pub fn cursor(&self) -> Result<ScheduleCursor, ErrorCode> {
        match self.interval_ms {
            Some(interval) => ScheduleCursor::finite_interval(
                self.first_due_ms,
                interval,
                self.occurrences,
                self.grace_ms,
                self.missed,
            ),
            None if self.occurrences == 1 => {
                ScheduleCursor::once(self.first_due_ms, self.grace_ms, self.missed)
            }
            None => return Err(ErrorCode::InputInvalid),
        }
        .map_err(|_| ErrorCode::InputInvalid)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleBinding {
    pub identity: CommandIdentity,
    pub grant_epoch: u32,
    pub input_digest: String,
    pub timeout_ms: u32,
}

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, specta::Type, schemars::JsonSchema,
)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ClaimResult {
    Claimed,
    Submitted { run_id: String },
    Rejected { code: ErrorCode },
    Interrupted,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleClaim {
    pub token: String,
    pub index: u32,
    pub due_ms: u64,
    pub result: ClaimResult,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleRecord {
    pub id: String,
    pub revision: u32,
    pub definition: ScheduleDefinition,
    pub binding: ScheduleBinding,
    pub next_index: u32,
    pub last_observed_ms: Option<u64>,
    pub cancelled: bool,
    pub skipped: u32,
    pub last_claim: Option<ScheduleClaim>,
}
impl ScheduleRecord {
    fn cursor(&self) -> Result<ScheduleCursor, ErrorCode> {
        let mut cursor = self.definition.cursor()?;
        cursor.next_index = self.next_index;
        cursor.last_observed_ms = self.last_observed_ms;
        cursor.cancelled = self.cancelled;
        Ok(cursor)
    }
    pub fn next_due_ms(&self) -> Option<u64> {
        self.cursor().ok().and_then(|cursor| cursor.next_due_ms())
    }
    fn validate(&self) -> Result<(), ErrorCode> {
        let invalid = || ErrorCode::StoreCorrupt;
        self.definition.cursor().map_err(|_| invalid())?;
        if uuid::Uuid::parse_str(&self.id).is_err()
            || self.revision == 0
            || self.binding.grant_epoch == 0
            || self.binding.timeout_ms == 0
            || self.binding.timeout_ms > 300_000
            || !valid_digest(&self.binding.input_digest)
            || !valid_digest(&self.binding.identity.package_digest)
            || !matches!(
                self.binding.identity.caller.as_str(),
                "local-cli" | "local-desktop"
            )
            || self.next_index > self.definition.occurrences
            || self.skipped > self.next_index
            || self
                .last_observed_ms
                .is_some_and(|time| time > MAX_TIMESTAMP_MS)
        {
            return Err(invalid());
        }
        if let Some(claim) = &self.last_claim {
            if uuid::Uuid::parse_str(&claim.token).is_err()
                || claim.index >= self.next_index
                || claim.due_ms
                    != self.definition.first_due_ms
                        + self.definition.interval_ms.unwrap_or(0) * u64::from(claim.index)
                || matches!(&claim.result, ClaimResult::Submitted{run_id} if uuid::Uuid::parse_str(run_id).is_err())
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    format_version: u16,
    profile_digest: String,
    recovery_generation: Option<String>,
    clock_floor_ms: u64,
    records: Vec<ScheduleRecord>,
}

fn scope(store: &DataStore) -> Result<(String, Option<String>), ErrorCode> {
    let profile = if let Some(path) = store.database_path() {
        let canonical = std::fs::canonicalize(path).map_err(|_| ErrorCode::StorageFailed)?;
        digest(&serde_json::json!(canonical
            .to_str()
            .ok_or(ErrorCode::StorageFailed)?))
    } else {
        digest(&serde_json::json!("memory-only-schedules"))
    };
    let generation = store
        .connection
        .query_row(
            "SELECT value FROM core_metadata WHERE key='recovery-generation'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::StorageFailed)?;
    Ok((profile, generation))
}

fn load(store: &DataStore) -> Result<State, ErrorCode> {
    let (profile_digest, recovery_generation) = scope(store)?;
    let text: Option<String> = store
        .connection
        .query_row(
            "SELECT value FROM core_metadata WHERE key=?1",
            [KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::StorageFailed)?;
    let Some(text) = text else {
        return Ok(State {
            format_version: 1,
            profile_digest,
            recovery_generation,
            clock_floor_ms: 0,
            records: Vec::new(),
        });
    };
    if text.len() > MAX_BYTES {
        return Err(ErrorCode::StoreCorrupt);
    }
    let state: State = serde_json::from_str(&text).map_err(|_| ErrorCode::StoreCorrupt)?;
    if state.format_version != 1
        || state.clock_floor_ms > MAX_TIMESTAMP_MS
        || state.records.len() > MAX_SCHEDULES
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    if state.profile_digest != profile_digest || state.recovery_generation != recovery_generation {
        return Err(ErrorCode::RecoveryPending);
    }
    let mut ids = std::collections::HashSet::new();
    for record in &state.records {
        record.validate()?;
        if !ids.insert(&record.id)
            || record
                .last_observed_ms
                .is_some_and(|last| last > state.clock_floor_ms)
        {
            return Err(ErrorCode::StoreCorrupt);
        }
    }
    Ok(state)
}

fn save(store: &mut DataStore, state: &State) -> Result<(), ErrorCode> {
    let text = serde_json::to_string(state).map_err(|_| ErrorCode::StorageFailed)?;
    if text.len() > MAX_BYTES {
        return Err(ErrorCode::BudgetExceeded);
    }
    let tx = store
        .connection
        .transaction()
        .map_err(|_| ErrorCode::StorageFailed)?;
    tx.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![KEY,text]).map_err(|_| ErrorCode::StorageFailed)?;
    tx.commit().map_err(|_| ErrorCode::StorageFailed)
}

pub struct ScheduleStore;
impl ScheduleStore {
    pub fn list(store: &DataStore) -> Result<Vec<ScheduleRecord>, ErrorCode> {
        Ok(load(store)?.records)
    }
    pub fn create(
        store: &mut DataStore,
        id: String,
        definition: ScheduleDefinition,
        binding: ScheduleBinding,
        now_ms: u64,
    ) -> Result<ScheduleRecord, ErrorCode> {
        definition.cursor()?;
        let mut state = load(store)?;
        if now_ms > MAX_TIMESTAMP_MS || now_ms < state.clock_floor_ms {
            return Err(ErrorCode::InvalidRequest);
        }
        if state.records.len() >= MAX_SCHEDULES {
            return Err(ErrorCode::BudgetExceeded);
        }
        if state.records.iter().any(|record| record.id == id) {
            return Err(ErrorCode::IdempotencyConflict);
        }
        let record = ScheduleRecord {
            id,
            revision: 1,
            definition,
            binding,
            next_index: 0,
            last_observed_ms: None,
            cancelled: false,
            skipped: 0,
            last_claim: None,
        };
        record.validate().map_err(|_| ErrorCode::InputInvalid)?;
        state.clock_floor_ms = now_ms;
        state.records.push(record.clone());
        save(store, &state)?;
        Ok(record)
    }
    pub fn cancel(
        store: &mut DataStore,
        id: &str,
        revision: u32,
    ) -> Result<ScheduleRecord, ErrorCode> {
        let mut state = load(store)?;
        let record = state
            .records
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or(ErrorCode::JobNotFound)?;
        if record.revision != revision {
            return Err(ErrorCode::RevisionConflict);
        }
        if !record.cancelled {
            record.revision = record
                .revision
                .checked_add(1)
                .ok_or(ErrorCode::BudgetExceeded)?;
            record.cancelled = true;
        }
        let result = record.clone();
        save(store, &state)?;
        Ok(result)
    }
    /// T0 selects eligible schedules (e.g. suppressing overlap with active jobs).
    /// The position and random token commit before any dispatch attempt.
    pub fn claim_due(
        store: &mut DataStore,
        now_ms: u64,
        eligible: impl Fn(&ScheduleRecord) -> bool,
    ) -> Result<Vec<ScheduleRecord>, ErrorCode> {
        let mut state = load(store)?;
        if now_ms > MAX_TIMESTAMP_MS || now_ms < state.clock_floor_ms {
            return Err(ErrorCode::InvalidRequest);
        }
        let mut claims = Vec::new();
        let mut changed = false;
        for record in &mut state.records {
            if !eligible(record)
                || record.next_due_ms().is_none_or(|due| due > now_ms)
                || record
                    .last_claim
                    .as_ref()
                    .is_some_and(|claim| claim.result == ClaimResult::Claimed)
            {
                continue;
            }
            let mut cursor = record.cursor()?;
            let observation = cursor
                .advance(now_ms)
                .map_err(|_| ErrorCode::InvalidRequest)?;
            record.next_index = cursor.next_index;
            record.last_observed_ms = cursor.last_observed_ms;
            record.skipped += observation.skipped;
            if let Some(occurrence) = observation.occurrence {
                record.last_claim = Some(ScheduleClaim {
                    token: uuid::Uuid::new_v4().to_string(),
                    index: occurrence.index,
                    due_ms: occurrence.due_ms,
                    result: ClaimResult::Claimed,
                });
                claims.push(record.clone());
            }
            changed = true;
        }
        if changed {
            state.clock_floor_ms = now_ms;
            save(store, &state)?;
        }
        Ok(claims)
    }
    pub fn finish(
        store: &mut DataStore,
        id: &str,
        token: &str,
        result: ClaimResult,
    ) -> Result<(), ErrorCode> {
        if result == ClaimResult::Claimed {
            return Err(ErrorCode::InvalidRequest);
        }
        let mut state = load(store)?;
        let record = state
            .records
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or(ErrorCode::JobNotFound)?;
        let claim = record
            .last_claim
            .as_mut()
            .ok_or(ErrorCode::RevisionConflict)?;
        if claim.token != token {
            return Err(ErrorCode::RevisionConflict);
        }
        if claim.result != ClaimResult::Claimed {
            return if claim.result == result {
                Ok(())
            } else {
                Err(ErrorCode::RevisionConflict)
            };
        }
        claim.result = result;
        record.validate().map_err(|_| ErrorCode::InvalidRequest)?;
        save(store, &state)
    }
    /// A crash between a committed claim and outcome leaves uncertainty. Never
    /// repeat that attempt; report it as interrupted and preserve the position.
    pub fn recover(store: &mut DataStore) -> Result<(), ErrorCode> {
        let mut state = load(store)?;
        let mut changed = false;
        for record in &mut state.records {
            if let Some(claim) = &mut record.last_claim {
                if claim.result == ClaimResult::Claimed {
                    claim.result = ClaimResult::Interrupted;
                    changed = true;
                }
            }
        }
        if changed {
            save(store, &state)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
