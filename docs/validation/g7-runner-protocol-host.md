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
