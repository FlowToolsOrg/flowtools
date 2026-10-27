use flowtools_runtime_core::scheduling::{
    MissedRunPolicy, Occurrence, ScheduleCursor, ScheduleError, MAX_GRACE_MS, MAX_OCCURRENCES,
    MAX_TIMESTAMP_MS, MIN_INTERVAL_MS,
};

const FIRST: u64 = 100_000;
const PERIOD: u64 = 60_000;

#[test]
fn one_shot_waits_for_due_time_and_is_returned_only_once() {
    let mut schedule = ScheduleCursor::once(FIRST, 1_000, MissedRunPolicy::Skip).unwrap();
    assert_eq!(schedule.next_due_ms(), Some(FIRST));
    let early = schedule.advance(FIRST - 1).unwrap();
    assert_eq!(early.occurrence, None);
    assert!(!early.finished);
    let due = schedule.advance(FIRST).unwrap();
    assert_eq!(
        due.occurrence,
        Some(Occurrence {
            index: 0,
            due_ms: FIRST
        })
    );
    assert_eq!(due.skipped, 0);
    assert!(due.finished);
    assert_eq!(schedule.next_due_ms(), None);
    assert_eq!(schedule.advance(FIRST).unwrap().occurrence, None);
    assert_eq!(schedule.advance(FIRST + 1).unwrap().occurrence, None);
}

#[test]
fn skip_policy_accepts_the_grace_boundary_and_skips_one_millisecond_later() {
    for (lateness, returned) in [(0, true), (999, true), (1_000, true), (1_001, false)] {
        let mut schedule = ScheduleCursor::once(FIRST, 1_000, MissedRunPolicy::Skip).unwrap();
        let result = schedule.advance(FIRST + lateness).unwrap();
        assert_eq!(result.occurrence.is_some(), returned);
        assert_eq!(result.skipped, u32::from(!returned));
        assert!(result.finished);
    }
}

#[test]
fn resume_skips_stale_occurrences_without_replaying_them() {
    let mut schedule =
        ScheduleCursor::finite_interval(FIRST, PERIOD, 4, 1_000, MissedRunPolicy::Skip).unwrap();
    let result = schedule.advance(240_000).unwrap();
    assert_eq!(result.occurrence, None);
    assert_eq!(result.skipped, 3);
    assert!(!result.finished);
    assert_eq!(schedule.next_due_ms(), Some(280_000));
    assert_eq!(schedule.advance(240_000).unwrap().skipped, 0);
    assert_eq!(
        schedule.advance(280_000).unwrap().occurrence.unwrap().index,
        3
    );
    assert!(schedule.advance(280_001).unwrap().finished);
}

#[test]
fn coalescing_a_long_sleep_returns_only_the_latest_finite_occurrence() {
    let mut schedule = ScheduleCursor::finite_interval(
        FIRST,
        PERIOD,
        MAX_OCCURRENCES,
        0,
        MissedRunPolicy::CoalesceLatest,
    )
    .unwrap();
    let result = schedule.advance(MAX_TIMESTAMP_MS).unwrap();
    assert_eq!(result.skipped, MAX_OCCURRENCES - 1);
    assert_eq!(result.occurrence.unwrap().index, MAX_OCCURRENCES - 1);
    assert!(result.finished);
    assert_eq!(schedule.advance(MAX_TIMESTAMP_MS).unwrap().occurrence, None);
}

#[test]
fn overdue_one_shot_requires_the_explicit_coalescing_policy() {
    let mut schedule = ScheduleCursor::once(FIRST, 0, MissedRunPolicy::CoalesceLatest).unwrap();
    assert_eq!(
        schedule
            .advance(FIRST + PERIOD)
            .unwrap()
            .occurrence
            .unwrap()
            .due_ms,
        FIRST
    );
    assert!(schedule.advance(FIRST + PERIOD).unwrap().finished);
}

#[test]
fn late_observations_never_shift_the_fixed_interval_anchor() {
    let mut schedule =
        ScheduleCursor::finite_interval(FIRST, PERIOD, 3, 1_000, MissedRunPolicy::Skip).unwrap();
    assert_eq!(
        schedule
            .advance(100_500)
            .unwrap()
            .occurrence
            .unwrap()
            .due_ms,
        100_000
    );
    assert_eq!(schedule.next_due_ms(), Some(160_000));
    assert_eq!(
        schedule
            .advance(160_999)
            .unwrap()
            .occurrence
            .unwrap()
            .due_ms,
        160_000
    );
    assert_eq!(schedule.next_due_ms(), Some(220_000));
    assert_eq!(
        schedule.advance(220_000).unwrap().occurrence.unwrap().index,
        2
    );
    assert_eq!(schedule.next_due_ms(), None);
}

#[test]
fn a_fresh_latest_occurrence_can_run_while_older_ones_are_skipped() {
    for policy in [MissedRunPolicy::Skip, MissedRunPolicy::CoalesceLatest] {
        let mut schedule =
            ScheduleCursor::finite_interval(FIRST, PERIOD, 4, 1_000, policy).unwrap();
        let result = schedule.advance(280_500).unwrap();
        assert_eq!(
            result.occurrence,
            Some(Occurrence {
                index: 3,
                due_ms: 280_000
            })
        );
        assert_eq!(result.skipped, 3);
        assert!(result.finished);
    }
}

#[test]
fn clock_rollback_is_refused_without_consuming_the_next_occurrence() {
    let mut schedule =
        ScheduleCursor::finite_interval(FIRST, PERIOD, 3, 1_000, MissedRunPolicy::Skip).unwrap();
    schedule.advance(FIRST).unwrap();
    assert_eq!(
        schedule.advance(FIRST - 1),
        Err(ScheduleError::ClockMovedBackward)
    );
    assert_eq!(schedule.next_due_ms(), Some(160_000));
    assert_eq!(schedule.advance(FIRST).unwrap().occurrence, None);
    assert_eq!(
        schedule.advance(160_000).unwrap().occurrence.unwrap().index,
        1
    );
}

#[test]
fn cancellation_is_idempotent_and_prevents_catch_up() {
    let mut schedule =
        ScheduleCursor::finite_interval(FIRST, PERIOD, 4, 0, MissedRunPolicy::CoalesceLatest)
            .unwrap();
    schedule.cancel();
    schedule.cancel();
    assert_eq!(schedule.next_due_ms(), None);
    let result = schedule.advance(MAX_TIMESTAMP_MS).unwrap();
    assert_eq!(result.occurrence, None);
    assert_eq!(result.skipped, 0);
    assert!(result.finished);
}

#[test]
fn invalid_counts_intervals_grace_and_timestamp_overflow_are_refused() {
    for count in [0, MAX_OCCURRENCES + 1, u32::MAX] {
        assert!(
            ScheduleCursor::finite_interval(FIRST, PERIOD, count, 0, MissedRunPolicy::Skip)
                .is_err()
        );
    }
    for period in [0, MIN_INTERVAL_MS - 1, u64::MAX] {
        assert!(
            ScheduleCursor::finite_interval(FIRST, period, 2, 0, MissedRunPolicy::Skip).is_err()
        );
    }
    assert!(ScheduleCursor::once(FIRST, MAX_GRACE_MS + 1, MissedRunPolicy::Skip).is_err());
    assert!(ScheduleCursor::once(u64::MAX, 0, MissedRunPolicy::Skip).is_err());
    assert!(
        ScheduleCursor::finite_interval(MAX_TIMESTAMP_MS, PERIOD, 2, 0, MissedRunPolicy::Skip)
            .is_err()
    );
    let mut schedule = ScheduleCursor::once(MAX_TIMESTAMP_MS, 0, MissedRunPolicy::Skip).unwrap();
    assert_eq!(
        schedule.advance(u64::MAX),
        Err(ScheduleError::TimeOutOfRange)
    );
    assert_eq!(schedule.next_due_ms(), Some(MAX_TIMESTAMP_MS));
    assert!(schedule
        .advance(MAX_TIMESTAMP_MS)
        .unwrap()
        .occurrence
        .is_some());
}

#[test]
fn elapsed_intervals_across_a_dst_boundary_stay_absolute_utc_instants() {
    // 2026-11-01T05:30Z: New York's repeated local hour is deliberately not resolved here.
    let first = 1_793_511_000_000;
    let mut schedule =
        ScheduleCursor::finite_interval(first, 3_600_000, 2, 0, MissedRunPolicy::Skip).unwrap();
    assert_eq!(
        schedule.advance(first).unwrap().occurrence.unwrap().due_ms,
        first
    );
    let next = schedule.advance(1_793_514_600_000).unwrap();
    assert_eq!(next.occurrence.unwrap().index, 1);
    assert!(next.finished);
}
