# G3 terminal verification

2026-10-07. Scope: Windows/current-user/fixed-T1 prototype. Code review here was
performed by Codex; it is not an independent security approval or production gate.

## Repairs and executed checks

- [Stop receipt/cancellation repair](g3-runtime-stop-receipt.md): a real remote
  failure led to two deterministic regressions, each failing before its repair.
  Host now cancels business work before waiting for the authenticated stop receipt
  to be consumed. All 39 core and eight Host Rust tests passed, and clippy passed.
- Desktop's main window and both isolated validation configurations now enable
  Tauri `zoomHotkeysEnabled`. The installed Tauri/Wry default was false, disabling
  user zoom shortcuts. The configuration/identity contract and dedicated native
  compilation passed. Actual 200% keyboard zoom/reflow remains unverified.
- Rebuilt Runtime and the dedicated compiled identity, then ran the existing
  `node apps/ui-test/scripts/validate-managed-hosts.ts` against a fresh private
  profile. Initial launcher, keyboard Todo Add, shared revision 4, preservation
  of unknown Todo fields, shared job inventory, actual diagnostic JSON download,
  online `STORE_BUSY`, matching offline schema-2 backup and business-channel
  `SESSION_INVALID` all passed. A real background run completed after GUI exit.
  Diagnostic screenshot was inspected; raw receipt/screenshots remain ignored.
- The eight real CLI execution contracts and strict stop hook passed. The full
  clean Windows quality script and current-head remote results are recorded in
  the PR; older green runs do not prove the current revision.

The Native harness supplies explicit T0 fixture policy. It does not click native
consent and does not certify a complete keyboard or screen-reader interaction.
The earlier actual native confirmation/cancellation and recovery-failure/retry
checks are recorded in [Desktop acceptance fixes](g3-desktop-acceptance-fixes.md).

## Security engineering review

Review baseline: `c8b692b` and the window zoom configuration in this change.
Reviewer: Codex, 2026-10-07. Conclusion: the scoped stop repair preserves the
examined authorization/data boundaries; the findings above have regressions.
Independent reviewer/date/conclusion: pending, no approval claimed.

| Boundary                                | Source and rejection evidence                                                                                                                                                                                                                                                                                                                                                       | Limit                                                                                       |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Host identity and management separation | [Core dispatch](../../packages/runtime-core/src/runtime.rs) binds caller to the authenticated connection; [native bridge](../../apps/desktop/src-tauri/src/managed_runtime.rs) checks Debug/main/dev origin and fixed build-owned binary. Real Native business-channel management refusal passed.                                                                                   | Current-user T0/T1 prototype; same-account compromise is outside the claimed boundary.      |
| Effects, scope and revocation           | [Broker](../../packages/runtime-core/src/broker.rs) requires manifest AND grant scopes, exact identity/package, epoch, expiry and per-run call budget. Existing tests reject identity/SQL/path/executable injection, over-scope calls and stale sessions. [Policy](../../packages/runtime-core/src/policy.rs) validates before transactional import and revokes on catalog changes. | File/tool descriptors are not IO adapters; origin matching is not DNS/redirect enforcement. |
| Stop and effect commit                  | [Core](../../packages/runtime-core/src/runtime.rs) cancels before the receipt; Todo commit runs fresh checks inside the Host lock. New tests reject an unauthorized stop without cancellation, prevent queued launch and leave data revision zero after authorized stop. [Server](../../apps/runtime/src/server.rs) bounds receipt/peer waiting and worker drain.                   | Crashes/OS termination can lose receipts; explicit reread remains necessary.                |
| Private payload and diagnosis           | [Private jobs](../../packages/runtime-core/src/private_jobs.rs) use Windows DPAPI with run/kind binding and digest checks; [profile](../../apps/runtime/src/profile.rs) verifies the current-user private ACL. Core diagnosis explicitly selects metadata fields. Real JSON export equals the displayed metadata and excludes fixture inputs/results.                               | ACL/DPAPI do not isolate an attacker already controlling the same account.                  |
| Data, single writer and recovery        | [Data store](../../packages/runtime-core/src/data.rs) uses Host-derived namespace, parameterized SQL, revisions, quotas and transactional writes; [recovery](../../packages/runtime-core/src/recovery.rs) validates UUID/digest/journal, preserves originals, revokes grants, disables cold start and prevents replay. Existing real lock/failure/retry fixtures remain required.   | Backup/restore is same-profile and Windows only; unknown side effects require review.       |
| External entrypoints                    | Existing service-level default-denial/build-mode tests and actual artifact scans remain required in full CI. This change adds no native command, Tauri permission, CSP, remote URL, file/network/data scope, executable selector or third-party entry.                                                                                                                              | Signed distribution and T2/T3/TL isolation remain later milestones.                         |

ADR-0001/0002 and SEC-002/003/004/006/009/010/013/015 remain open. This review adds
evidence, without accepting production risk or advancing maturity.

## Outstanding acceptance

Complete Tab/Shift+Tab navigation, native-dialog focus return, actual 200% zoom
and NVDA are still pending. The official NVDA 2026.2 portable download passed
Authenticode validation, but after the interrupted session Computer Use's callable
entrypoint was absent. Neither NVDA utterances nor actual zoom were then observed;
installation preparation, screenshots and UIA names are not screen-reader proof.

An actual independent reviewer must record their name, date, reviewed revision,
scope and conclusion. CI and Codex self-review cannot supply that approval. PR
remains Draft while required acceptance is incomplete. Other platforms, signed
distribution and production readiness are not part of this Windows/T1 result.
The maintainer's local checklist and checkmarks remain ignored and unmodified.
