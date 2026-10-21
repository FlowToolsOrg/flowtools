# G7 P3.1a4 Runtime status diagnostic preview

Date: 2026-10-21. Desktop status-report prototype only.
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
through their host's download behavior.

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
the subsequent run collected and passed all fourteen assertions/tests without
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

## Remaining gates

Only bounded status-report preview/download is implemented. Native WebView
download/save acceptance, full logs/crash diagnostics, tray Runtime integration,
production identity/profile migration, OS login/reboot, monitor/Explorer recovery
and accepted-job continuity remain pending. G5/G6 execution and third-party
refusals are unchanged; SEC-003/007/009/013 and independent review remain open.
