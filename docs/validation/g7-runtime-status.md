# G7 P3.1a3 read-only Runtime status

Date: 2026-10-20. Fixed Windows/T1 source Desktop prototype.
P3.1a/P3.1/G7 remain pending; independent security approval is pending.

## Scope and behavior

`apps/desktop/src/settings/runtime-status-settings.tsx` adds a separate settings
card. Mounting, navigation and remounting do not query or start Runtime. The
explicit keyboard-accessible query displays the actual Host instance, mode,
recorded/active job counts and the local receipt time. It labels the result as a
snapshot with no automatic updates, separate from Desktop tray residency and the
existing start/refresh management controls. It neither persists a duplicate store
nor reads job payloads, results or grants.

The existing Rust-derived `managed_runtime_control` command accepts only
`runtime.status` with `initialize=false` for this branch. After the existing
Debug/main/configured-origin check, `status_snapshot` uses the build-owned,
hash-verified Runtime helper for `--desktop-bootstrap` with cold start disabled.
It opens only the existing current-user pipe, authenticates a Desktop session,
requests `Call::Status`, checks its instance against the handshake and drops the
owned pipe. It does not call ensure-runtime, manager fallback, initialization,
policy, stop or business execution. No new IPC command, DTO, webview permission,
plugin capability, environment selector or client path was added.

Release/packaged-origin access remains denied by the existing managed-client
guard until distribution acceptance. Browser preview disables the query. Missing
bootstrap or an offline/inaccessible Host produces an honest unconfirmed state;
the UI does not infer a definite offline/uninitialized state from a generic
failure. Starting a query clears the previous snapshot, and failure leaves it
cleared. No retries, timers, background polling or write replay were added.

## Verification

Seven pinned Chromium component regressions cover:

- no query at mount/remount and explicit Enter activation;
- clearing a prior snapshot during a pending query and after disconnection;
- initialization/access failure without automatic retries;
- unexpected outcome refusal and unknown mode fallback;
- browser-preview disabled controls;
- late completion from an unmounted page not populating its successor.

Two actual native tests use independent private temporary profiles: a missing
profile remains absent; an initialized management-only Host with no grants
returns the same real instance/status as its existing session. After that Host
exits, the status query fails disconnected, no listener appears, and bootstrap/
SQLite bytes remain unchanged. Only test setup starts/initializes that temporary
Host. The status function itself does neither.

An actual MockRuntime IPC regression runs the production command table with
no app setup, plugins or user database. It refuses a non-main window, a foreign
window URL even with a trusted invoke URL, and status with initialization enabled.
The build-only `mock_command_builder` is gated to `codegen`; integration tests
retain the existing Common Controls v6 test manifest. Generated bindings stay
Rust-derived and unchanged because the existing Call/Outcome contract is reused.

Production frontend preview and controlled invoke rendering were inspected at
1280x900 and 390x844, in light/dark and reduced-motion settings. Keyboard query,
receipt display, disconnection clearing and card width passed with no page errors.
The controlled frontend uses conspicuous fixture metadata; it is not evidence of
native window clicking. Actual native Host queries above are separate evidence.
Ignored files under `execution-validation/g7-shell/` include
`status-ui-20261020.mjs`, `.json`, `status-build-20261020.log` and
`20261020-status-{browser,controlled}-{desktop,narrow}.png`.

Focused Desktop lint/types, UI types and the new native/browser tests passed.
The full Windows gate for this implementation is pending until a clean commit
is checked; earlier full-gate successes or failures do not substitute for it.
The previous head `1451c77` passed both remote Windows runs:
[push](https://github.com/FlowToolsOrg/flowtools/actions/runs/39231722749) and
[PR](https://github.com/FlowToolsOrg/flowtools/actions/runs/39231729010).
The local `73a4a1d` Runtime failure remains in
[shell history](./g7-desktop-shell.md), with its cause unestablished.

## Remaining gates

This completes only the settings status-query slice. Tray Runtime integration,
native status UI clicking, diagnostics, production profile/identity migration,
OS login/reboot, monitor hotplug, Explorer restart and accepted-job continuity
remain pending. G5/G6 execution gates and third-party refusal are unchanged.
SEC-003/007/009/013 remain open; no signing, isolation, grant or security
certification is inferred from a status response or passing test.
