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
Rust Clippy passed before the final ACL regression; the full root gate below
rechecks all targets. Dedicated-identity Debug native build passed. Real Windows
acceptance confirmed initial launcher render, Alt+F4 hides while the same process
survives, Ctrl+Shift+Space from a test-only GUI restores it, and a second instance
with business-like arguments exits 0 and restores the same first instance.
No Runtime/business command, system startup change or user profile was used.
Native tray menu interaction, conflict fixture and OS restart remain unverified.

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
plugins inside Rust. No new webview command/permission, CSP, remote URL,
file/network/data scope or plugin API. Single-instance IPC is a wakeup, never
authenticated execution. Startup creates no grants/cold-start permission;
disabling it affects future logins, not Runtime jobs. Exit never requests Host stop.

Independent security reviewer/date/conclusion: pending. Existing main-realm T1
and CSP gaps remain; official plugins are not an OS sandbox. Settings/diagnostics,
production identity migration, Runtime status integration, full P3.1a acceptance,
P3.1b scheduling, P3.2 search/UI and P3.4a office loops remain pending. Preserve
G5/G6 package/platform gates and external-code denial.
