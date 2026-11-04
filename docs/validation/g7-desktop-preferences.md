# G7 P3.1a2 Desktop appearance preference

Date: 2026-10-16. Independently scoped Host UI `prototype`.
P3.1a/P3.1/G7 remain pending.

Current follow-up (2026-10-19): `06e29cd` passed the complete local Windows gate
and both remote runs. Dedicated native Webview process restart also retained the
selected dark appearance; see [native acceptance](./g7-native-window-acceptance.md).
The later full gate on `73a4a1d` failed three unchanged Runtime descendant paths
(21/24); latest local integration remains unaccepted, see
[the retained failure](./g7-desktop-shell.md). Appearance/browser success does not
override that result.
Historical failures below are retained, with their underlying cause still unknown.

## Behavior and ownership

Desktop's root layout owns the system/light/dark preference across routes.
It restores the preference before the first React paint, even without opening
settings. System mode listens to live OS color changes. Explicit light/dark wins
for Host CSS variables, native control color-scheme and Tailwind dark utilities.
The settings buttons expose pressed state and support ordinary keyboard activation.

Only `flowtools.desktop.appearance` is persisted in the current Webview origin's
localStorage: formatVersion 1 plus one mode, bounded to 128 UTF-16 code units on
read. Unknown versions/fields, wrong types, malformed or oversized records fall
back to system with visible feedback, preserving original bytes until an explicit
choice/reset. Write/reset failure applies the requested appearance for this
window and reports that restart may restore the old preference. Retry remains
available. Reset removes only this record and returns to system mode.

Implementation: `apps/desktop/src/settings/appearance.tsx` and
`appearance-settings.tsx`, mounted in `App.tsx`; CSS precedence is in `App.css`.
This does not select E02 plugin resources or add a second Runtime data store.
Origin storage is a UI convenience, not a trusted identity, secure store,
cross-origin synchronization or OS isolation control. No native command,
permission, CSP, grant, command execution or package admission changes.

## Verification

Eight pinned-Chromium browser regressions in
`apps/ui-test/src/test/appearance/desktop-appearance.test.tsx` passed: route
withdrawal/restoration, provider restart before settings, live system changes,
explicit light, reset, five malformed/unsupported record cases, unrelated-key
preservation, read/write/reset failures and retry. Desktop/UI-test lint and
types passed.

An isolated pinned-Chromium context against the actual Desktop Vite page verified
route navigation and reload persistence, explicit light on an emulated dark OS,
distinct light/dark CSS variables and backdrop, keyboard Enter reset and 390px
reflow without horizontal document overflow. Visual inspection covered both
1280x900 themes and the 390x844 layout. No page errors were emitted. The original
combined Tailwind background-image value included a color and was invalid;
the fixed Host background uses a theme variable with the valid background shorthand.
Ignored local evidence: `execution-validation/g7-shell/appearance-browser.json`,
`appearance-light-desktop.png`, `appearance-dark-desktop.png` and
`appearance-narrow.png`. This browser context does not use a native user profile.
Native OS startup/Runtime actions were not invoked.

The full root gate on `f669162` passed documentation/generated contracts, all
11 workspace lint/type gates, all 80 Chromium tests and 222 Core tests plus two
doctests, then failed Runtime 23/24 at the existing real-descendant readiness
check. This is not a passing full gate. The earlier green gate on `8741845`
does not cover the new appearance commit. Native process-creation controls and
reverted experiments are documented in [shell evidence](./g7-desktop-shell.md).
Separate follow-up validation passed all 11 forced workspace builds (2m56.04s),
Web 22 / Desktop 24 byte-identical production opt-in/canary artifact checks, and
the restored main runner's four protocol/execution tests. Logs are retained in
ignored `appearance-root-build.log`, `appearance-production-artifacts.log` and
`runner-restored-tests.log` under the same evidence directory. These do not
replace the failed full root gate.

2026-10-17: the actual production preview also passed the route/reload, computed
light/dark colors, keyboard reset and narrow-layout checks; final screenshots
and JSON above were refreshed after transitions settled. CSS minification turns
`#333333` into `#333`; the disposable browser check compares computed RGB colors.
The later full Desktop test attempt emitted independent Bun OOM/illegal-instruction
crashes, including the `bun -e 'process.exit(0)'` CI helper probe, before a complete
result. No passing Desktop suite is claimed for that attempt. After available
system memory recovered, three minimal Bun exit probes passed, but the original
unchanged root-cancel test still failed its five-second readiness check (5.14s).
No further full-gate retry was used to conceal that failure.

```powershell
bun run --cwd apps/ui-test test -- src/test/appearance/desktop-appearance.test.tsx
```

## Remaining acceptance

Production profile/identity migration, diagnostic export, authoritative Runtime
status, native tray menu/exit, OS restart, monitor hotplug and accepted-job
continuity remain separate. No G5/G6 execution gate is opened. SEC-003/007/009/013
remain open. Actual independent security reviewer/date/conclusion: pending.

Controlled native Webview restart and shortcut-conflict recovery passed on
2026-10-19 with an isolated Debug identity. This does not establish OS reboot,
power-loss durability, tray-menu interaction or production profile compatibility.
