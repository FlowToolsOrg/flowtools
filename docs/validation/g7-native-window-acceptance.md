# G7 controlled native window acceptance

Date: 2026-10-19. Windows/x64, product source commit `06e29cd`.
P3.1a/P3.1/G7 remain `pending` / `prototype`.

## Scope and prerequisites

The dedicated Debug build uses `tauri.shell-validation.conf.json`, identifier
`com.flowtools.shell-validation-20261015`, and visible title
`FlowTools shell validation - test data only`. Preflight must confirm both before
launch. A previously built `desktop.exe` returned the default identity during
preflight, so it was rebuilt before this acceptance; it was never launched against
the default profile. Debug/fixture autostart remains denied.

The manual positive-control helper is
`apps/desktop/test/fixtures/shortcut-owner.cs`. It registers only the temporary
Ctrl+Shift+Space combination against its own window, displays registration and
WM_HOTKEY counts, and unregisters on explicit release or close. It has no file,
network, command-dispatch or other-application automation logic. It is not a
packaged tool, Runtime fixture, plugin entry or CI replacement. The helper uses
the documented Windows [RegisterHotKey contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey).

Build from repository root, keeping generated files in ignored evidence storage:

```powershell
bun run --cwd apps/desktop tauri build --debug --no-bundle --config tauri.shell-validation.conf.json
if ($LASTEXITCODE -ne 0) { throw 'Shell fixture build failed' }
& .\apps\desktop\src-tauri\target\debug\desktop.exe --runtime-validation-identity

$g7Root = (Get-Location).Path
$g7Source = Join-Path $g7Root 'apps\desktop\test\fixtures\shortcut-owner.cs'
$g7Output = Join-Path $g7Root 'execution-validation\g7-shell\shortcut-owner.exe'
$g7Compiler = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
New-Item -ItemType Directory -Force (Split-Path $g7Output) | Out-Null
& $g7Compiler /nologo /target:winexe /reference:System.Windows.Forms.dll "/out:$g7Output" $g7Source
if ($LASTEXITCODE -ne 0) { throw 'Shortcut owner build failed' }
```

Use absolute Windows paths for the legacy C# compiler. This requires the local
.NET Framework compiler and Windows Forms; it is an explicitly launched manual
fixture, not an added workspace task or automatic CI dependency.

## Procedure and observed results

1. Launch the helper first and require `Registered: True`, `Received: 0`.
   Stop if another process already owns the key. Launch the identity-checked
   Desktop fixture: the main window must still appear despite the real conflict.
2. Open settings and choose dark. Observe the selected state and saved message.
   Alt+F4 hides the main window while preserving its PID/start time. From the
   helper, press Ctrl+Shift+Space: its received count must increase while the
   Desktop main window remains hidden.
3. Start a second Desktop with fixed synthetic `--run` / `--input` arguments.
   It must exit 0 and restore the original window/PID. Settings and the dark
   selection remain. Single-instance args/cwd remain ignored by the native
   callback; this does not exercise or authorize a business command.
4. Release the helper key and require `Registered: False`. Stop only the recorded
   test Desktop process after checking its executable path and start time, then
   restart the identical binary. The new native process must display a dark
   launcher before opening settings; settings must retain dark as the selection.
   This is a forced Desktop-process restart, not an OS reboot or power-loss test.
5. Hide that restarted main window. Press Ctrl+Shift+Space from the released
   helper. Desktop must restore the same restarted PID/window; the helper count
   must remain unchanged. This validates recovery after release **and Desktop
   restart**; automatic re-registration without restart was not implemented or tested.
6. Release/close the helper and stop only the recorded Desktop fixture. Verify
   both owned processes are gone. No Host stop or business execution was requested.

All six steps passed. Interactive Windows inputs used Computer Use; bounded CLI
actions checked identity and managed only the recorded test processes. PID/start-time
checks distinguished retained windows from new processes. The initial and
restarted native window handles differed; the second-instance and shortcut
restorations each retained the expected handle/process. Native screenshots showed
the saved dark selection after restart, with no settings page opened beforehand.

Local evidence is under ignored `execution-validation/g7-shell/`:

- `native-build-20261019.log`, `native-identity-20261019.txt` and
  `native-artifact-20261019.json` record the exact build/identity/hash.
- `native-process-20261019.json`, `native-hidden-20261019.json`,
  `native-second-instance-20261019.json`, `native-restarted-process-20261019.json`
  and `native-{before,after}-hotkey-20261019.json` record process observations.
- `20261019-shortcut-conflict.png`, `20261019-second-instance-restored.png`,
  `20261019-native-restart-launcher.png`, `20261019-native-restart-preference.png`
  and `20261019-hotkey-restored.png` contain only the task-owned fixture UI.

## Current automated gate

Clean commit `06e29cd` passed the full
`pwsh -NoProfile -File scripts/check-ci.ps1` on this machine with exit 0 and a clean
final worktree: 11 workspace lint/type/test/build gates, 80 Chromium tests, Core
222 tests plus two doctests, Runtime 24 native plus 16 integration tests, Desktop
92 Bun / 28 Rust unit / four integration tests. Uncached tests took 7m2.669s;
sequential forced builds took 1m4.001s. Web 22 / Desktop 24 production artifacts
were byte-identical under opt-in/canaries; Rust fmt/check/all-target Clippy passed.
The native fixture build afterwards also left generated tracked files unchanged.

The same product commit's two remote Windows runs also passed:
[push run](https://github.com/FlowToolsOrg/flowtools/actions/runs/39000034587),
[PR run](https://github.com/FlowToolsOrg/flowtools/actions/runs/39000037009).
Local full-gate log/exit: `root-ci-20261019.log` / `.exit` in the evidence directory.
Earlier timeout/OOM observations remain in [shell history](./g7-desktop-shell.md);
this passing run does not identify their cause. No source fixture, deadline,
assertion, failure propagation or retry policy was changed to obtain it.

## Launcher declaration wording follow-up

The native screenshots above exposed the legacy `安全沙箱` chip for commands
without `requiresNative`. The follow-up changes that chip to `未声明原生需求`
or `声明需原生支持`, with a tool icon, and describes the permission page as
`查看能力声明与当前限制`. These labels report metadata only; they do not represent
isolation, user grants or a capability approval. The earlier native screenshots
retain the exact UI accepted at product commit `06e29cd`.

The changed production preview passed pinned Chromium checks at 1280x900 and
390x844: declaration text, keyboard navigation to permissions, the existing
limitations notice and no horizontal document overflow. Screenshots and observed
DOM state are in `20261019-{launcher,permissions}-declarations-*.png` and
`launcher-declarations-20261019.json` under the ignored evidence directory.
The browser preview lacks native inventory IPC and shows its connection error;
these checks do not replace the separate native window acceptance above.

## Remaining acceptance

The available UI control surface did not expose taskbar/tray windows, so tray
menu clicks, its status text and its exit action remain unverified. OS login/
reboot autostart, monitor hotplug, Explorer restart, private-profile/production
identity migration and accepted-job continuity require separate controlled
acceptance. Diagnostic export and authoritative Runtime status remain future
P3.1a work. No G5/G6 execution gate is opened. Product commands, permissions and
trust boundaries are unchanged; independent security review remains pending.
