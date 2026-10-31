# G5 P2.2 Windows headless feasibility

- Date: 2026-10-13
- Scope: prototype, Windows/x64, independent disposable native harness
- Status: targeted implementation checks passed; P2.2/G5 remain pending
- Engineering review: Codex; independent security reviewer/date/conclusion pending
- Threats: SEC-001, SEC-002, SEC-008, SEC-014, SEC-015
- Decision: [ADR-0004](../adr/0004-headless-platform-isolation.md)

`packages/headless-sandbox` is a separate Cargo/Bun workspace containing a
source-only JavaScript engine child and T0-owned native attack fixtures. It has
no dependency on product Runtime activation, package inventory, consent or grants.
The existing T1 runner is unchanged and does not become a third-party sandbox.

## Fixed profile

The embedded engine is QuickJS-NG v0.17.0, commit
`6d46d07d04041b40f4f49eaa7fdebe44c314c699`. Vendored provenance, license and
per-file SHA-256 values are retained; the build checks those bytes. The
engine vendor tree has explicit LF Git attributes so a Windows CRLF-default
checkout preserves the pinned bytes. A disposable real Git checkout regression
rehashes every original engine input against the same inventory. Only the
engine is embedded, without the upstream CLI, libc/std/os modules, native module
loading or untrusted bytecode. JSON input is double encoded and parsed as data,
preserving own `__proto__`/`constructor` keys. Source has an owned terminating NUL;
embedded source NUL is rejected.

Windows creates a unique disposable OS AppContainer profile, refusing an existing
name. Its SID gets read/execute access to the exact copied fixture image and no
access to private canaries. This fixture is separate from FlowTools user data.
The static-CRT image starts suspended and detached, with the AAP opt-out policy,
zero capability SIDs, a fixed stdin/output handle list and child-process denial.
T0 assigns its non-breakaway Job before inspecting the real AppContainer token,
exact SID and zero capabilities, then resumes it. AAP-only AccessCheck and actual
file tests include an ordinary AppContainer positive control; an undocumented
LPAC token-information query is not treated as proof.

Only SystemRoot and bootstrap LOCALAPPDATA are constructed from Windows APIs.
Windows supplies the resulting profile LOCALAPPDATA/TEMP/TMP. Tests require exactly
these four resulting keys, without inherited PATH or Host secrets. The parent
sets a temporary process-local error mode before CreateProcess, captures failures
before restoration and verifies the exact previous mode is restored. Parent WER
state is untouched; the dedicated child sets and reads back its own no-UI flags.
No registry or machine-wide policy is changed.

Engine limits are 16 MiB heap, 256 KiB stack and a 250 ms cooperative interrupt.
Parent limits are 8 KiB input, 16 KiB output and 750 ms wall time. The Job limits
active processes to one, process memory to 128 MiB and CPU to one second/20%, with
kill-on-close. Termination, empty-Job verification and pipe-worker joins share one
500 ms cleanup deadline. Every unverified custody exit, including unwinding and
already-attempted cleanup, marks the owning fixture uncertain. Uncertain fixtures
refuse reuse and preserve their directory/OS profile for diagnosis.

## Actual results

Windows 11 build 26200, Rust 1.96.0 and Bun 1.3.14:

- Standard `build`, `lint`, `check-types` and `test` passed. The test inventory
  collects 20 tests: seven unit/acceptance controls and 13 native integrations,
  with no ignored tests.
- Pure JSON execution, missing native JS APIs, owned JSON keys and near-limit
  source buffers passed their positive and rejection checks.
- Actual private file reads/writes and sibling SID reads were denied. The exact
  image read control passed, image write failed and original canary bytes stayed
  intact. An AAP-readable file worked for the ordinary AppContainer control and
  was denied for the selected opt-out profile.
- Infinite JS, heap/stack exhaustion and oversized output returned stable failures.
  Native infinite work/output was forcibly terminated. A small native allocation
  passed; an allocation above the Job budget failed. The Host executed another
  pure command after these failures.
- Native child-process creation was denied. Wrong bound SID refused before resume
  and the still-suspended child was drained.
- The explicitly included Event handle signaled the exact parent object. An
  omitted inheritable Event left that object unsignaled and exited with precise
  `STATUS_INVALID_HANDLE` (`0xc0000008`), approximately 5 ms, with Job active count
  zero. Tests permit only this exact status or a structured `ERROR_INVALID_HANDLE`
  refusal; deadlines and unrelated crashes do not qualify.
- Child error mode read back as 32771 and WER flags as 32. A deliberate unhandled
  native exception returned exact `0xe0422201`, drained the Job and allowed a
  subsequent pure command. Parent error mode and original WER state remained
  unchanged after successful creation and creation refusal.
- TCP and UDP Host loopback controls really connected/sent a packet. The selected
  child returned Winsock `WSAStartup` 10107 before socket operations, with no child
  connection/packet. A platform where initialization succeeds must deny the tested
  socket/connect/send stage with 10013; an absent endpoint or arbitrary error is
  not acceptance.
- Explicit teardown verified the owned directory and OS profile were gone.
  Deterministic worker-error regressions verified uncertainty propagation,
  retained fixture/profile and refusal of reuse without leaking a real child.
  Post-run inspection found no owned child/startup-control process.

Local detailed observations are retained in ignored `.git/g5-headless-repair-observed.log`
and the final 20-test run in `.git/g5-headless-custody-tests-v2.log`.

## Failure history

SID derivation alone, an empty environment and CREATE_NO_WINDOW did not initialize
the child on this machine. Even a fixed kernel32-only startup control failed under
the latter launch profile. The selected ADR now specifies create-only OS profiles,
explicit bootstrap environment and DETACHED_PROCESS; there is no runtime fallback.
The exact underlying loader cause is not certified by those observations.

The original file-handle diagnostic did not return its expected receipt and a
separate Windows error record reported a fast-fail. A later omitted Event probe
exposed the exact invalid-handle exception shown in the maintainer's screenshot.
Suppressing UI does not turn either observation into successful execution: the
new tests retain the precise rejection, a matching included Event positive
control, bounded drain and subsequent Host health. The Event result does not
close every other handle-type gate.

Attempting to snapshot uninitialized parent WER flags refused with `0x80070490`.
The corrected design never changes parent WER state. A first cleanup-test control
also incorrectly assumed that GetExitCodeProcess must reject an Event handle;
that failed run is retained, and a deterministic real worker IO error now checks
the intended cleanup propagation. No assertion, deadline or platform boundary was
weakened to conceal those failures.

## Reproduction and limits

Run Cargo from the package directory so its fixed static-CRT configuration loads:

```powershell
bun run --cwd packages/headless-sandbox build
bun run --cwd packages/headless-sandbox lint
bun run --cwd packages/headless-sandbox check-types
bun run --cwd packages/headless-sandbox test
```

Turbo uses the standard tasks without cached test successes; native build retains
dependency builds and CARGO_TARGET_DIR. The independent lock participates in the
Windows native cache key. This does not authorize native plugin side-loading.

This machine's Winsock initialization refusal is not complete network confinement
evidence: external TCP/UDP, DNS, redirects, WFP policy and other supported Windows
versions need separate acceptance. Event exclusion is not every handle type.
Engine vulnerabilities, complete ACL/filesystem races, arbitrary OS cleanup
failure and physical power loss remain outside this evidence.

Signed installed identity, real broker/session binding, trusted consent and
revocation, data lifecycle, package health/LKG, product UI/CLI integration,
production pins, release/manual acceptance, other platforms and independent human
security approval remain pending. No third-party product entry is enabled and no
maturity label advances.
