# G3 P1.4b shared Runtime clients

Scope: Windows x64, current-user private profile, fixed compiled T1 inventory.
Maturity remains prototype. Independent Security Reviewer/date/conclusion:
pending; SEC-002/003/004/006/009/010/013/015 and ADR-0001/0002 apply.

Runtime owns jobs, grants and SQLite. Rust-derived jobs.list excludes results,
returns at most 128 active-first/recent snapshots, and reports activeJobs.
Caller identity comes from native bootstrap/session, never selectors or input.
Local CLI and Desktop can cancel the same user's jobs. T0 manager and validation
cross-caller cancellations stay denied. Original keys remain caller-bound.

CLI run now uses Host; createPluginRunner remains the explicit SDK adapter API
for controlled library consumers. Parser and real command regressions stay in
that adapter; CLI receipt/terminal/JSON/text/exit behavior has an actual compiled
Node + native Host fixture at apps/runtime/test/job-control-fixture.mjs.
Lost-ACK and failed lookup contracts retain one key, no second submit and a
visible ACCEPTANCE_UNKNOWN. Existing actual durable/crash/revoke tests remain.

Native Desktop managed_runtime.rs pins a build-owned executable/hash, verifies
profile ACLs through a bounded fixed native bootstrap, and keeps credentials in
native memory. The business allowlist cannot reach management APIs. Native
Dialog confirmation owns initialization/grants/cold policy/full stop. Revocation
is immediate. DEV/main/configured-origin checks reject external windows/origins;
release distribution is intentionally denied until its packaging gate exists.
Desktop depends on the Runtime workspace so builds/tests cannot race its native
bootstrap artifact. Wire client 0.2.0 rejects old 0.1.0 at handshake; no DB schema
change is required. This is not a third-party sandbox or a signed release.

Settings shows the shared task inventory and active count. Todo hydrates and
watches real Runtime data, writes with exact revision, reports conflicts and
authorization failures, and keeps only an in-memory view. Native Todo disables
legacy store/storage; original data is preserved, without implicit migration.
Closing GUI releases native sessions; explicit background tasks belong to Host.

The actual cold-start regression reproduced waiting for descendant stdio EOF
after the helper exited. Node/Rust native adapters now bound the helper's first
line and exit rather than waiting for background Host pipe EOF. CLI job control
passes after correction; no business retry or global timeout was introduced.

Actual Native acceptance (2026-10-06) passed with
`node apps/ui-test/scripts/validate-managed-hosts.ts`: initial launcher, keyboard
Add, GUI/CLI same instance and jobs, bidirectional revision 4 data, retention of
unknown Todo fields, actual background receipt after GUI process exit and
SESSION_INVALID on business-channel policy calls. Listings contain no results
or fixture text. Screenshots were inspected: [Todo](assets/g3-desktop-shared-todo.png)
and [tasks](assets/g3-desktop-shared-jobs.png). Fixture policy approval came from
explicit T0 CLI setup; it is not evidence of clicking native consent.

The actual final background run was `3ed3208b-cf82-45e4-88ba-f6aa55bf2c33`
in the first complete fixture; the final extended fixture receipt is preserved
under ignored execution-validation/g3. All profiles are disposable. Windows
Known Folder APIs ignore APPDATA overrides, so Native metadata now has an
explicit FLOWTOOLS_DESKTOP_VALIDATION_DATA_ROOT guarded by Debug + exact compiled
runtime validation identity + mode + safe absolute fixture directory. Production
identity, missing mode and relative paths are rejected. No default user identity
is used; fixture data and old metadata are preserved.

A broken Todo transport releases its owned session. Explicit reread reconnects;
there is no automatic write retry. A controlled lost-write-ACK regression reads
the already committed revision from a new session and verifies exactly one
submission. Native keyboard/role checks use the real required label Todo\*.

All eleven uncached root test and forced build tasks passed in sequence, along
with zero-warning lint, types, generated contracts/catalog/workspace checks,
Web/Desktop production artifact byte-identity gates and Desktop Rust clippy.
Final local changes additionally rerun affected Desktop/plugin checks and builds.
The eight existing compiled CLI success/flags/describe/batch/text/exit assertions
now live in Runtime tests, with explicit grants and a fresh managed profile.
The SDK library adapter still tests actual built plugins with controlled context.

P1.4b implementation and automated/native shared-client scope are delivered.
Native consent clicking is still pending because Windows Computer Use is
unavailable. The PR stays Draft; no manual waiver, independent security approval,
NVDA, other platform, signed release or third-party isolation is claimed.
P1.5b is the next independently committed implementation item.
