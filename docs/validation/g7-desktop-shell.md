# G7 P3.1a1 Windows desktop entry prototype

Date: 2026-10-15. First independently reviewable part of P3.1a.
G7/P3.1/P3.1a remain pending; this is a `prototype`.

Current integration status (2026-10-20): `1451c77` passed both remote Windows
runs. The later local full gate on status product commit `6432d76` failed three
unchanged Runtime descendant-readiness/drain paths (21/24), after 87 Chromium and
Core 222+2 passed. See [status evidence](./g7-runtime-status.md) for the exact
results; latest local integration is not accepted. Original Runtime/service
fixtures and deadlines remain unchanged from main; earlier process-creation and
Bun failure observations are retained without claiming a root cause.
Controlled native conflict/restart acceptance is recorded in
[native window acceptance](./g7-native-window-acceptance.md); OS/menu and other
P3.1a gates remain pending.

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

Next P3.1a substeps: verify actual tray/menu interaction, integrate diagnostics
and authoritative Runtime status, then controlled OS restart/multi-screen/background
job acceptance. Native shortcut-conflict recovery and appearance restart now have
controlled evidence; the earlier failures still lack an established cause.
P3.1b and file/media execution cannot bypass their dependencies.

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

## 2026-10-16 full Windows gate

Clean commit `8741845` passed `pwsh -NoProfile -File scripts/check-ci.ps1`
with exit code 0 and a clean final worktree. All 11 workspaces passed lint,
types, uncached tests (8m34.724s) and sequential forced builds (1m27.52s).
This includes 72 Chromium tests, 222 Core unit tests plus two doctests,
all 24 Runtime native tests and the 15 real service paths, and Desktop's
92 Bun / 28 Rust unit / four integration tests.
Web 22 / Desktop 24 production artifacts remained byte-identical under
child-only opt-in/canaries; Rust fmt, locked check and all-target codegen
Clippy passed. Logs and exit code are retained in ignored
`execution-validation/g7-shell/root-ci-resume.log` and `.exit`.

This resolves the previously recorded local full-gate failure. It does not
replace the remaining native manual/OS acceptance or independent security
review, advance G7 maturity, or open G5/G6 execution gates.

## 2026-10-16 follow-up: local process creation remains unstable

The next full gate on appearance commit `f669162` passed all 80 browser tests,
lint/types and Core tests but reproduced the root-cancel fixture's readiness
timeout (Runtime 23/24). Therefore the previous single green gate does not
establish that the asynchronous spawn change eliminated the local timing issue.
An eight-spawn disposable native-child probe also observed two initial creation
calls near four seconds followed by six below 12ms. This is not specific proof
of a Bun child startup defect; the Windows/internal cause remains unestablished.

Further temporary experiments did not establish a source fix: waiting for C's
running state before starting the five-second readiness timer failed the third
exact sample; using the original 9.5-second live-root deadline failed the second
sample with a real Timeout. A minimal native child, with and without a console,
also failed the original fixture budget. All those source experiments were
reverted, including the additional native artifact and build changes. The
previously committed asynchronous spawn change was also restored to main's
original fixture; it had not established a fix. Runtime/runner/Core/SDK sources
therefore remain unchanged from main.

A disposable Rust DLL called from the same Bun parent measured eight actual
`std::process::Command::spawn` calls for a fixed native sleep-only child, outside
Runtime, SQLite, runner frames and Jobs. Observed creation-call durations were
10039, 10026, 5015, 6, 8, 7, 9 and 12ms; every child was killed and reaped.
This demonstrates local process-creation instability across both JS and Rust
creation paths, but does not identify the OS/internal cause or certify remote CI.
Evidence sources are ignored `execution-validation/g7-shell/spawn-native-library.rs`
and `.mjs`; the observations are retained alongside them.

The failed full gate remains in `execution-validation/g7-shell/root-ci-appearance.log`
(exit 1). Original five-second readiness, 9.5-second execution deadline and all
actual descendant/Job/drain/lease/migration assertions remain unchanged. No tests
are skipped or retried by the gate. Latest local integration is not accepted as
passing; independent clean Windows/OS investigation remains required. Appearance
verification and follow-up build evidence are recorded separately in
[P3.1a2](./g7-desktop-preferences.md).

2026-10-17: a later Desktop-suite attempt also emitted Bun OOM and illegal-
instruction crashes in unchanged CI helper subprocesses, including
`bun -e 'process.exit(0)'`. System available memory subsequently rose from about
2.6GB to 12.8GB; three minimal Bun probes then passed, but the unchanged original
root-cancel test still failed readiness in 5.14s. These observations do not
identify the underlying cause. The current integration remains unaccepted;
there is no test-timeout increase, skip, failure retry or OS-setting change.

## 2026-10-19 current acceptance

`06e29cd` passed the full local Windows gate with exit 0 and a clean final
worktree, plus both remote Windows runs. Tests were uncached and builds sequential;
all original real-descendant/drain/lease assertions remain. This supersedes the
earlier _current status_, while preserving its failure evidence and unknown cause.
Dedicated native build and identity preflight also passed without tracked drift.

Real native UI acceptance confirmed a registered competing hotkey owner, recovery
through a second instance while the hotkey remained occupied, preference retention
after a forced Desktop-process restart, and shortcut recovery after release plus
Desktop restart. All test-owned processes were cleaned up. See
[the reproducible procedure and evidence](./g7-native-window-acceptance.md).
The tool did not expose taskbar/tray windows, so actual tray menu/status/exit
clicks remain unverified, along with OS restart/autostart, monitor hotplug,
accepted-job continuity and independent security acceptance.

## 2026-10-19 final follow-up: full gate failure retained

The full Windows gate on clean product commit `73a4a1d` passed all 11 workspace
lint/type gates, 80 Chromium tests and Core's 222 unit tests plus two doctests,
then failed in Runtime (21/24 native tests):

- `provider_error_retains_lease_until_its_real_descendant_drains`
- `oversized_provider_frame_retains_lease_until_its_real_group_drains`
- `root_cancel_retains_lease_until_the_actual_provider_group_drains`

The first two observed no descendant-ready PID file at the actual drain boundary
and then timed out waiting for that boundary; the third hit the existing five-second
readiness deadline. Test command exit 101 terminated the root script with exit 1.
It did not reach full build, production artifact, final Rust or clean-worktree gates.
After failure, a read-only worktree check was clean. The OS reported about 12.9 GiB
of available physical memory after failure; this does not identify a cause.

No failure retry, fixture/source change, timeout increase or assertion relaxation
was applied. The earlier passing `06e29cd` run remains valid evidence for that run
and does not establish stable local process creation. Full log/exit are ignored
`execution-validation/g7-shell/root-ci-final-20261019.log` and `.exit`.
Native conflict/restart acceptance above remains independent of this failed gate.

Separate follow-up checks on the unchanged product source passed: Desktop's
92 Bun / 28 Rust unit / four integration tests; all 11 forced workspace builds
(1m11.15s); and Web 22 / Desktop 24 byte-identical production-entrypoint artifacts
under opt-in/canaries. These are focused results, not a rerun or replacement of the
failed full gate. Logs are `desktop-final-20261019.log`, `build-final-20261019.log`
and `production-final-20261019.log` (plus their `.exit` files) in the same ignored
evidence directory.
Separate Rust fmt, locked check and all-target codegen Clippy also passed without
warnings; logs are `rust-{check,clippy}-final-20261019.log` and
`rust-final-20261019.exit`. They do not change the full-gate result.
