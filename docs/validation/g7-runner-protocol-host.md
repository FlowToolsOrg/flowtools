# Windows runner protocol fixture Host

2026-10-28. The complete gate on `6016f26` passed static/contract checks and
earlier workspace tests, then failed the real truncated/oversized/forged frame
test's unchanged 30-second bound. Bun killed the dangling child; its code 143
correctly failed the required code-1 assertion. This is a failure, not acceptance.
See `execution-validation/g7-shell/search-root-ci-20261028.log`.

Diagnostics separated runner behavior from parent startup. An actual Node Host
sent the same truncated/oversized bytes to the same compiled Bun runner; both
refused with code 1, empty stdout and RUNNER_FAILED in 205/143ms. An empty process
through Bun.spawn blocked its parent for 9,596ms before the spawn returned;
stdin end added less than 1ms and the child then exited. Repeated parent startups
can consume the protocol test budget without testing more protocol behavior.
This identifies the measured parent startup problem, not its OS/engine internals.

The fixture now batches the exact original frames in one real Node Host. Each
frame still starts a fresh, actual compiled Bun runner with piped stdin/stdout/
stderr. The matching service-pin positive control still imports its real marker;
the old pin must refuse before importing. No handler, package, byte sequence,
rejection assertion, production reader, runner deadline, Job/drain/lease logic
or test's 30-second bound is relaxed. Results also require one real PID per case,
exact case counts, no output-budget breach and no forced timeout. The Node Host
has a stricter 25-second aggregate watchdog and only kills its own active child.
The helper remains test-only and is not imported or bundled by runner production.

The `.mjs` helper is included in strict `allowJs`/`checkJs`/`noEmit` checks; this
prevents untyped fixture code and accidental JS source overwrites. Existing Node
20.19+ fixture support is used; no toolchain/CI/env/permission upgrade is added.
All four runner tests passed (54 assertions), followed by clean types/lint.
Log: `execution-validation/g7-shell/runner-protocol-host-20261028.log`, exit 0.
A new complete root gate is required. Older Runtime descendant readiness/drain
failures remain unmodified and cannot be replaced by this fixture result.

The integrated gate on `08e0f34` subsequently passed all four protocol tests and
stopped on three Core inventory tests (240 passed / 3 failed): operation count
11 vs 12, dependency package/root counts 12 vs 13, and native Manifest count 12
vs 13. A complete source search found the additional Manifest count assertion
behind the operation assertion as well. All fixed inventories now explicitly
include the notification operation and thirteenth real package; no validation
loop or refusal condition is removed. This is recorded in
`runner-host-root-ci-20261028.log`; the corrected tree needs the full gate.
The corrected complete Core library suite then passed all 243 tests in 47.94s;
see `reminder-core-inventory-20261028.log` (exit 0). Other workspace/full gates
are still separate evidence.

The scheduling fixture now locates its actual executable through
`CARGO_TARGET_DIR` (falling back to this checkout's target directory), matching
the established Windows CI cache contract. It does not load a stale/default
target when an isolated checkout uses the configured native artifact directory.
The actual scheduling/reminder wrapper also validates its two known output lines
and requires the native notification-attempt record, so a missing-capability
fallback cannot masquerade as native adapter coverage. Rebuilding and running
that test with the alternate Desktop Cargo target passed (1 test / 4 assertions,
21.14s); see `scheduling-alt-target-20261028.log`, exit 0.

2026-10-28 native drain fixture Host: `bf5f20a` passed the new CLI Node Host
cases and Core 252 + 11 + 2, then retained the original Runtime failure (19/24;
deadline, malformed, oversized, provider-error and root-cancel readiness/drain).
The C provider had not produced its actual descendant marker before the old
deadline. Independent observations of Bun's parent spawn call taking ~10s and
Node starting the same Bun binary in <1s isolate a test-Host startup obstruction;
the underlying OS/engine cause is still unknown.

Only the seven abnormal-drain fixtures now use a test-only Node Host for the
same compiled A/B/C handlers and unchanged Runtime service runner. The normal
eight service tests and production config still use Bun. The C handler checks
the build-pinned Bun binary's SHA-256, starts that actual Bun descendant, waits
for its OS spawn event and writes its PID. The test never creates/assigns a
replacement descendant itself. The Runtime still observes that living PID in
the exact owned OS Job before the barrier, proves it exits after drain and
retains leases/migration refusal until then. All existing 5s readiness/barrier,
9.5s run and 10s drain-entry budgets, failure modes, expected error codes and
real-process assertions remain unchanged; there is no skip/retry or product
Node runtime option. The fixture is a neutral process-containment test, not
evidence that Bun's own child-startup problem has been repaired.

All fifteen actual native service tests passed in 47.89s, including the normal
Bun chains, six abnormal failure/lease cases and timeout descendant cleanup.
Log: `execution-validation/g7-shell/native-drain-node-host-20261028.log`, exit 0.
The freshly built fixture manifest pins the actual universal compiled artifact;
Node is selected/hash-verified only by Rust unit-test setup. No production
inventory, loader, Tauri permission, Job implementation or policy code changed.
The repaired tree still requires the complete root gate.
