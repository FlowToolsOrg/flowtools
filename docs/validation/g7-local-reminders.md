# G7 headless local reminders

2026-10-28, P3.1b5, fixed T1 / Windows prototype. Manual acceptance is waived,
not passed. This does not advance G7 or production/security labels.

The thirteenth actual built-in package, `plugin-local-reminder`, exposes the same
compiled headless command to GUI and CLI. Desktop's schedule form supplies title,
content, explicit IANA time/DST policy, one-shot or finite elapsed intervals,
preview, native approval, listing, editing and cancellation. The existing Runtime
approval surface can approve this exact package's notification scope and explicit
background permission. Declaring the effect never grants it.

The bounded runner notification operation contains plain title/message only.
Runtime rechecks its bound root/service session, manifest AND grant, scope,
epoch, expiry, cancellation and call budget at the effect point. The existing
DataStore writer commits a metadata-only attempt before OS IO. Only a digest,
run ID and status persist there; title/content remain in encrypted private job
storage. Exact repeats return the first result; changed content refuses. Crash
unknown attempts never resend. SQL failure before commit has no OS effect;
post-attempt write failure fails closed and retains uncertainty.

Windows uses pinned `tauri-winrt-notification` 0.7.2 and Desktop's fixed
`com.hmsuiji.desktop` identity, plain text, no sound/actions/images/XML. There
is no caller app ID, shell command, PowerShell impersonation or new webview
permission. `requested` means OS API acceptance, not confirmed presentation.
API refusal records `failed`; other platforms return `unsupported`. `unknown`
records uncertainty. Desktop offers an explicit result query, with no polling.

The native fixture closed all GUI/CLI connections before executing the actual
reminder, then read its durable command result. The Windows API returned
**requested**. It also verified unchanged job counts on later ticks, encrypted
input, no plaintext body in SQLite, grant revocation, cancel, stale replacement
refusal and exact retry. This is Host/API evidence, not visual OS acceptance.
Log: `execution-validation/g7-shell/scheduling-notification-e2e-20261028.log`,
exit 0. Three Core policy/durability tests, Core/Runtime all-target Clippy, six
pinned Chromium tests and nineteen compiled-entry/CLI tests passed. The latter
cover all thirteen actual commands, including a consumer without React/UI/source,
schema rejection and abort. Desktop and runner types/lint passed. The required
first root gate passed install, contracts, generation, lint and types, then found
two outdated inventory-count assertions in Runtime client tests (11 operations /
12 plugins). The assertions now include the notification operation and 13th
package, with extra app-ID/XML injection refusal cases. This failure is retained
in `scheduling-root-ci-20261028.log`; the corrected tree needs a new full gate.
Earlier Runtime descendant failures also remain recorded.

Primary API reference: [Toast API 0.7.2](https://docs.rs/tauri-winrt-notification/0.7.2/tauri_winrt_notification/struct.Toast.html).
Search, file/media workflows and external G5/G6 execution prerequisites remain
separate work. No fixture trust pins or arbitrary local executables are enabled.
