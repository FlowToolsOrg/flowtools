# G7 P3.1a1 Windows desktop entry prototype

Date: 2026-10-15. First independently reviewable part of P3.1a.
G7/P3.1/P3.1a remain pending; this is a `prototype`.

## Behavior

- Official single-instance plugin registers before other plugins and database
  setup. Second-instance arguments/cwd only wake the existing window; they never
  become commands, paths, URLs or grants.
- Native tray shows Desktop residency, shortcut availability and OS autostart
  state. Close hides the main window only with a working tray and successful
  hide. Missing tray/failure retains ordinary close. Exit affects Desktop only.
- Ctrl+Shift+Space restores on press. Conflict/plugin failure preserves window
  and tray recovery and reports unavailable shortcut. Restore unminimizes,
  shows/focuses and moves an unreachable titlebar into an available work area.
- Autostart remains unchanged until an explicit native enable/disable action.
  Requests are idempotent; success requires OS readback. Errors report unknown
  state and allow retry. Debug and fixture identities deny OS startup access.
  Release uses the current identifier, fixed FlowTools startup name and fixed
  `--background` argument. Background launch hides only after a tray exists.
- No Runtime store, scheduler, grant or stop call. Desktop residency does not
  claim Runtime connectivity, accepted jobs or cold-start authorization.

## Verification

Seven Rust policy regressions cover tray/other-window close, debug/fixture
startup denial with zero OS access, idempotent requests and failed write/readback,
background launch without tray, removed monitors, negative/extreme coordinates
and unreachable titlebars. Desktop tests freeze the fixture identity and absence
of webview autostart/shortcut permissions.

2026-10-15 results so far: `bun run test:desktop-shell` passed (one Bun test,
seven Rust policy tests and one real Tauri IPC test covering seven denied calls).
Dedicated-identity Debug native build passed. Real Windows
acceptance confirmed initial launcher render, Alt+F4 hides while the same process
survives, Ctrl+Shift+Space from a test-only GUI restores it, and a second instance
with business-like arguments exits 0 and restores the same first instance.
No Runtime/business command, system startup change or user profile was used.
Native tray menu interaction, conflict fixture and OS restart remain unverified.
The final fixture also verified the updated launcher/settings shortcut text and
restoration from a minimized window. Fixture-only screenshots are retained in
ignored `execution-validation/g7-shell/launcher-final.png` and `settings-final.png`.

The authoritative `scripts/check-ci.ps1` run on `2e6cdf1` passed documentation,
catalog, generated contracts, all 11 workspace lint/type gates and 72 browser
tests, but failed in the Runtime native test
`server::service_tests::root_cancel_retains_lease_until_the_actual_provider_group_drains`:
`service_tests.rs:611`, C fixture descendant readiness timeout (5 seconds).
The exact isolated test reproduced the same failure. Runtime, runner, SDK and
core sources are unchanged from main; the cause has not been established.
The full root gate did not reach build/production-artifact/final-clean stages.
This failure is retained, not waived. Root/core tests included 222 passing unit
tests and two passing doctests. Desktop tests are run separately below; neither
focused success nor a Draft PR establishes a passing full gate.

Separate follow-up checks passed: full Desktop tests (92 Bun, 28 Rust unit and
four Rust integration tests), all 11 workspace builds (`bun run build --force`),
and production external-entrypoint checks (22 Web/24 Desktop byte-identical
artifacts under child-only unsafe opt-in/canaries). Final all-target codegen
Clippy passed after replacing a linted error-extraction expression in the IPC
test. The final focused shell regression was rerun after that test-only edit.
The production refusal scan does not establish package signing or isolation.

Next P3.1a substeps: resolve the full-gate failure, verify actual tray/menu and
shortcut-conflict recovery, integrate persisted desktop preferences/diagnostics
and authoritative Runtime status, then controlled restart/multi-screen/background
job acceptance. P3.1b and file/media execution cannot bypass their dependencies.

A real Tauri MockRuntime IPC regression (in the existing Common Controls v6
integration-test target) uses the generated app context to reject
menu/tray creation and tray mutation, startup enable/disable and shortcut
registration before handler/payload processing. `core:default` previously also
granted menu/tray control; the capability now lists its other seven core defaults
explicitly, withholding menu/tray to prevent spoofed native actions or removal
of the recovery tray. Existing frontend code uses neither API.

Run the focused gate with `bun run test:desktop-shell`. APIs follow official
[tray](https://v2.tauri.app/learn/system-tray/),
[single-instance](https://v2.tauri.app/plugin/single-instance/),
[shortcut](https://v2.tauri.app/plugin/global-shortcut/) and
[autostart](https://v2.tauri.app/plugin/autostart/) documentation.

Isolated native acceptance build from repository root:

```powershell
bun run --cwd apps/desktop tauri build --debug --no-bundle --config tauri.shell-validation.conf.json
```

Launch only that fixture, never the default Debug app against user data. Check
initial rendering, close-to-tray, shortcut restore, minimize/restore, second
instance, native menu/keyboard status and explicit Desktop exit. Logs/screenshots
belong under ignored `execution-validation/`. Do not enable OS startup on this
development machine. Reboot/startup, actual monitor removal, Explorer restart and
accepted-job continuity require separate controlled Windows acceptance.

## Security and remaining gates

SEC-003/007/009/013 remain open; ADR-0001/0002 Host/Runtime boundaries remain.
New native APIs: tray/menu and official single-instance/global-shortcut/autostart
plugins inside Rust. Their invoke namespaces remain denied to webviews. No new
application IPC command, webview grant, CSP, remote URL, file/network/data scope
or plugin API. Single-instance IPC is a wakeup, never
authenticated execution. Startup creates no grants/cold-start permission;
disabling it affects future logins, not Runtime jobs. Exit never requests Host stop.

Independent security reviewer/date/conclusion: pending. Existing main-realm T1
and CSP gaps remain; official plugins are not an OS sandbox. Settings/diagnostics,
production identity migration, Runtime status integration, full P3.1a acceptance,
P3.1b scheduling, P3.2 search/UI and P3.4a office loops remain pending. Preserve
G5/G6 package/platform gates and external-code denial.

## 2026-10-15 continuation: descendant fixture startup

Test-only phase markers showed that A/B/C verify and read the start frames in
under a second on this machine. C entered its daemon implementation and then
blocked inside its synchronous `Bun.spawn()` call before writing the descendant
PID. A disposable process-only probe reproduced delayed spawn return without
Runtime, SQLite or an OS Job; stdin closure and inherited environment did not
eliminate it. The Bun internal cause has not been established.

The fixed fixture uses `node:child_process.spawn` with a hidden console and waits
for its successful `spawn` event before writing the PID. `detached: true` and
`unref()` remain, so the actual descendant survives provider failure until the
Host's Job drains. No production runner/protocol/Job code, grant, timeout or
assertion changed. Temporary diagnostic code and phase markers were removed.

Runner lint/types and all four protocol/actual-runner tests passed. Both exact
previously failing paths and the final 15-test native service suite passed,
including same-Job membership/aliveness, actual descendant exit, retained leases,
update refusal and real QUERY-only termination denial. The earlier failure above
is retained as historical evidence; the new full root gate is recorded separately.
