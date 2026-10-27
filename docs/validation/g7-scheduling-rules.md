# G7 P3.1b1 bounded scheduling rules

Date: 2026-10-27. Prototype. The preceding desktop-shell engineering scope is
closed with [remaining manual acceptance waived](./g7-shell-closeout.md).

## Implemented scope

`packages/runtime-core/src/scheduling.rs` owns an inert Rust `ScheduleCursor`.
It plans a one-shot or finite elapsed interval from absolute UTC milliseconds.
Constructors validate the entire immutable span before returning a cursor:
1–10,000 occurrences, interval at least 1,000 ms, lateness grace 0–60,000 ms,
and timestamps from the Unix epoch through `253402300799999` ms. Overflow and
out-of-range schedules refuse before any state is created. No dependency was added.

Each `advance` consumes all due positions in constant time and returns at most
the latest due occurrence, its zero-based index, skipped count and completion.
`Skip` emits that latest position only within the chosen grace; `CoalesceLatest`
explicitly permits one overdue position even outside the grace. Older due
positions are discarded, never emitted in a catch-up burst. The interval stays
anchored at the original due time rather than drifting with observation latency.

Repeated equal-time observations cannot return the same position. A backwards
T0 clock returns `ClockMovedBackward` without moving the cursor or lowering its
observed clock floor. Invalid observation time likewise leaves state unchanged.
Cancellation is permanent and idempotent; subsequent observations cannot catch up.

The first due instant must already be resolved by T0. Intervals measure elapsed
milliseconds, including across DST transitions. This module does not resolve
local calendar times, timezone databases, repeated/nonexistent local hours or
daily wall-clock recurrences; those require a separate explicit adapter.

## Validation

Eleven actual Rust integration tests passed: one-shot timing and repeat refusal,
inclusive grace boundary, resume skip policy, a maximum-sized coalesced schedule,
explicit overdue one-shot choice, no interval drift, fresh latest/older skipped,
clock rollback preserving state, cancellation, invalid counts/intervals/grace/
timestamp overflow, and absolute elapsed instants across a DST boundary.
All-target Core Clippy with warnings fatal passed. Preimplementation compilation
failed because the scheduling module did not exist; that evidence is retained.

Ignored logs/exit files under `execution-validation/g7-shell/`:
`scheduling-red-20261027`, `scheduling-focused-20261027` and
`scheduling-clippy-20261027`.

## Boundaries and next step

This module has no timer, database, IPC DTO, plugin/SDK API, job, permission or
notification effect. Its output is not a grant, a durable claim or a job ID.
Losing or recreating the in-memory cursor can repeat a plan; no cross-process,
restart or exactly-once effect guarantee is claimed.

Next, bind schedule state and occurrence claims to the existing DataStore single
writer, define recovery/missed-run handling and authenticated management. Only
then connect dispatch through current package/command/grant/epoch checks and
separate unattended authorization. Notification delivery and unsupported-platform
feedback follow through a typed Host adapter. P3.1b/G7 implementation and G5/G6
execution prerequisites remain pending. SEC-013 stays open; independent security
acceptance is not inferred from arithmetic tests or the manual waiver.
