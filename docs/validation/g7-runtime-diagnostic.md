# G7 P3.1a4 Runtime status diagnostic preview

Implementation/acceptance: 2026-10-21. Evidence/cleanup rechecked: 2026-10-25.
Desktop status-report prototype only.
P3.1a/P3.1/G7 and independent security approval remain pending.

## Scope and behavior

The existing Runtime settings card has two explicit actions: generate a JSON
preview, then request its download. Neither action queries or starts Runtime.
Generation reads only the native app/Tauri versions through the official core
app API and the current page's ephemeral observation. It reuses the existing
`core:app:default` permission; no IPC command, Rust-derived DTO, permission,
dependency, persistence, native file writer or caller-selected path was added.

`apps/desktop/src/settings/runtime-status-diagnostic.ts` projects an allowlist
into a local formatVersion-1 report: scope, prototype label, generation time,
app/Tauri versions and one observation. `not-queried` has no invented counts;
`observed` contains the query receipt time, Host instance, normalized mode and
recorded/active job counts; `unconfirmed` contains only its time and an existing
public stable error code. This report is not a Runtime wire DTO or authorization
receipt. Unknown object fields and `toJSON` are not serialized. Raw exceptions,
logs, task inputs/results, paths, credentials and grants are not selected.

Versions use the SDK's public canonical version validator with a 64-character
limit. Times must be exact UTC ISO timestamps; instance IDs are bounded to 128
ASCII identifier characters, counts to uint32 with active <= recorded. Unknown
modes become `unknown`; unknown codes, invalid metadata or reports exceeding
4096 UTF-8 bytes refuse with a fixed message. Generation failures leave query
controls usable. Querying invalidates both an existing preview and any pending
metadata read; late completion cannot revive the previous report.

`runtime-diagnostic-download.ts` uses a JSON Blob and a fixed suggested filename,
`flowtools-runtime-status.json`, after the separate download button. Its object
URL is revoked even when dispatch throws. The UI reports a download request,
not a confirmed OS save. There is no upload, automatic retry, file overwrite
API or retained copy after navigation. Users choose whether and where to save
through their host's download behavior; a host may save directly to its configured
download folder without displaying a chooser.

## Verification

Seven new Bun projection regressions passed, including exact unqueried and
unconfirmed reports, unknown-field/private-canary exclusion, hostile `toJSON`,
unknown modes/codes, invalid identifiers/counters/times and oversized versions.
The status component's fourteen pinned Chromium regressions passed: the seven
existing query tests plus seven diagnostic tests for explicit preview/download,
stale clearing, metadata failure, in-flight invalidation, URL release on failure,
browser denial and oversized metadata. Focused Desktop/UI types and lint passed.

The first browser collection encountered Vite late dependency optimization and
reloaded before collecting tests. Explicit prebundling now resolves Ajv and
SDK semver/Zod through their declaring workspaces, plus vitest-browser-react;
the subsequent run collected and passed all fourteen tests without
late reload. Browser tests remain serial, pinned and fatal with no empty-suite
or retry bypass.

Production frontend preview was inspected in pinned Chromium at 1280x900 light
and 390x844 dark/reduced-motion. Ordinary browser controls remain disabled. A
controlled native-invoke frontend fixture used the actual official app-version
command names, explicit keyboard preview and the existing status command. Its
445-byte JSON downloads at both widths matched the displayed preview exactly;
generation/download added zero Runtime queries. Failure discarded the previous
snapshot and produced only the stable code. The narrow card was 320 pixels wide
with no horizontal overflow; no page errors occurred. This is frontend/download
evidence, not native WebView or OS save-dialog acceptance.

Ignored evidence in `execution-validation/g7-shell/`:
`diagnostic-ui-20261021.mjs`, `.json`, the two diagnostic-download JSON files,
`20261021-diagnostic-*.png` and `diagnostic-build-20261021.log` / `.exit`.

The preceding head `3cd59fa` passed both remote Windows runs:
[push](https://github.com/FlowToolsOrg/flowtools/actions/runs/39423557975) and
[PR](https://github.com/FlowToolsOrg/flowtools/actions/runs/39423562645).
Those successes do not erase the separate local Runtime 21/24 failure on
`6432d76`, recorded in [status evidence](./g7-runtime-status.md). Runtime source,
fixtures, deadlines and real-process/drain/lease assertions are unchanged here.

On clean product commit `4617afb`, the full Windows script passed all eleven
workspace lint/type gates, all 94 Chromium tests in 29 files and Core 222 unit
tests plus two doctests. Runtime then failed two of 24 native tests:

- `root_cancel_retains_lease_until_the_actual_provider_group_drains`: the
  compiled C fixture's real-descendant readiness reached its original deadline;
- `deadline_retains_lease_until_the_actual_provider_group_drains`: its descendant
  PID file was absent, then the drain boundary reached its original deadline.

The previous malformed-frame/provider-error failures passed on this run. The
cause remains unestablished. Native test exit 101 terminated the root script
with exit 1 before Desktop/build/post-build stages; no retry was applied, and
the product worktree stayed clean. The latest local full integration is not
accepted. Exact log/exit: `root-ci-diagnostic-20261021.log` / `.exit`.

Separate complete Desktop tests on the same product source passed 99 Bun,
30 Rust unit and five Rust integration tests. Subsequent sequential forced
builds passed all eleven workspaces (2m35.914s); Web 22 / Desktop 24 production
artifacts were byte-identical under opt-in/canaries. Rust fmt, locked check and
all-target codegen Clippy with warnings fatal also passed. These focused checks
do not replace the failed root gate. Logs/exits in the same ignored directory:
`diagnostic-desktop-20261021`, `diagnostic-build-all-20261021`,
`diagnostic-production-20261021`, `diagnostic-rust-{fmt,check}-20261021`,
`diagnostic-clippy-20261021`; the combined Rust exit file records 0.

## Controlled native download acceptance

After the automated checks, the dedicated fixture was rebuilt with
`bun run --cwd apps/desktop tauri build --debug --no-bundle --config tauri.shell-validation.conf.json`.
Its metadata-only `--runtime-validation-identity` output was checked before
launch: `com.flowtools.shell-validation-20261015` and
`FlowTools shell validation - test data only`. Artifact SHA-256:
`20210BF11447B97D4EB2004A9CE36A8A7E7E785D889E5892C9597DB3DAD25B0D`.
Only this fixture's returned native window was controlled with Computer Use.

Opening settings left Runtime unqueried/unconnected. Explicit generation read
actual app `0.1.0` / Tauri `2.11.3` versions and displayed a `not-queried` report.
The separate download action saved a new 260-byte JSON file in the host's
Downloads folder without showing a save dialog. Its SHA-256 matched the exact
report reconstructed from the observed native accessibility text:
`2E1C7ED29FC8BDBA3DFA104080F7A20BF9122BC248C451D15CA79FF1B3CB41AB`.
The UI continued to say only that download was requested. No Runtime query,
start, initialization, grant, task or startup-menu action was requested.

The new file's creation time was checked against the recorded fixture process,
and its exact bytes/hash were checked before moving it into ignored evidence.
On 2026-10-25, cleanup rechecked the recorded PID, executable path and start time:
no live process matched that owner. Unrelated processes were left untouched.
The product source is still exactly `4617afb`; follow-up edits are evidence only.
This verifies one Windows Debug unqueried-report save, without accepting other
platforms, packaged release, destination/collision/cancellation behavior or
native observed/error-report flows.

Native evidence in the same ignored directory: `diagnostic-native-build-20261021`
log/exit, `diagnostic-native-identity-20261021.txt`, artifact/process/result JSON,
`diagnostic-native-{expected,saved}-20261021.json`,
`20261021-diagnostic-native-request.png`, `diagnostic-native-ui-20261021.txt`
and `diagnostic-native-cleanup-20261025.json`.

## Remaining gates

Only bounded status-report preview/download is implemented. Broader native
download/save acceptance, full logs/crash diagnostics, tray Runtime integration,
production identity/profile migration, OS login/reboot, monitor/Explorer recovery
and accepted-job continuity remain pending. G5/G6 execution and third-party
refusals are unchanged; SEC-003/007/009/013 and independent review remain open.
