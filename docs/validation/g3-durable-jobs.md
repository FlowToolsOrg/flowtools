# G3 P1.3b: granted execution and durable receipts

Implemented on 2026-10-05 for the fixed Windows T1 inventory, maturity prototype.
The managed Host runs the actual Base64 and Todo compiled implementations. Todo
receives only a Host-bound async data snapshot; staged CAS writes return to the
broker and are checked for declaration, scope, grant epoch, deadline and budget
under the Runtime lock before the SQLite transaction. Network and other effects
remain denied without a real adapter. Validation mode still denies Todo jobs.

Accepted metadata commits before the receipt, then queued/running/terminal state
and metadata-only events commit in order. Terminal state and result envelope
commit together. Caller plus idempotency key bind the package version/digest,
command, immutable dependency lock, canonical input, background flag and deadline.
Conflicts reject; `jobs.lookup` returns the original receipt for the authenticated
caller even after grant revocation. A storage failure with ambiguous acceptance
stops new work and returns ACCEPTANCE_UNKNOWN, preserving the original key.

Raw job input and output never enter jobs metadata, events or error messages.
They are separate current-user DPAPI protected files in the private profile,
bound to run UUID and input/output role by entropy. JSON, package, input/action
hash and output schemas are revalidated. See Microsoft's
[DPAPI protection contract](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata).
Input files are removed at completion, cancelled/failed outputs removed, orphan
files removed on startup and completed outputs expire after 24 hours. Metadata
and idempotency keys remain bounded at 1024 records; 128 active jobs, 256 private
files and 64 MiB private payload quota fail explicitly rather than overwriting.
Expiry cleanup occurs on startup, not by a background retention service.

Restart resumes explicitly background accepted/queued jobs only with current
package, grant and deadline. Running Base64 alone is proven pure/deterministic;
other running jobs become interrupted and require review. A crash between Todo
commit and completion is not exactly-once: the write may exist, but is never
replayed automatically. Management-only startup does not run business jobs.
Revocation checks before spawn, during execution and before CAS stop stale work.
Foreground disconnect cancels; explicitly granted background survives disconnect.
Windows unnamed Job Objects close with the Host and kill the owned runner tree;
this is lifetime containment, not an OS sandbox. See
[Windows job lifetime](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).

Evidence: core durable receipt/crash-after-write regression; DPAPI role/path/quota
regressions; actual runner Todo staging/schema and default refusal; actual Node
named-pipe fixture simulates lost ACK at the client, restarts decrypted results,
kills a running Todo Host, confirms interrupted/no replay, and revokes an active
Todo before write. Existing G2 pure-task and shared-data fixtures remain intact.
Run `bun run --cwd apps/runtime test` and `bun run --cwd packages/plugin-runner test`.
Full workspace gates are recorded in the PR. No visual product flow changes in
this step; independently installed CLI and GUI integration follow P1.4a/P1.4b.

SEC-002/003/004/006/007/009/010/013/015 and ADR-0001/0002 remain open production boundaries.
Same-account compromise is outside DPAPI/ACL protection. Signed installation,
third-party sandbox, cross-platform support and an actual independent security
Reviewer/date/conclusion remain pending. No maturity promotion is implied.

Local validation uses Turbo concurrency=1 after documented V8 commit-memory
failures, preserving all tasks. The existing SDK real subprocess cleanup test
keeps every original assertion in a Node fixture with a 30-second integration
budget; ordinary unit timeouts are unchanged. This is not a startup SLA. The
bootstrap CI repair is a separate focused commit and both remote Windows gates
passed. Final P1.3b validation remains separately recorded in the PR.

Final local acceptance: docs, lint (zero warnings), types, all 11 uncached workspace test tasks, all 11 forced build tasks, Rust format/clippy, generated Runtime/manifests/command-doc read-only checks and production artifact refusal passed. Core 32 tests, native Host 4 tests, actual Host client fixtures 4 tests and fixed runner 2 tests passed. Root test/build ran sequentially with local Turbo concurrency=1; prior resource failures remain in execution-validation logs. Independent security review and standalone/GUI/diagnostic G3 steps remain pending.
