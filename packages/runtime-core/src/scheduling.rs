//! P3.1b1: bounded, inert scheduling rules owned by Runtime.
//!
//! The cursor is inert; `store` adds single-writer claims, never authorization.
//! T0 supplies absolute UTC milliseconds; local calendar/DST resolution belongs
//! to a future explicit adapter. A fixed interval means elapsed milliseconds.

pub const MAX_TIMESTAMP_MS: u64 = 253_402_300_799_999;
pub const MAX_OCCURRENCES: u32 = 10_000;
pub const MIN_INTERVAL_MS: u64 = 1_000;
pub const MAX_GRACE_MS: u64 = 60_000;

pub mod commands;
pub mod store;
pub mod time;

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
    schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum MissedRunPolicy {
    /// Discard overdue occurrences outside the explicit lateness grace.
    Skip,
    /// Return only the latest due occurrence, even outside the grace.
    /// Older occurrences are discarded; no catch-up burst is produced.
    CoalesceLatest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    InvalidSchedule,
    TimeOutOfRange,
    ClockMovedBackward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Occurrence {
    /// Zero-based position in this immutable finite schedule, not a job ID.
    pub index: u32,
    pub due_ms: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ScheduleObservation {
    pub occurrence: Option<Occurrence>,
    /// Due positions discarded by this observation, excluding a returned one.
    pub skipped: u32,
    pub finished: bool,
}

/// A single in-memory planning cursor. Losing/recreating it can replay plans;
/// durable claims, recovery and effect idempotency are separate future work.
/// No serialization, wire API or production scheduler is provided here.
#[derive(Debug)]
pub struct ScheduleCursor {
    first_due_ms: u64,
    interval_ms: Option<u64>,
    occurrences: u32,
    grace_ms: u64,
    missed: MissedRunPolicy,
    next_index: u32,
    last_observed_ms: Option<u64>,
    cancelled: bool,
}

impl ScheduleCursor {
    pub fn once(
        due_ms: u64,
        grace_ms: u64,
        missed: MissedRunPolicy,
    ) -> Result<Self, ScheduleError> {
        Self::new(due_ms, None, 1, grace_ms, missed)
    }

    pub fn finite_interval(
        first_due_ms: u64,
        interval_ms: u64,
        occurrences: u32,
        grace_ms: u64,
        missed: MissedRunPolicy,
    ) -> Result<Self, ScheduleError> {
        Self::new(
            first_due_ms,
            Some(interval_ms),
            occurrences,
            grace_ms,
            missed,
        )
    }

    fn new(
        first_due_ms: u64,
        interval_ms: Option<u64>,
        occurrences: u32,
        grace_ms: u64,
        missed: MissedRunPolicy,
    ) -> Result<Self, ScheduleError> {
        if !(1..=MAX_OCCURRENCES).contains(&occurrences)
            || grace_ms > MAX_GRACE_MS
            || interval_ms
                .is_some_and(|period| !(MIN_INTERVAL_MS..=MAX_TIMESTAMP_MS).contains(&period))
        {
            return Err(ScheduleError::InvalidSchedule);
        }
        let last_due_ms = interval_ms
            .unwrap_or(0)
            .checked_mul(u64::from(occurrences - 1))
            .and_then(|span| first_due_ms.checked_add(span))
            .filter(|last| *last <= MAX_TIMESTAMP_MS);
        if last_due_ms.is_none() {
            return Err(ScheduleError::TimeOutOfRange);
        }
        Ok(Self {
            first_due_ms,
            interval_ms,
            occurrences,
            grace_ms,
            missed,
            next_index: 0,
            last_observed_ms: None,
            cancelled: false,
        })
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn next_due_ms(&self) -> Option<u64> {
        (!self.finished()).then(|| self.due_ms(self.next_index))
    }

    /// Consume all due positions in constant time, returning at most the latest.
    /// Equal timestamps cannot repeat an occurrence. A backwards T0 clock fails
    /// without changing the cursor; it never resets or rewinds the schedule.
    pub fn advance(&mut self, now_ms: u64) -> Result<ScheduleObservation, ScheduleError> {
        if now_ms > MAX_TIMESTAMP_MS {
            return Err(ScheduleError::TimeOutOfRange);
        }
        if self.last_observed_ms.is_some_and(|last| now_ms < last) {
            return Err(ScheduleError::ClockMovedBackward);
        }
        self.last_observed_ms = Some(now_ms);
        let mut observation = ScheduleObservation {
            occurrence: None,
            skipped: 0,
            finished: self.finished(),
        };
        if self.next_due_ms().is_none_or(|next| now_ms < next) {
            return Ok(observation);
        }
        let latest = self.interval_ms.map_or(0, |period| {
            ((now_ms - self.first_due_ms) / period).min(u64::from(self.occurrences - 1)) as u32
        });
        let due_ms = self.due_ms(latest);
        let return_latest =
            now_ms - due_ms <= self.grace_ms || self.missed == MissedRunPolicy::CoalesceLatest;
        observation.skipped = latest - self.next_index + 1 - u32::from(return_latest);
        observation.occurrence = return_latest.then_some(Occurrence {
            index: latest,
            due_ms,
        });
        self.next_index = latest + 1;
        observation.finished = self.finished();
        Ok(observation)
    }

    fn finished(&self) -> bool {
        self.cancelled || self.next_index == self.occurrences
    }

    fn due_ms(&self, index: u32) -> u64 {
        // The immutable schedule's complete span was checked at construction.
        self.first_due_ms + self.interval_ms.unwrap_or(0) * u64::from(index)
    }
}
