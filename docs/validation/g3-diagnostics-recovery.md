# G3 P1.5b run diagnosis and recovery

Scope: Windows x64/current-user fixed T1 prototype. SEC-002/003/004/006/009/010/013/015
and ADR-0001/0002 apply. Independent Security Reviewer/date/conclusion: pending.

`packages/runtime-core/src/runtime.rs` derives diagnostics from stored run
metadata without loading private output. `jobs.diagnose` is authenticated business
IPC; offline storage actions are a separate T0 native interface. Rust generates
all DTOs/schemas/rejections. Exact client 0.3.0 rejects old 0.2.0 at handshake;
protocol major 1 and database schema 2 remain unchanged.

Diagnostics select run/parent/caller/plugin/command/package/version/digest/dependency
lock/grant epoch, state/sequence, accepted/start/finish/duration, background,
stable failureCode, resultExpired and requiresReview. Inputs, result content,
resource paths, environment, credentials and arbitrary messages never enter this
summary/export. Interrupted work and failed/cancelled effectful work require review.
No cross-service child chain is fabricated: services remain G4 future scope.

The full gate reproduced readiness disconnection under 50 concurrent launchers.
CLI status/start can reauthenticate the same instance and re-query
status, including transient disconnection during reconnection, within the same
original finite retry window. They do not resubmit jobs or retry stop,
grants, writes, instance changes or authorization refusals. Per-frame 5-second
bounds remain unchanged. Contract tests assert the exact read-only method set;
the actual 50-process acceptance still launches all clients concurrently.
The native listener preserves ownership while replacing only an aborted peer
connection; access/other errors remain fatal. A real early peer exit and subsequent
Windows handshake are regression-tested. See [ConnectNamedPipe behavior](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-connectnamedpipe).
The CLI separates fixed executable/bootstrap validation into native-runtime.ts.
Standalone Bun compilation can instantiate an application error class twice, so
phase serialization checks the shared public RuntimeClientError base and the
finite connect/query/reconnect and pipe/authenticate field allowlists. Actual compiled CLI and relocated
bundle tests assert offline failure phase and nonzero exit; no private message
or resource is serialized.
The native fixture assigns each new key its own original 10-second budget;
same-key retries preserve the initial absolute deadline.

`packages/runtime-core/src/recovery.rs` and `apps/runtime/src/startup.rs` own the
offline pipeline. The private profile ACL, same startup mutex and first-instance
named pipe are verified/reserved before SQLite IO; active Host returns STORE_BUSY.
Only canonical backup UUIDs are accepted. Same-parent regular files and every path
ancestor reject reparse/symlink traversal. Backups use SQLite backup, not a raw copy
of an active database. Each file is bounded to 64 MiB and inventory to 64 backups.

Restore uses a sanitized stage and synced immutable recovery journal. All restored
grants are revoked, cold start disabled and nonterminal jobs interrupted before
the replacement is visible. Original DB and private-job directory remain in
same-profile quarantine. Encrypted private payloads are not placed in backup or
support summaries. Retained old outputs return RESULT_EXPIRED while diagnosis
preserves actual success metadata; new run outputs still work. Missing keys after
rollback return ACCEPTANCE_UNKNOWN. Recovery never creates replacement business
requests, grants or success results.

Retry verifies original/stage digests and recovery generation, continues the same
journal and preserves quarantine. It recognizes replacement already completed and
the original-renamed/replacement-pending Windows failure state. Ambiguous or changed
files refuse with the marker intact. Pending recovery refuses Host opening before
fresh SQLite creation. Completed receipt is synced before removing the marker.
A malformed/partially written journal fails closed and requires manual preservation
and recovery review; it is never deleted to force startup. Snapshot rollback can
lose post-backup metadata; it cannot prove whether those effects happened.

The implementation follows the primary [SQLite backup API](https://www.sqlite.org/backup.html)
and [Windows ReplaceFile failure states](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew).

## Verification

Core tests cover corrupt-original preservation, prepared/replaced recovery
interruption/retry, tampered stage/receipt rejection, pending marker refusal and schema/data
contracts. Actual Native Host/compiled CLI fixture is
`apps/runtime/test/recovery-fixture.ts`: successful private diagnostic canary,
online refusal, explicit approval, invalid identity, actual Windows no-delete file
lock causing failed replacement, retry of that same generation, revoked grants,
disabled cold start, old result expiry, unknown key and restored Todo revision/data.
Actual crash/revoke diagnostic coverage remains in jobs-fixture.mjs.

Native UI uses `node apps/ui-test/scripts/validate-managed-hosts.ts` after building
the dedicated config, preflight identity and fresh private test database. It checks
initial launcher, shared real runs/data, actual metadata-only diagnostic and routing.
Native consent clicks remain unverified: fixture grants are explicit CLI T0 setup,
not confirmation evidence. NVDA, other platforms, signed distribution, same-account
compromise protection and independent security review remain pending.

2026-10-06: all eleven uncached root test and forced build tasks passed in
sequence; all eleven lint/type tasks passed with zero lint warnings. Core has 38
tests; Native Host has seven, including aborted-peer ownership and real handshake.
Desktop has 84 Bun and 16 + 3 native tests. The actual concurrent 50-launcher gate
passed without removing assertions or changing frame/unit budgets. Web/Desktop
production output and opt-in/canary rebuilds were identical; native clippy passed.

The dedicated Native harness passed initial launcher/keyboard, GUI/CLI shared
revision/data/jobs, actual metadata diagnosis, actual JSON download with parsed
content equal to the UI object, STORE_BUSY online, matching CLI/GUI offline
backup UUID/schema and selection, manager refusal, and real background completion
after GUI exit. Screenshot inspected: [run diagnosis](assets/g3-desktop-run-diagnostic.png).
The option check waits for DOM attachment of the exact returned UUID: a collapsed
native select does not make its options visible. Recovery controls correctly
remain disabled until their prerequisites exist; the harness waits for the relevant
diagnostic action rather than requiring every button to be enabled.
The final Desktop status text fix clears an earlier busy error after a successful
backup list, and the actual Native harness verifies that message.

No risk or maturity label is advanced by passing these checks. Independent security
review, actual Native consent clicks, NVDA and other platform acceptance remain pending.
