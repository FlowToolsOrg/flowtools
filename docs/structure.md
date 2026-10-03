# Flow Tools 架构设计规范

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

- `packages/sdk/src/execution/executor.ts` / `test/executor.test.ts`：共享真实
  `run()` 执行边界，校验/defaults、稳定 envelope、取消/异步等待上限与资源清理。
  输入摘要只保留类型和大小；宿主接入在 P0.2b 验收，执行器不是 sandbox。

- `scripts/docs-check.ts` / `docs-check.test.ts`：十份核心/ADR/威胁/PR 文档的
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
- `packages/cli/src/*.test.ts`：CLI 参数、schema、formatter 与 runner。
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

Desktop (基于 Tauri): 已有 `apps/desktop` 壳，用于承载 HeroUI + Tailwind 桌面启动器、TanStack Router 桌面路由、React/SDK 插件面板、HTML 插件目录和 iframe 插件运行容器。原生能力通过官方 Tauri plugins 安装，再由 desktop SDK adapter 暴露为标准 capability。

Desktop 插件元数据逐步迁移到 Rust 后端。`apps/desktop/src-tauri/src/models/plugin.rs`
定义插件记录，`repositories/plugin_repository.rs` 负责数据库增删改查、启用/禁用和基础校验，`commands/plugin_commands.rs` 暴露 Tauri IPC。持久化的 `state`
与 SDK registry 使用同一组状态值。

### HTML 插件兼容导入层

- `packages/sdk/src/compat/html-plugin.ts`: HTML `plugin.json` 类型、命令解析、兼容支持分级。
- `scripts/inspect-html-plugins.ts`: 扫描本地 HTML 插件 checkout，生成 runtime 目录和 docs 副本；同时记录静态资源目录并跳过源码态 Vite HTML 入口。
- `apps/desktop/src/data/html-plugin-catalog.json`: desktop 启动器直接读取的 HTML 插件目录。
- `docs/html-plugin-catalog.json`: 同源的人类可读目录副本，供后续插件市场/导入器参考。
- `apps/desktop/src/runtime/desktop-capabilities.ts`: 将 SDK `fs/network/clipboard/dialog/notification/storage/db/native` capability 映射到 Tauri plugins 或 WebView API。
- `apps/desktop/src/runtime/html-plugin-bridge.ts`: 在 HTML iframe 中注入旧版宿主 API，并通过 `postMessage` 回到 desktop SDK runtime。

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
