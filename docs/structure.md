# Flow Tools 架构设计规范

## 下一阶段结构与当前实现

[下一阶段实施设计](./next-milestones.md) 定义 G0–G8 的顺序与验收。
G2 已新增 `packages/runtime-core`（无 Tauri 依赖的 Rust crate）、`apps/runtime`
（无界面运行服务）、`packages/runtime-client`（生成 DTO 的 TS 客户端）和
`packages/plugin-runner`（受管执行入口）。这些目录已交付 G2 的固定 T1 / 可丢弃 profile 验证基础；G3 T1 broker 与单写者数据基础已交付，持久授权与发行仍待后续子项。
SDK 已有命令/Manifest；服务依赖和异步 data 契约留待后续。Rust core 当前管理
验证任务与 P2.3a T1 内存策略 broker；单写者数据；持久 grants、服务锁及工具
artifact/lease/GC 仍为后续目标。
专用 Desktop 与 Node 客户端已使用同一验证服务。
普通受管 T1 runner 不是 T3 sandbox，Web 原型不自动连接本机服务。

## 安全设计来源与实现边界

- `docs/adr/0001-plugin-trust-boundaries.md`：T0/T1/T2/T3/TL 执行位置、身份
  来源、隔离边界与 v1 非目标；Single React Tree 只适用于可信 T1。
- `docs/adr/0002-capability-and-package-policy.md`：Rust broker、scope、
  签名准入、撤销、迁移与恢复的 accepted-design，不是已实现控制。
- `docs/security/threat-model.md`：稳定 SEC ID、实际入口证据、owner、目标
  缓解、恶意 fixture 与残余风险；实现验收由 production-roadmap 指定阶段负责。

以下生命周期、权限与消息流水线包含目标设计，不能由流程图推断已强制执行。
当前原生 adapter/raw SQL/HTML iframe 的具体缺口以威胁模型为准。

## 工程验证结构

- `scripts/production-artifacts.ts`：固定生产目录的只读 byte/hash、危险指纹、
  已知 certification AST 与 canary 检查；拒绝路径重定向，不执行产物。
  `verify-production-entrypoints.ts`：仅两端固定 build，child-only probes 后
  所有文件逐字节一致。`production-artifacts.test.ts` 与 CI contract 回归
  覆盖拒绝/顺序/fatal。post-build CI 执行，无 native launch 或用户 DB。
  见 [产物 gate](./validation/p0-production-artifacts.md)。

- Desktop `runtime/html-development-policy.ts` 是 Host 构建策略；普通
  `html-plugin-bridge.ts` 永久拒绝。`development-html-launch.ts` / bridge / surface
  仅在 DEV + opt-in 下动态加载；生产不创建 iframe 或读外部入口，开发也不支持
  raw native/SQL/FS/opener。共享 command types 不反向导入整个 Host App。
  `test/html-plugin-bridge-gate.test.ts` 与 `html-development-matrix.test.ts` 覆盖
  普通入口、副作用之前拒绝、六构建模式及原生危险方法始终禁用；不 mock
  用户数据。这些自动化不是实窗验收；独立 r3 人工清单与身份核查分别记录于
  [Desktop gate](./validation/p0-desktop-external-gate.md)。
  `test/html-mode-build.ts` 为每个模式提供独立实际编译进程，规避已复现的
  Windows Bun 文件缓存生命周期问题；该进程不是插件隔离 runner。

- `packages/sdk/src/services/plugin-file-loader.ts`：普通 SDK 的 deny-only 外部
  入口；`development-plugin-file-loader.ts` / `development-policy.ts` 为显式
  DEV + opt-in 的危险预览实现，只从 `@flowtools/sdk/development` 动态导入。
  `packages/sdk/test/external-code-gate.test.ts` 覆盖导入前拒绝与构建模式矩阵。
- Web `src/app/development-policy.ts` 与 registry store 在读文件/持久化之前
  拒绝；bootstrap 不恢复外部源码，旧 IndexedDB 数据保留。对应
  `src/app/external-code-gate.test.ts` 覆盖绕过 UI 和恢复/状态无副作用。
  此为 P0.3b1 停用策略，不是第三方隔离；Desktop gate 仍待后继独立实施。
  `apps/ui-test/scripts/validate-web-source-gate.ts` 用三个真实 Web server 与临时
  Chromium context 验证默认拒绝、显式预览、键盘及旧源码保留；见
  [源码入口验收](./validation/p0-web-source-gate.md)。

- `packages/ui/src/components/plugin-status`：共享 maturity/evidence badge，
  消费 SDK 词表；`ToolStatus` / `ToolMarketStatus` 是 PluginMaturity 别名。
  `apps/ui-test/src/test/plugin-status` 覆盖四状态、缺省 Prototype 与证据不升级；
  Web/Desktop 直接使用 metadata/catalog，不维护“stable”镜像状态。

- `packages/sdk/src/compat/catalog.ts`：portable Catalog schema/path 与独立
  indexed/entry-resolved 证据；公开无 React 的 `@flowtools/sdk/compat/catalog`。
- `scripts/inspect-html-plugins.ts`：只读扫描 checkout，生成两份相同的 package
  identity/相对资源/hash Catalog；无本机根路径、development URL 或认证升级。
- `scripts/verify-plugin-catalog.ts` / `scripts/catalog.test.ts`：路径/identity/
  hash fixture/假认证/重复目录拒绝门禁。fixture 位于
  `scripts/fixtures/html-catalog/static-entry-v1.json`；控制文本 hash 归一化换行。
- Desktop `src/runtime/catalog-entry.ts` / `test/catalog-entry.test.ts`：明确 DEV
  与开发者指定的 checkout root 才解析本地相对入口；不是原生 scope enforcement。

- `packages/sdk/src/types/maturity.ts` / `test/maturity.test.ts`：成熟度统一词表、
  prototype 默认与独立 compatibility evidence enum；不执行安全认证。
- `scripts/generate-manifests.ts`：构建时排序扫描内置 metadata，生成 Web/Desktop
  manifest 与 `packages/cli/src/builtin-manifests.ts` 固定清单；read-only --check
  检查三个输出。`plugins/test/maturity-contract.test.ts` 通过实际 compiled imports 和
  CLI 子进程 list/info 验证 maturity 一致。源码正则扫描仍不是完整 manifest 验证。
- `packages/cli/src/discovery.ts`：只读构建内嵌清单，未知 ID 在 IO 前拒绝；仅加载
  固定 dist 普通文件，拒绝缺失/破损、junction/symlink 与 metadata 不一致，不再
  扫描或改写源码。`src/discovery.test.ts` 复制真实 compiled CLI 到临时隔离目录，
  执行未知入口、坏产物与顶层源码 canary 拒绝回归；CLI test 自行构建 CLI，
  十二真实内置执行仍由 plugins smoke 验证，不新增循环 workspace 依赖。

- `packages/sdk/src/execution/executor.ts` / `test/executor.test.ts`：共享真实
  `run()` 执行边界，校验/defaults、稳定 envelope、取消/异步等待上限与资源清理。
  输入摘要只保留类型和大小；CLI/Web/Desktop 已接入，执行器不是 sandbox。
- `packages/sdk/src/execution/history.ts`：versioned 元数据-only external store，
  bounded records、失败/取消、刷新恢复与非法记录拒绝；不导入旧未验证 key。
- `packages/ui/src/components/run-panel/execution-panel.tsx`：三态操作、JSON/schema、
  实际 envelope、取消/卸载清理与不可序列化输出失败；不制造演示结果。
- `apps/web-vite/src/runtime/plugin-runtime.tsx` / Desktop
  `src/runtime/plugin-execution.ts`：宿主真实能力 context + SDK executor。
- `apps/ui-test/scripts/validate-execution-hosts.ts`：真实 Web/Tauri 验收，Node
  Playwright + 独立测试 identity/CDP，不 mock 原生 IPC；记录与截图在
  [宿主验收](./validation/p0-execution-hosts.md)。
- Desktop `tauri.execution-validation.conf.json` / `tauri.manual-validation.conf.json`：
  隐藏自动化与可见人工验收的独立测试身份，均以根路由查询标记进入首页。
  `src-tauri/tests/bindings.rs` 用真实 Tauri MockRuntime 验证入口 URL，不启动宿主；
  URL 回归不替代独立包实窗验收。

- `scripts/docs-check.ts` / `docs-check.test.ts`：十一份核心/设计/ADR/威胁/PR 文档的
  只读契约，内联本地链接路径与风险字段验证；脚本由 Desktop 测试任务消费。
  不检查远端 URL、Markdown anchor、运行时安全或 reviewer 批准。
- `.github/pull_request_template.md`：安全边界变更的 threat/ADR、scope、拒绝
  回归、撤销恢复与审阅证据；不是仓库强制合并保护配置。
- `.github/workflows/windows-quality.yml`：固定工具链与 Action SHA 的 Windows
  PR 工作流；只读权限，不缓存 JavaScript 构建产物。
- `scripts/check-ci.ps1` / `ci-gates.ps1`：本地与 CI 共用的顺序门禁、退出码与
  工作树漂移检查；`ci-contracts.test.ts` 验证工作流和 PowerShell 失败传播。
- Web/Desktop `generate:routes` 使用锁定的官方 TanStack CLI；Desktop
  `src-tauri/src/bin/export-bindings.rs` 使用同一命令构造器导出 Rust bindings。
  两者均在 fresh checkout 的类型检查前生成，不依赖一次手动桌面启动。
  所有 workspace 提供统一的 `lint`、`check-types`、`test` 和 `build`。根 Turbo
  测试图依赖 `^test`，package 合约测试自行构建产物后断言，再运行消费者测试；
  禁止空测试成功选项与测试缓存。根 `test` 和 `build` 必须顺序执行。

- `packages/sdk/test`：SDK 值对象、registry、lifecycle、watchdog 与公开导出。
- `packages/cli/src/*.test.ts`：CLI 参数、schema、formatter、SDK runner/context
  与 entry 子进程拒绝路径；使用无 React 的 `@flowtools/sdk/execution` 子入口。
- `plugins/test/cli-execution.test.ts`：真实编译后 CLI 的 generated flags / JSON
  envelope / schema 错误 / 非法 timeout / text 回归；不替换内置插件 `run()`。
- `plugins/test/smoke-fixtures.ts` / `plugin-smoke.test.ts`：十二个真实 compiled
  entries 与 CLI discovery 一一对应；注入受控 request/storage，实际 success、
  schema/abort 拒绝无副作用及异常 envelope。根 `smoke:plugins` 显式构建前置产物。
- `plugins/test/state-network.test.ts`：SDK request 不能退回 raw fetch；Todo
  app store/CLI key、缺失 capability 与损坏旧数据的保护回归。
- `packages/ui/test`：构建后公开导出；`test/consumer` 独立编译声明消费。
- `plugins/plugin-entries.ts`：构建与合约测试共享目录 inventory；
  `plugins/test` 验证内置插件和 CLI 执行链路。
- `apps/web-vite/src/app/*.test.ts`：逐插件 manifest、真实加载与命令注册合约；
  冷加载/批量注册集成测试使用有限 30 秒预算，普通用例保留默认超时，不重试。
- `apps/ui-test/src/test`：固定 Playwright Chromium 的无界面组件交互测试。
- `apps/desktop/src-tauri/src`：内存数据库 Rust 合约测试，不使用用户 app-data。
- `apps/desktop/test`：Rust 测试清单非空检查及其回归，防止 Cargo 零测试假绿。

浏览器与插件生产配置不引入 Bun 测试全局类型；测试配置单独类型检查。手动 UI
验证继续补充视觉、无障碍、路由和插件渲染的自动化覆盖。

## 1. 核心抽象层 (Abstract Layer)

定义系统核心调度骨架与双端路由机制。

### SDK & HOST 交互规范

- SDK -> HOST: 注入 Types definition (类型定义) 与 Function controller (功能控制器)。

- HOST -> SDK: 支持 Override (覆盖重写)。

### 双端路由策略 (Data Hooks & Ctx API)

- UI 路由 (Human End): HOST -> UI -> React -> Setup Render -> Plugins。

- CLI/AI 路由 (Server/AI End): HOST -> CLI -> Commander -> Features -> Plugins。

### Features Cli

- 触发机制: CLI 接入 ctx api，结合 AI contextable (AI 上下文感知) 实现 No config Auto generated (免配置自动生成)。

- 结构定义:
  - Metadata: 元数据 (包含 name, key, description)。

  - Main Function: 主函数 (特性：Atomizable 原子化、Composable 可组合、Low-invasive 低侵入)。

  - Zod Schema: 数据结构与校验模式。

## 2. 生命周期状态机 (Plugin Lifecycle)

规范插件从装载到卸载的 5 个标准阶段：

### Phase 1: Pre-lifecycle (High Cache / 预处理)

load plugin -> dependencies mounting -> metadata parser (store) -> CLI adapt。

### Phase 2: Before Mounted (Check Stage / 挂载前校验)

Permission Checkout (权限校验) -> Compatibility check (兼容性检查) -> [Performance check] (性能基准测试, 暂时未验证必要性)。

### Phase 3: Mounting (挂载执行)

Hydration (状态水合) -> AOP subscription (切面订阅) -> Activate plugin (激活插件) -> Render (渲染)。

### Phase 4: Keep Alive (保活态)

能力支持：Full-AOP (全量切面)、Messageable (消息通信)。

### Phase 5: Unmounted (卸载清理)

State Preservation (状态持久化) -> AOP unsubscribe (退订切面) -> UI unload (卸载视图) -> Message lock (消息锁死)。

## 3. 权限安全管道 (Permission System)

插件运行权限的安全校验与熔断机制。

- 配置层: Multi-declaration (多重声明) & Capability API (能力 API)。

- 校验流水线:
  1. Pre-release detection (预发布检测)。
  2. 分流判定:
  - Safe Plugin (安全插件) -> 进入下一步。
  - Unsafe Plugin (不安全插件) -> 触发 Re-verifine (阻断或重验)。
  3. Runtime Checkout (运行时校验)。
  4. 执行决议: Pass (放行) 或 Fuse (熔断)。

## 4. 消息通信总线 (Message System)

定义 User, Plugin 与 Host 之间的事件交互拓扑。

### 基础交互 (Direct IO)

User -> Trigger A -> Plugin 1 -> Feedback -> User。

### 宿主能力调用 (Host API)

Plugin 1 -> Capability Use -> Host -> Response -> Plugin 1。

### 跨插件 RPC 调度 (Host-Mediated / 虚拟桥接链路)

Virtual Bridge Link 流程:

1. User 发起 Trigger B 至 Plugin 1。

2. Plugin 1 发起中转请求: C1 Call -> Host。

3. Host 驱动目标插件: Run C1 -> Plugin 2。

4. Plugin 2 回传结果: Response -> Host。

5. Host 转发至调用方: Forward -> Plugin 1。

## 5. 跨平台适配层 (Platform Adapter)

Web (基于 Vite): 实现 Dynamic Import (动态导入)、SDK Mount (SDK 挂载)、Import Map (导入映射)。

Desktop (基于 Tauri): 已有 `apps/desktop` 壳，用于承载 HeroUI + Tailwind 桌面启动器、TanStack Router 桌面路由、React/SDK 内置插件面板和 HTML 插件目录。HTML/Legacy 执行默认拒绝，iframe runner 仅供显式危险 DEV 预览。原生能力通过官方 Tauri plugins 安装，再由 desktop SDK adapter 暴露为内置插件 capability；这不构成第三方授权。

Desktop 插件元数据逐步迁移到 Rust 后端。`apps/desktop/src-tauri/src/models/plugin.rs`
定义插件记录，`repositories/plugin_repository.rs` 负责数据库增删改查、启用/禁用和基础校验，`commands/plugin_commands.rs` 暴露 Tauri IPC。持久化的 `state`
与 SDK registry 使用同一组状态值。

### HTML 插件兼容导入层

- `packages/sdk/src/compat/html-plugin.ts`: HTML `plugin.json` 类型、命令解析、兼容支持分级。
- `scripts/inspect-html-plugins.ts`: 扫描本地 HTML 插件 checkout，生成 runtime 目录和 docs 副本；同时记录静态资源目录并跳过源码态 Vite HTML 入口。
- `apps/desktop/src/data/html-plugin-catalog.json`: desktop 启动器直接读取的 HTML 插件目录。
- `docs/html-plugin-catalog.json`: 同源的人类可读目录副本，供后续插件市场/导入器参考。
- `apps/desktop/src/runtime/desktop-capabilities.ts`: 将 SDK `fs/network/clipboard/dialog/notification/storage/db/native` capability 映射到 Tauri plugins 或 WebView API。
- `apps/desktop/src/runtime/html-plugin-bridge.ts`: 普通入口永久拒绝；开发实现拆分至
  `development-html-plugin-bridge.ts`，仅动态加载的危险预览使用有限方法表。

兼容级别：

- `webview`: 可优先进入 Tauri WebView 承载。
- `preload-bridge`: 需要补齐旧版宿主 API bridge。
- `native-bridge`: 需要 Tauri/Rust 原生能力配合。
- `metadata`: 仅完成索引或适合重写为 FlowTools headless/tool 插件。

新增 Tauri 原生能力时，优先在 `apps/desktop` 内执行
`bun run tauri add <plugin-name>`，再把插件 API 适配到 SDK capability；避免让业务插件直接依赖 Tauri API。

## 6. 插件捆绑包结构 (Plugins Bundle)

Plugins Bundle Metadata (捆绑包元数据)

Presets (预设基座): 插件的预设配置, 做到开箱即用。

Instances (插件实例): 独立打包的 Plugin A, Plugin B 等。

## P0.3c catalog and permission presentation

Desktop market actions save built-in configuration only. HTML entries display
不可安装 and saved records cannot grant execution. Removing a record is not a
package uninstall. The permission center shows capability declarations and
explicitly records that per-plugin grants and isolation are not implemented.
Legacy persisted DTO status values remain metadata for compatibility.
All maturity labels stay prototype; SEC-001–SEC-012 remain open.

## P1.1a 新增模块

packages/sdk/src/manifest 已存在：schema、json-schema、execution、legacy 和
Node-only package verifier。packages/sdk/test/manifest\*.test.ts 覆盖协议与文件
拒绝；scripts/verify-manifests.ts 为只读契约与十二实际包 gate。见 [协议](./manifest-v1.md)。
plugins/plugin-\*/commands.ts 为纯命令源；index.tsx 为 UI；command-contract.ts
声明同源 operation Schema。build-manifests.ts 写 plugins/.generated 下的纯 JSON，
与 dist 分离避免自引用 hash；这两个目录均为构建产物。

P1.1c 新增 SDK manifest/catalog.ts/loader.ts、CLI command-schema.ts/run-arguments.ts，
scripts/generate-command-docs.ts 和只读 drift 回归；docs/builtin-commands.md
是生成文件，CLI 兼容/批处理协议见 [CLI v1](./cli-contract-v1.md)。Web/Desktop
既有 runtime adapters 和生成 loader 接入该 SDK 契约；G1 当时没有新增 G2/G3 module；当前 G2 模块见下节。
Turbo build outputs 覆盖 dist/.generated；固定 Chromium gate 串行执行所有文件，
相关缓存完整性与 runner 约束由 scripts/ci-contracts.test.ts 验证。

## G2 lifecycle progress

P1.2a uses one SDK lifecycle controller with registry-owned per-plugin queues.
Loader and LifecycleManager share load/activate/deactivate/unload ordering.
State transitions reject illegal edges; failed hooks retain their cause and
cleanup failures until explicit unload/reload recovery. Disabled instances are
reused without loading again. Removal requires completed cleanup.
P1.2b adds reactive command projection, generation leases, execution draining,
and cooperative load/activation/view/runner resource scopes. Enabled module state
is separate from a resident runner; only explicitly owned runner resources count
as running. Web GUI executions acquire the current registry instance. Updates
and removal drain accepted calls and clean resources; cleanup errors block removal.
P1.3a adds `packages/runtime-core`, `apps/runtime`, `packages/runtime-client`
and a fixed T1 `packages/plugin-runner`. Windows validation uses a current-user
ACL named pipe, Host-bound connection proofs and disposable profiles. Task facts
live in Rust; GUI/Node clients query the same runId. Receipts are distinct from
terminal execution results; foreground disconnect cancels, explicit background
jobs survive. This only evaluates pure built-in commands: side effects require
APPROVAL_REQUIRED. No user DB migration, production unattended execution, cold
start, new grants, third-party runner or OS sandbox is delivered. P1.5a adds Rust-derived wire/golden drift checks, stable refusal codes,
redacted events and actionable version/disconnection diagnostics. G2 is complete
for its declared Windows/T1/disposable-profile scope. Runtime/core Rust builds are uncached; generate:hosts
creates Rust-derived client DTO/schema and Desktop bindings before quality gates.
See [G2 acceptance](./validation/g2-runtime.md).

P1.5a: `bun run verify:runtime-contracts` compares generated types, JSON Schema
and full Manifest/wire fixtures without rewriting tracked artifacts. Both Rust
and TS validate the same fixtures/digests. Manifest fixture files use controlled
LF text; runtime package integrity retains actual build file hashes. Connection
loss never auto-resubmits;
a submit with a lost response marks acceptance unknown. Reconnect checks instance
identity; same-key retries preserve input/package/background/deadline. Diagnostic
exports allow only version/code/summary/action/acceptance status, excluding private
exception text and payloads. Desktop validation also binds the actual configured
origin. Production builds exclude the DEV validation panel. Durable restart
recovery, user data/grants and independent CLI distribution remain G3 scope.
Validation must use the metadata-only native preflight before launching a Host
or GUI; literal identifier strings cannot establish the compiled identity.
Explicit validation refuses incorrect native identity before plugins/database IO.

## Workspace change delivery

`build:packages` includes the Runtime client required by Desktop and the
`apps/ui-test` native harness. The harness declares its workspace dependency and
uses public exports. `.gitattributes` pins the three Rust-derived Runtime source
artifacts to LF so fresh Windows checkout and codegen agree.

Completed workspace tasks follow the automatic feature-branch push and PR
workflow authorized on 2026-10-05 in [AGENTS.md](../AGENTS.md). Preserve applicable
quality gates and focused commits, and return the PR link and current status.
Merging, deployment and repository settings require separate authorization.

## G3 P2.3a capability broker foundation

The validation Runtime now uses a T0-owned T1 policy broker with Host-bound
caller/package/command identity, separate effects and operation scopes, revocation
epochs, expiry and per-run call budgets. Runner sessions are opaque, checked before
launch and during execution, and cleaned on cancellation/completion. Rust-derived
operation descriptors and rejection codes have cross-language schema fixtures.

Approvals are transient Rust Host APIs, absent from the wire. Pure T1 validation
continues; sensitive IO and ordinary Runtime startup remain denied. Persistent
grants, shared user data, GUI/CLI migration and independent distribution remain
pending G3 subitems. Existing Desktop/Web adapters are not replaced by this step.
File/tool descriptors are not IO implementations; network origin checks are not
DNS/redirect enforcement. Maturity stays prototype and SEC risks remain open.
Evidence: [P2.3a scope and validation](./validation/g3-capability-broker.md).

## G3 P2.6a shared data foundation

Runtime owns one SQLite writer with versioned migrations, backup/recovery,
plugin namespaces, revision/CAS and atomic transactions. The asynchronous
`@flowtools/sdk/data` API and Runtime client share that writer; they expose no
raw SQL, namespace or file path. Legacy Todo sources require explicit validated
import and remain preserved. Desktop Debug startup no longer deletes its DB;
corrupt/unsupported data fails closed. Dedicated native validation connects the
actual WebView and CLI to the same data. Normal user UI integration, durable
grants/jobs and independent distribution remain subsequent G3 subitems.

Evidence: [P2.6a scope and validation](./validation/g3-shared-data.md).

## G3 P2.4a persistent grants and CLI management

The single Runtime DB persists version/hash-bound grants, revocation epochs,
bootstrap policy and bounded metadata-only policy audit. CLI-only interactive
init or explicit policy import uses a Host-bound management role; management
mode cannot run plugins. Private Windows profiles and inherited credentials
require the current-user protected ACL. Package changes/rollback never restore
old grants. Independent distribution and ordinary GUI/CLI execution integration
remain the next G3 subitems; maturity stays prototype.

Evidence: [P2.4a validation and boundaries](./validation/g3-persistent-grants.md).
