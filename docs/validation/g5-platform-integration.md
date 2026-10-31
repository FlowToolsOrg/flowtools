# G5 platform prototype integration

- Date: 2026-10-31
- Base: main `aa165e5759c2f27db03f91226419df833307ffed`
- Scope: prototype; G5 completion and independent security acceptance pending

PR #11 was merged and its branch removed while this local work was paused.
The current main branch also contains the subsequently merged engineering
changes for controlled trust recovery, inert inventory reconciliation and tool
admission. This integration preserves those implementations.

Only original-artifact commit pins and the standalone WebView/headless feasibility
profiles are carried forward. The unmerged same-process trust-handoff experiment
is retained on the original local reference branch; its conflicting recovery API
is not substituted for main's root/recovery dual-quorum protocol. The applicable
recovery authority remains [ADR-0004](../adr/0004-controlled-trust-recovery.md).
The independent headless decision is renumbered
[ADR-0005](../adr/0005-headless-platform-isolation.md), preserving the established
recovery ADR identity.

## Evidence and boundaries

- [Commit pins](./g5-commit-artifact-pins.md): original candidate/provider handles
  survive final verification through the inventory transaction. Current-main
  controlled recovery and orphan reconciliation remain separate operations.
- [WebView feasibility](./g5-webview-feasibility.md): four hidden disposable
  profiles, actual storage controls, empty native authority, bound inert sessions,
  HTTP refusal and three normal/interrupted cleanup paths. The known speculative
  TCP and invoking-frame gaps remain open; this is not a product sandbox.
- [Headless feasibility](./g5-headless-feasibility.md): 20 native/contract tests,
  exact invalid-handle and crash statuses, bounded Job drain and fixture
  uncertainty. The selected machine's Winsock-startup refusal does not certify
  full network confinement or every handle type.
- The Windows build checks the opt-in WebView feature without launching it, and
  runs the independent headless workspace through standard uncached tasks.
  Engine input bytes have LF attributes and a real CRLF-default Git checkout
  regression against their pinned hashes.

The original targeted native evidence was collected on 2026-10-13; the standalone
headless standard build/lint/types and 20-test run also passed on 2026-10-31.
Integration on current main requires fresh checks; historical results are not
current-revision acceptance.

Local resumed contract tests exposed a native direct-spawn problem under pinned
Bun 1.3.14. The new WebView suite had eight of nine passes, with a process-helper
timeout; an existing CI Cargo metadata test also timed out. Node controls and
a minimal cmd-launch control completed, while direct Bun probes reproduced the
timeout. No deadline, assertion, required gate or Turbo environment is relaxed.
See the separately retained failure observations and
[related upstream report](https://github.com/oven-sh/bun/issues/32011); the exact
local cause is not certified. Current-revision full checks and remote CI are
recorded with delivery and remain pending until completed.

Signed installed execution identity, real third-party broker/consent/data
integration, complete platform network/handle acceptance, private profile ACLs,
filesystem races/power loss, health/LKG, production pins and independent human
security approval remain pending. Maturity and production refusal do not change.
