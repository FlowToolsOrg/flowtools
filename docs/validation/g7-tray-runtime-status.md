# G7 P3.1a5 native tray Runtime snapshot

Date: 2026-10-25. Windows source Desktop prototype.
P3.1a/P3.1/G7 and independent security acceptance remain pending.

## Scope and behavior

The native tray retains its separate Desktop residency line and adds a disabled
Runtime snapshot/time pair plus an explicit `查询 Runtime 状态 · Prototype`
action. Construction, opening the menu, window restore and Desktop exit never
query or start Runtime. There is no timer, polling, subscription or retry.

`apps/desktop/src-tauri/src/shell/runtime_status.rs` owns the native menu items
and an ephemeral atomic query gate. An explicit request clears the old snapshot
before opening any connection; overlapping requests are ignored. Its owned
lease resets the gate when the future completes, errors or is cancelled. The
query action stays available so a native menu enable/disable failure cannot
strand it. If initial menu updates fail, no query is dispatched. Menu failures
use a fixed recovery message/log code without copying native exception text.

`managed_runtime::status_for_main_window` resolves only the actual main webview,
then reuses the existing Debug/main/configured-origin/profile guard and exact
`status_snapshot` implementation. It has no initialization argument, alternate
profile selector or relaxed packaged-origin behavior. It uses Desktop bootstrap
with cold start disabled, authenticates an owned short-lived pipe, checks the
handshake/status instance and drops that connection. There is no ensure-runtime,
manager fallback, grant, database initialization, execution or stop operation.

Success shows only a known mode label and actual active/recorded job counts.
Unknown modes use a fixed fallback; impossible active > recorded counts refuse
success. Failure shows an unconfirmed state and only the existing public stable
error code, without declaring a definite offline state. Receipt time uses the
existing Jiff dependency and an explicit local UTC offset; clock-format failure
has a fixed unavailable-time message. Every completed result states that it does
not refresh automatically. Instance IDs, tasks, payloads, credentials, paths and
raw errors are not copied into the menu, and no snapshot is persisted.

There is no new IPC command, Rust wire DTO, permission, dependency or plugin
capability. Core menu/tray access remains withheld from webviews. The codegen-only
`mock_shell_status` is a direct T0 Rust test adapter, never an invoke handler;
ordinary native builds do not compile it. Autostart and exit behavior are unchanged.

## Focused verification

All 35 Desktop Rust unit tests and six binding integration tests passed. Five
new unit regressions cover overlapping requests, cancellation releasing the
owned gate, bounded mode/count projection, stable failure metadata without an
offline claim or previous counts, and impossible-count refusal. The actual
MockRuntime native-tray adapter refuses a missing main window, foreign origin,
packaged origin and absent configured development origin. Existing actual Host
tests still prove read-only status, no missing-profile initialization, no cold
start after Host exit, instance agreement and unchanged bootstrap/SQLite bytes.
Existing real IPC regressions continue to refuse webview menu/tray creation and
startup/shortcut control.

The first window-test placement in the unit-test binary failed startup with
`STATUS_ENTRYPOINT_NOT_FOUND`; moving those tests into the existing `bindings`
target with its required Common Controls v6 manifest resolved it. Build manifest
directives, production permissions and test assertions were not weakened.
Native tests are not skipped or retried by the quality gate.

Ignored focused log/exit: `execution-validation/g7-shell/tray-status-rust-20261025`
and its `.log` / `.exit` files. The pre-implementation compiler refusal is in
`tray-status-red-20261025.log` / `.exit`.

## Existing integration boundary

The preceding source head `0e1f2b6` passed both remote Windows runs:
[push](https://github.com/FlowToolsOrg/flowtools/actions/runs/40195190133) and
[PR](https://github.com/FlowToolsOrg/flowtools/actions/runs/40195193965).
The earlier local full Runtime 22/24 failure remains recorded in
[diagnostic evidence](./g7-runtime-diagnostic.md).

A single unchanged diagnostic invocation of the original root-cancel native
test on 2026-10-25 again failed its original descendant readiness in 5.14 seconds.
The old independent Rust creation probe already used the no-console flag, so
that flag is not an established fix. Runtime source, fixture bytes, original
five-second readiness, 9.5-second execution deadline and all real process/Job/
drain/lease assertions remain untouched. No full-gate retry or OS setting change
was used to reinterpret the old failure. Exact baseline log/exit:
`runtime-baseline-20261025.log` / `.exit` in the same ignored directory.

## Full gate on this product source

The clean product commit `ca062a4` passed all eleven workspace lint/type gates,
all 94 Chromium tests in 29 files and Core 222 unit tests plus two doctests.
The unchanged Runtime native suite failed four of 24 tests:

- `provider_error_retains_lease_until_its_real_descendant_drains`;
- `oversized_provider_frame_retains_lease_until_its_real_group_drains`;
- `real_job_access_denial_stops_host_and_refuses_provider_migration`;
- `root_cancel_retains_lease_until_the_actual_provider_group_drains`.

The first three could not find the real descendant PID marker before their
drain boundary expired; root-cancel reached its original readiness deadline.
Cause remains unestablished. Test exit 101 terminated the script with exit 1
before Desktop/build/post-build stages. The worktree stayed clean; no failure
retry, source-fixture change, deadline extension or assertion relaxation was
applied. Latest local full integration is not accepted. Log/exit:
`root-ci-tray-status-20261025.log` / `.exit` in the ignored evidence directory.

Separate full Desktop tests on the same product source passed 99 Bun, 35 Rust
unit and six binding integration tests. Subsequent sequential forced builds
passed all eleven workspaces (1m11.367s); Web 22 / Desktop 24 production artifacts
were byte-identical under opt-in/canaries. Rust formatting, locked check and
all-target codegen Clippy with warnings fatal passed. These focused checks do
not replace the root failure. Logs/exits:
`tray-status-desktop-20261025`, `tray-status-build-all-20261025`,
`tray-status-production-20261025`, `tray-status-fmt-20261025`,
`tray-status-check-20261025` and `tray-status-clippy-20261025`.

## Controlled native construction and recovery smoke

The dedicated shell fixture was rebuilt after these checks. Its metadata-only
`--runtime-validation-identity` output was verified before launch: marker
`flowtools-runtime-validation-preflight-v1`, identifier
`com.flowtools.shell-validation-20261015`, configured origin
`http://127.0.0.1:1420/` and the test-data-only window title. The source commit and
artifact hash were recorded. Computer Use controlled only the returned fixture
window; it did not expose a taskbar/tray window for menu acceptance.

Closing the main window retained the recorded process and made the old window
unavailable for capture. A second launch restored the same window, PID and
process start time. This exercises native tray/menu construction and the
existing close, residency and second-instance recovery paths.

Settings initially showed an unqueried status and an unconnected Runtime panel.
One explicit read-only status query returned the expected `SESSION_INVALID`
packaged-origin refusal, without stale counts; the separate Runtime panel
remained unconnected. No Runtime start, initialization, grant, task or OS startup
action was requested. The owned fixture process was then stopped only after its
exact executable path and recorded start time matched; exit was confirmed.

Ignored evidence: `tray-status-native-build-20261025.log` / `.exit`,
`tray-status-native-artifact-20261025.json`,
`tray-status-native-identity-20261025.txt`,
`tray-status-native-process-20261025.json`,
`tray-status-native-hidden-20261025.json`,
`tray-status-native-restored-20261025.json`,
`tray-status-native-ui-20261025.txt`,
`20261025-tray-status-native-refusal.png` and
`tray-status-native-result-20261025.json` under `execution-validation/g7-shell/`.
This smoke does not validate actual tray clicks, native layout or successful
Runtime snapshot display.

## Remaining acceptance

Actual tray clicks, native menu layout and observed/error snapshot rendering
remain manual gates. The available window-control surface previously did not
expose taskbar/tray windows. Dedicated shell identity preflight remains required
before native launch. Packaged-release status, OS login/reboot, Explorer/monitor
recovery, accepted-job continuity, complete diagnostics, G5/G6 execution and
independent security approval remain pending. This prototype adds a bounded
query surface; it does not close G7 or certify runtime isolation.
