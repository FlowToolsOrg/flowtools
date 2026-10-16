# G7 P3.1a2 Desktop appearance preference

Date: 2026-10-16. Independently scoped Host UI `prototype`.
P3.1a/P3.1/G7 remain pending.

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
Native OS startup/Runtime actions were not invoked. The full root gate is recorded
after the independently committed change.

```powershell
bun run --cwd apps/ui-test test -- src/test/appearance/desktop-appearance.test.tsx
```

## Remaining acceptance

Dedicated native Webview restart/profile migration, diagnostic export, authoritative
Runtime status, production identity, native tray/conflict and OS restart/multi-screen
acceptance remain separate. No G5/G6 execution gate is opened. SEC-003/007/009/013
remain open. Actual independent security reviewer/date/conclusion: pending.
