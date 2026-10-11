# G3 P1.4a standalone CLI evidence

Scope: prototype Windows x64, fixed compiled T1 inventory. Build with
`bun run pack:cli`. The printed directory contains `flowtools.exe`, fixed private
Node/Bun executables, Runtime, managed runner, catalog and compiled plugin bytes.
Copy the whole directory together; Desktop/WebView and system PATH Node/Bun are
not needed. This is a local reviewable bundle, not a published signed release.
The two Rust binaries use the explicit Windows MSVC target with
[`crt-static`](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes),
so this package does not require a separate MSVC redistributable for them.

The native launcher pins Node and CLI SHA-256. CLI pins Runtime bytes; Runtime
pins its relative runner, Bun and vendor/artifact inventory at startup and before
execution. Ancestor reparse points, traversal, missing/non-regular files and byte
changes fail closed. Hashing runs outside the IPC scheduler. Package manifests
retain full file digests. No user executable path, raw plugin argv or PATH lookup
exists in this runtime path. Build tools alone resolve installed Node/Bun.

`init --policy FILE --profile DIRECTORY` explicitly initializes the current-user
private profile and imports finite command grants. `runtime start/status/stop`
uses that profile; status/stop do not cold-start. An approved global cold-start
policy and command-specific cold-start/background/effect grants remain separate.
The eleven currently supported pure/data T1 commands declare cold-start support;
network execution stays denied. Bundle `run` uses the actual granted Runtime
task, preserves JSON envelope/text formatter and returns nonzero on refusal.
Source-workspace CLI/GUI integration follows P1.4b.

Cold-start coordination uses a current-user DACL Windows named mutex bound to
SID plus canonical profile, with a 30-second acquisition limit. Only its owner
starts the fixed native Host. Child readiness is bounded to 10 seconds and
requires the expected pipe/protocol; the authenticated client then verifies the
session/version. Failed owned children are killed/waited. First pipe instance is
reserved before opening SQLite. The synchronous lock remains on one OS thread;
abandoned ownership is followed by endpoint and single-writer checks.
See [Microsoft mutex semantics](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createmutexw).

Actual acceptance is in `apps/runtime/test/standalone-fixture.mjs`: build, move
to a disposable directory/profile, remove all executable PATH resolution, explicit
init/policy, fifty independent CLI launchers sharing one instance, real Base64
JSON/text, unapproved Todo refusal and missing/tampered CLI/Runtime/runner denial.
Native regressions cover real finite lock contention/release and fixed artifact
traversal/hash rejection. Existing durable jobs/cancel/revoke fixtures stay intact.
The actual four PE import tables reject external MSVC C++ runtime dependencies;
an actual native-directory junction is refused before Node execution. A transient
management Host preserves queued durable jobs on close. The regression first
reproduced the Cancelling state and now retains Queued and the original key.
This uses a fresh Runtime profile under the current OS user; it does not certify
a separate Windows account, installer, other platforms or manual assistive-tech use.

SEC-002/003/004/006/009/010/013/015 and ADR-0001/0002 remain applicable. Pins are T1
integrity checks, not publisher provenance, code signing, protection against a
same-user attacker replacing the launcher or a TOCTOU-free OS sandbox. Same-user
mutex squatting can deny availability. Native grants/revocation and DPAPI recovery
retain P2.4a/P1.3b limits. Security Reviewer/date/conclusion: pending independent
review; no maturity promotion. P1.4b/P1.5b and parent G3 acceptance remain pending.

Local final gates: docs/lint/types pass with no lint warnings; all eleven uncached
workspace test tasks pass (including 32 core tests, six native tests, five actual
Host/CLI fixtures and 59 Chromium assertions); all eleven forced build tasks pass
after tests. Windows serial Turbo concurrency=1 preserves all checks. Generated
contract/manifest/docs/catalog checks and production artifact refusal also pass.
The new 300-second process integration budget includes bundle compilation and
all fifty independent launchers; it does not change the 30/10-second startup
lock/handshake limits or ordinary unit-test budgets. Remote CI is tracked in PR.

Windows path regression: native executable/bootstrap reads now check each
ancestor for redirection and canonicalize regular files, accepting legitimate
DOS short-name aliases used by runner temporary profiles. Junction and size
rejections remain covered; bounded native startup codes propagate without raw
stderr. The first remote P1.4a gate failed initialization and is rerun after
this correction; local evidence above does not imply remote acceptance.

2026-10-06 CI fixture correction: P1.4b remote Windows runs failed with EPERM
renaming the previously exercised native directory. The junction rejection now
uses a fresh sibling bundle, preserving all assertions, the pinned launcher and
50 concurrent cold starts. This avoids renaming recently mapped executables;
it does not retry a failed assertion, loosen timeouts or grant execution. The
actual standalone fixture passed after correction; final remote status is pending.

2026-10-11：PR #11 的 `afd0cde` 两次远程 Windows gate 一次成功、一次在
50 个并发冷启动的非零退出断言失败。整批诊断被 Bun 截断，不能据此认定
具体失败阶段。fixture 的逐项退出断言现使用同一经过 allowlist 的单项诊断，
保留 index/code/errorCode/phase/stage/elapsedMs；数量、实例一致性与模式
断言仍保留整批信息。没有原始 stdout/stderr 或凭据写入诊断，不减少调用数、
重试失败断言或放宽超时。独立测试子进程限制为一个 CPU 的实际 bundle
fixture 通过，但未复现原失败；间歇原因仍待远程完整诊断，不宣称已修复。

同日 `b196809` 的首次本地完整 gate 在另一条多命令 CLI envelope 回归失败：
真实 run、首次 jobs list 和 dependencies calls 已成功，最后只读 jobs list
等到 30 秒测试 deadline，Bun 报告 killed dangling process；随后命令和全部
50 路 standalone 冷启动通过。首次日志保留，不把该 gate 记为通过。
该多命令场景改为现有 dependency-plan 验收同样的 Node fixture：Node 管理
实际编译 CLI 子进程并等待 close/两条 stdio EOF，Bun 只管理单个 fixture。
保留 SDK envelope、生成 flags、calls 空列表、jobs 字节不变、退出码/stderr
全部断言与原 30 秒超时，没有 retry、跳过或生产行为变更。Runtime 18 项原生
与 16 项 JS 回归通过。首次挂起没有足够进程内部诊断，不能认定精确 Bun
内部原因或此前 50 路 CI 间歇根因已经修复；最终完整 gate 另行记录。
