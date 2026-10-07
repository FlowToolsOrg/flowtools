# G3 Runtime stop receipt regression

2026-10-07, Windows/current-user/fixed-T1 prototype. This is engineering
verification by Codex, not independent security approval.

## Failure and repair

The push [Windows run](https://github.com/FlowToolsOrg/flowtools/actions/runs/37466625820)
failed in the real CLI suite's `afterAll`: `runtime stop` returned
`RUNTIME_DISCONNECTED` with exit 1. Its eight business assertions passed. The
[PR run at the same revision](https://github.com/FlowToolsOrg/flowtools/actions/runs/37466631329)
passed; that success did not invalidate the observed failure.

`Call::Stop` marked the core stopping before the connection wrote its response.
The shutdown tick could then abort that connection, losing the stopping receipt.
The server now claims a receipt barrier under the same core lock. It keeps the
connection alive until the authenticated manager consumes the receipt and closes,
or the bounded write/close wait fails. A disappeared manager releases the barrier.
The write budget remains five seconds; a non-closing manager has a separate
five-second close bound. Process drain remains bounded independently.

Accepting an authorized stop now cancels queued and running work immediately,
before transport waiting. Queued work cannot launch; running Todo commits fail
with `ABORTED`. The later shutdown/drain is idempotent. A temporary management Host
still does not cancel restored business work it does not own. No business retry,
disconnected-as-success rule, grant, endpoint or protocol change was added.

## Regression evidence

- A one-byte duplex transport deterministically blocks the real connection's
  receipt write. The test failed on the old shutdown predicate (`Receipt is still
blocked`), then passed with the barrier. It reads the complete authenticated
  stop response, verifies shutdown remains gated until client close, and separately
  drops the peer before reading to verify bounded release. Existing real Windows
  pipe ownership/handshake tests remain in place.
- A real core/broker Todo fixture first attempts an unauthorized business-channel
  stop and verifies work remains active. Authorized T0 stop must cancel queued
  work and reject active mutations before receipt delivery, leaving revision zero.
  It failed before immediate cancellation (`Queued` versus `Cancelled`), then
  passed after the repair. The same fixture injects an actual SQLite trigger
  failure while saving cancellation. It reproduced `check_run = Ok` after stop;
  the stopping flag now independently denies launches and effect commits even
  when cancellation cannot be persisted. Durable metadata is not fabricated as
  cancelled on that failure; the Todo revision remains zero.
- All 39 core and eight native Host Rust tests passed. The eight real compiled
  CLI tests and their strict stop hook passed; the full repository gate and fresh
  remote runs are recorded separately in the PR.

## Security scope

SEC-003/006/009/010/013/015 remain open. ADR-0001/0002 are unchanged. Only the
existing authenticated T0 manager can receive `Outcome::Stopping`; the barrier
cannot authorize a stop, trust payload identity, or grant effects. Cancellation
precedes acknowledgement waiting and adapters retain the Host lock and fresh
session/epoch checks at their effect commit. No native command, Tauri permission,
CSP, remote URL, file/network/data scope, package entry or maturity label changes.
OS termination and crashes can still lose a reply and require explicit state
reread. Windows/T1 tests are not third-party isolation or production approval.
Independent reviewer/date/conclusion remains pending.
