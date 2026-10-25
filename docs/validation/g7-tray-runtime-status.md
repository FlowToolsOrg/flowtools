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

## Remaining acceptance

Actual tray clicks, native menu layout and observed/error snapshot rendering
remain manual gates. The available window-control surface previously did not
expose taskbar/tray windows. Dedicated shell identity preflight remains required
before native launch. Packaged-release status, OS login/reboot, Explorer/monitor
recovery, accepted-job continuity, complete diagnostics, G5/G6 execution and
independent security approval remain pending. This prototype adds a bounded
query surface; it does not close G7 or certify runtime isolation.
