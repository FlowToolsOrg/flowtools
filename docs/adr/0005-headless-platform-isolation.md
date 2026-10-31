# ADR-0005: Windows headless isolation feasibility profile

- Date: 2026-10-13
- Status: prototype design and executable feasibility harness; independent
  security approval and production activation pending
- Scope: P2.2, Windows x64, source JavaScript and pure JSON input/output
- Threats: SEC-001, SEC-002, SEC-008, SEC-014, SEC-015

The existing fixed T1 Bun runner and its lifetime Job do not confine untrusted
third-party OS access. Signed packages establish identity, not permission. The
new `packages/headless-sandbox` is an independent native harness, outside the
production Runtime/Host entry points. It cannot install, approve, activate or
execute an installed plugin. G5 and P2.2 remain pending until the declared
boundary has actual native validation and its remaining integration/acceptance.

Choose a pinned QuickJS-NG engine in a separate native child, with no upstream
CLI, libc modules, native module loader or bytecode input. Source and pure JSON
are bounded. Engine memory, stack and cooperative interrupt limits complement
parent wall-clock, OS memory, CPU, output and process budgets. An engine crash
must remain outside the Host process. This engine choice does not certify its
implementation against native vulnerabilities.

On Windows, create a unique disposable OS AppContainer profile with
`CreateAppContainerProfile`; an existing name refuses rather than adopting an
old profile. This OS fixture profile is separate from the user's FlowTools
Runtime profile. Deriving a SID without creating the OS profile did not provide
a working child on the development machine and is not a supported fallback.

Create the child suspended with the All Application Packages opt-out policy,
zero capability SIDs, no network capability and an explicit inherited handle
list containing only the bounded input/output pipes. The fixed image uses a
static CRT. Its detached process has no console window; `CREATE_NO_WINDOW`
produced `STATUS_DLL_INIT_FAILED` even with a minimal kernel32-only startup
control. `DETACHED_PROCESS` is the sole selected launch profile, not a runtime
retry after a weaker or failing launch.

Construct only `SystemRoot` from `GetWindowsDirectoryW` and the bootstrap
`LOCALAPPDATA` path from `GetAppContainerFolderPath`. An empty environment
refused AppContainer initialization on the development machine. Windows
rewrites `LOCALAPPDATA` and supplies the profile's `TEMP`/`TMP`; native tests
require exactly these four resulting keys and no Host `PATH` or secrets. No
broad parent environment is inherited.

A test-only directory grants the exact fresh SID read/execute access to the
fixed child image; private canaries have no such grant. Assign the suspended
child to a non-breakaway Job with one active process, 128 MiB process memory,
one CPU second, a 20% CPU rate and kill-on-close limits before resuming it.
Read the real token's AppContainer state, exact SID and zero capability count.
Use documented `AccessCheck` against an AAP-only package pass to validate the
opt-out behavior, with an ordinary AppContainer positive control and actual
AAP-file read denial. The rejected token-information class 46 query is not
treated as an available documented LPAC proof. Any setup or verification
failure refuses execution and drains the still-suspended child.

Before `CreateProcessW`, scope the harness parent's error mode under a
process-local lock. The inherited error mode applies before the first child
instruction; the child also sets and reads back its own WER no-UI flags. A
previous attempt to read parent WER settings found an uninitialized state
(`0x80070490`); parent WER is now never modified. Restore and read back the
parent's exact prior error mode after successful creation or refusal, preserving
`GetLastError` before restoration calls. An
explicit unhandled native exception must return its exact status and leave an
empty Job within the same deadlines. No system registry, machine-wide error
policy or other application's settings are changed.

Only bounded anonymous input/output pipes cross the boundary. No Host tokens,
user database, grants, package paths, environment, arbitrary executable, raw
argv or native capability API is exposed. Native malicious probes deliberately
attempt file, network, child-process, environment and handle access through OS
APIs so language-level API removal cannot masquerade as an OS denial. The Event
handle probe includes a whitelisted Event positive control; an omitted Event
must leave the exact parent object unsignaled and return only
`ERROR_INVALID_HANDLE` or the precise `STATUS_INVALID_HANDLE` process failure.
A deadline or unrelated crash does not count as handle refusal. An earlier
file-handle diagnostic did not reach its expected result; this Event evidence
does not close all handle-type acceptance.

TCP and UDP tests use live Host loopback endpoints and positive connection or
packet controls, require a stage-specific native refusal, and verify no child
connection or packet. On the development machine Winsock initialization itself
returns `WSASYSCALLFAILURE` (10107), before any socket operation. This is evidence
of this selected profile's observed refusal, not proof of WFP, external TCP/UDP,
DNS, redirect or full platform network confinement. If initialization succeeds,
the tested socket/connect/send stage must explicitly deny with `WSAEACCES`
(10013); absent endpoints and arbitrary errors do not qualify.

JS probes exercise loops, allocation, stack and oversized output. JSON is
double encoded and passed through `JSON.parse`, preserving own `__proto__` and
`constructor` keys; the engine source buffer has an owned NUL terminator and
embedded source NUL is rejected. The parent permits at most 8 KiB input,
16 KiB output and 750 ms wall time. Termination, Job drain and pipe-worker joins
share one 500 ms cleanup deadline. Custody holds the owning fixture's uncertainty
flag; early returns, unwinding and previously attempted cleanup cannot bypass
failure propagation from Drop. Verification failure preserves uncertain
fixture state and refuses reuse. Explicit teardown checks both the owned test
directory and OS profile removal. These bounds do not guarantee cleanup after
arbitrary OS failure or certify power-loss recovery.

Every tested refusal must preserve the Host and drain the owned Job; no retries
or weakened deadline may turn a failing machine into a passing one. Test
directories are disposable fixtures only, never the user's Runtime profile.

Unsupported OS/architecture refuses this profile. Real signed package identity,
T0-bound active sessions, P2.3b adapters, P2.4b trusted consent/revocation and
P2.6b data lifecycle must be integrated before any third-party activation.
T2/TL UI isolation remains independent P2.1 work. No main-realm or ordinary
process fallback is permitted. Independent security reviewer/date/conclusion,
production pins and release/manual acceptance remain external gates.

References: [trust boundaries](./0001-plugin-trust-boundaries.md),
[package policy](./0002-capability-and-package-policy.md),
[Windows AppContainer/LPAC](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer),
[process error-mode inheritance](https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-seterrormode),
[process-local WER settings](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-wersetflags),
[QuickJS-NG MSVC support](https://quickjs-ng.github.io/quickjs/diff/),
[QuickJS embedding limits and bytecode warning](https://bellard.org/quickjs/quickjs.html).
