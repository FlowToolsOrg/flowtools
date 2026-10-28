# G7 schedule editing and restore disposal

2026-10-28, P3.1b4, prototype. Manual acceptance remains waived, not passed.

`schedules.replace` requires a management session, the previous exact revision,
a fresh explicitly approved definition/input and existing background grants.
Runtime validates current package identity and epoch before storing the encrypted
input. The old cancellation and the new immutable schedule anchor share one
SQLite commit. Active old runs refuse replacement; stale edits and failed writes
leave the old schedule intact. Exact request retries return the same replacement.
Old cancelled records and their claim positions remain available.

Desktop edits use structured fields, a new explicit time preview and native
consent. Private old inputs are never exposed by the metadata list: the user
re-enters the intended replacement content. CLI uses the same generated contract:
`flowtools schedules replace <id> --revision <n> --definition <file>
--approve-schedule`. The input file supplies a stable requestKey for retries.

`schedules.recovery` reads a bounded challenge tied to the exact stored schedule
state and the T0-derived canonical profile/restore generation. The separate
`schedules.discard-restored` action needs that challenge and native confirmation
or CLI `--approve-discard`. It permanently cancels all restored schedules,
preserves positions, private inputs, attempts and the clock floor, and changes
unknown claims to interrupted. It does not restore grants or execute anything.
Once recovery is disposed, repeating the action cannot cancel newly created
schedules; a later restore requires a new challenge. Corrupt records refuse
disposal rather than being reset. No caller paths, pins or time enter this API.

Validation: 18 focused Core scheduling tests passed, including actual SQLite
write refusal, atomic replacement, stale revisions, restored claims/floors and
challenge replay. Core all-target Clippy passed. Five pinned Chromium tests
passed for form creation, editing, DST refusal, browser refusal and explicit
challenge disposal. The actual native Host fixture passed no-client execution,
exact replacement retry, old revision rejection, cancellation and revocation.
These focused checks do not replace the required clean root gate.

Notification delivery, search and file/media workflows remain separate G7 work.
Earlier full-gate failures remain recorded in the shell/dispatch evidence.

2026-10-28 restart regression: generic private-job orphan cleanup previously
kept only job IDs, so it could delete a schedule's encrypted input before its
first dispatch after Host restart. Cleanup now also keeps all IDs from the
strictly validated schedule store. Retention accepts old scope only to preserve
bytes; dispatch still refuses copied/restored scope with `RECOVERY_PENDING`.
Corrupt state aborts cleanup. No grants, cursor or recovery markers are reset.
Two actual private-profile Core regressions passed: restart before first
dispatch executes the original input once; restore-generation mismatch retains
the input while refusing dispatch, and explicit disposal still preserves it.
