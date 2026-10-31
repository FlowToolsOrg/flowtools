# G7 P3.1b3 authenticated Host scheduling

Date: 2026-10-27. Prototype. G7 implementation continues without manual gates.

## Implemented

The existing Host now owns a one-second scheduling tick. A finite occurrence
claim commits before ordinary `jobs.submit` acceptance. The scheduler binds the
root caller from T0's CLI/Desktop target, exact embedded command/package identity,
current grant epoch, private normalized input digest and timeout. Every attempt
rechecks current background authorization. Package/epoch changes or revocation
cancel the old schedule; regranting does not rearm it. Existing service leases,
runner validation, job deadlines, cancellation and process-group drain remain in
force. No plugin owns a timer, and Desktop exit never stops the Host.

`schedules.create` and `schedules.cancel` require an authenticated management
session. Creation needs an existing background grant and a separate explicit
scheduling action. Desktop reuses its native consent dialog, actual main-window
origin/Debug checks and manager transport; CLI uses a bounded definition file
plus `--approve-schedule`. That flag creates no grant. Retry keys bind the exact
normalized input/definition/identity and never rearm a cancelled schedule.

Inputs reuse the existing encrypted private payload store. Schedule metadata
contains identity, digest, counters and a stable outcome only. A failed claim
write returns no dispatch attempt. Unknown accepted outcomes fail closed; queue
backpressure retries queueing the same accepted run, never accepting it again.
There is no overlapping active job for the same schedule.

## Time and UI

Runtime-owned Jiff 0.2.37 resolves an explicit IANA zone and local datetime.
Ambiguous or nonexistent DST times reject by default; earlier/later resolution
requires an explicit choice. Returned UTC and numeric offset are previewed before
creation. Elapsed intervals stay distinct from daily wall-clock recurrence.
Desktop calls this pure function directly after its existing native guard;
preview starts no Host and initializes no profile. CLI uses the same wire contract
and implementation. Numeric DTO fields remain Rust `u64`; bounded safe-integer
domains use per-field Specta Number annotations and matching generated schemas.
The global BigInt refusal is unchanged. Client limits are generated from Rust.

Desktop settings provide structured primitive/enum command inputs, time preview,
finite count/interval, missed-run policy, query and revision-bound cancel. Mount
does no IO; browsers cannot use native controls. Composite inputs use a dedicated
command surface or the explicit CLI definition. Readback is a snapshot, never
claimed live. Editing timing invalidates the preview; unknown write outcomes do
not cause automatic retries. Full schedule editing and notifications follow next.

## Evidence

- Four Core tests: management/background refusal, private input acceptance after
  client disconnect/reopen, epoch change cancellation and exact create retry.
- Four timezone tests: explicit zone, repeated hour, missing hour and invalid
  zone/path/instant rejection.
- Three client tests: Rust-derived hostile wire rejection, numeric/finite limits
  and record/duplicate validation.
- Three pinned Chromium tests: browser/mount refusal, preview/create/cancel and
  DST refusal/preview invalidation. An initial duplicate implicit live region was
  fixed by giving the static time preview ordinary text semantics.
- Desktop: 36 Rust unit and six binding integration tests pass, including native
  confirmation privacy and existing actual IPC/origin refusals. Runtime Clippy
  with warnings fatal and package type checks passed.
- A real native Host fixture executes the actual compiled Base64 plugin when no
  GUI/CLI is connected, reads the genuine result, proves exactly one accepted run,
  and refuses revoked/cancelled schedules. Plaintext canaries are absent from
  SQLite metadata and the encrypted schedule input. The fixture's original stop
  cleanup left its own stdin open; closing that owned pipe and adding an overall
  watchdog fixed the harness without changing product deadlines or drain checks.

Ignored evidence under `execution-validation/g7-shell/`:
`scheduling-dispatch-20261027`, `scheduling-time-20261027`,
`scheduling-client-tests-20261027`, `scheduling-browser-20261027`,
`scheduling-native-desktop-20261027`, `scheduling-native-e2e-20261027` and
`scheduling-runtime-clippy-20261027` logs/exits.

The previous full local Runtime failure is retained, not overridden by these
focused checks. Previous `370ba6d` passed both remote Windows runs:
[push](https://github.com/FlowToolsOrg/flowtools/actions/runs/40516861445) and
[PR](https://github.com/FlowToolsOrg/flowtools/actions/runs/40516870101).
Latest source still needs its own complete integration result. Independent
security review, G5/G6 execution gates and whole-G7 completion are not claimed.
