# Flow Tool Architecture

Flow Tool 的产品方向是 **desktop-first（Tauri）** 的插件化工具平台。
当前仓库代码处于“先验证插件运行时，再落地桌面宿主”的阶段。

- 目标：以最小宿主内核 + 可扩展插件生态交付能力
- 当前机制：Capability Injection / 声明裁剪 / 内置插件共享 React Tree
- 生产目标：按信任等级隔离执行，Rust broker 强制验证身份、grant 与 scope
- 插件形态：`app`（长期 UI）与 `tool`（即时执行）

> 关键词：Capability Injection / Plugin Runtime / Permission Gating / Namespacing / Single React Tree

## 1. Reality Check（当前实现状态）

截至 2026-10-04，仓库中的实现状态（prototype）：

- 已实现：
  - `packages/sdk`：插件契约、hooks（工厂模式）、runtime provider、结果类型、Zod-based `inputSchema`
  - `packages/sdk/src/registry`：PluginRegistry、CommandRegistry、PluginLoader、PluginLifecycleManager、PluginErrorBoundary、withWatchdog
  - `packages/ui`：共享 UI 组件库（HeroUI 基础），含 CommandPalette 组件
  - `packages/cli`：统一 CLI 入口，`flowtools list/info/run` 子命令，Zod schema 自动生成 flags，`result.*` 结构化输出
  - `apps/web-vite`：web runtime 原型，含命令面板、插件注册中心、bootstrap 启动流程
  - `apps/desktop`：Tauri desktop 壳，当前提供 HeroUI + Tailwind 启动器界面、React/SDK 插件面板、HTML 插件 iframe runner，并用 TanStack Router 承载桌面路由
  - `apps/web-vite/src/stores`：pluginRegistryStore、commandStore、runHistoryStore、settingsStore
  - `packages/sdk/src/compat/html-plugin.ts`：HTML `plugin.json` 类型、命令解析和兼容分级
  - `scripts/inspect-html-plugins.ts`：扫描本地 HTML 插件 checkout 并生成 `apps/desktop/src/data/html-plugin-catalog.json` 与 `docs/html-plugin-catalog.json`
  - 12 个内置插件（全部为 app 类型，均提供 `setup()` + `run()` + `inputSchema`）
- 目录已创建，尚未实现：
  - `apps/docs`
  - `apps/web`

结论：架构方向是 desktop-first，当前 web 原型已具备注册中心、命令面板、生命周期管理、错误隔离等核心机制。
CLI 入口已就绪，桌面端可通过 `Command::new("flowtools")` 调用插件。HTML 插件兼容层已完成插件元数据与命令入口归一化，并在 desktop 端提供 React/SDK panel 渲染、HTML `main` iframe 启动容器、旧版宿主 API bridge，以及基于 SDK capability contract 的 Tauri 官方插件适配。

### 当前自动化验证边界

P0.3a1 在 SDK types 定义 maturity schema/type（prototype/experimental/beta/
production），缺失 metadata 仅默认 prototype；compatibility evidence 独立。
12 个内置 meta、Web/Desktop manifest、CLI list/info JSON/text 同步声明 prototype。
生成器按排序 inventory 同时更新两端；Desktop 补显式 plugins workspace 依赖，
根 Turbo 的依赖 tests/build 顺序随真实 dependency graph 推导。该标签不是签名、
API 认证或 grant，不能据此宣称 P0 done。

P0.3a2 Catalog formatVersion 1 使用 logical source、package identity/相对资源和
manifest/entry SHA-256；不保存 checkout root、development URL 或源态 main。
SDK 无 React 的 `compat/catalog` 子入口与只读 `verify:plugin-catalog` 检查结构、
身份、路径、两份目录及 fixture digest。125 项均为 prototype：47 项仅证明扫描
时入口文件存在（entry-resolved），78 项 indexed。没有 API/平台/签名认证。
Desktop 消费同一 schema；只有显式 DEV + VITE_HTML_PLUGIN_ROOT 才解析本地资源。
这不是安装、完整包依赖验证、运行时隔离、TOCTOU 防护或用户授权。

P0.3a3 共享 UI ToolStatus/ToolMarketStatus 直接复用 SDK PluginMaturity，卡片、
列表与详情的缺省标签为 Prototype。Web 从 manifest 读取，Desktop 从实际 built-in
meta/Catalog 读取；独立 compatibility badge 不升级 maturity，桥接需求不是 API
认证。状态仅在 render 中派生，没有另建持久状态或 effect 同步副本。
专用 Windows 验收以根路由查询标记进入首页，Tauri MockRuntime 回归覆盖最终
URL；实窗、开发模式与独立包验收分别记录，Mock 不启动宿主或用户数据库。

P0.3b1 将普通 SDK PluginFileLoader 改为执行前一律拒绝，移除普通入口的转译与
import-map 导出。危险实现分离到 `@flowtools/sdk/development`，只接受 Host 构建
的 DEV + `VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW=1`；Web 在同一条件下动态导入，
生产 bundle 不包含它。Web 文件/enable/reload 服务在副作用前拒绝外部入口，
IndexedDB 源码在所有模式都不自动恢复、不静默删除；内置 compiled manifest 的
加载与运行保留。开发预览仍共 realm、无签名/隔离/grant，仅限受控测试数据。
CLI 与 Desktop 关闭分别由 P0.3b2/b3 验收；SEC-002 仍 open，不能由此关闭整体风险。

P0.2a 已交付 `packages/sdk/src/execution/executor.ts`：`executePlugin()` 对
真实 app/tool `run()` 统一 schema/defaults、版本/时间戳/耗时、输入形状摘要、
稳定失败码、取消及异步超时。21 个新增回归覆盖校验期取消竞争、迟到结果和
timer/listener 清理；SDK 初始全部 61 个测试通过。
P0.2b1 已将 CLI runner 接入 SDK；JSON 输出变为共享 envelope，text formatter
保持原行为。CLI context 与 SDK 类型对齐，只提供已声明的内置插件 storage/network，
移除重复计时器与原始插件日志输出，storage 使用 `remove/zustand` 并拒绝路径键。
这不是持久用户 grant 或 canonical/symlink 隔离；源码/headless fallback 的生产
准入仍属 P0.3。CLI 33 tests、plugins 20 tests 包含真实编译后 CLI 的子进程
成功/校验失败/非法超时/text 回归。
P0.2b2 已接通 Web/Desktop app/tool 实际运行，共享 `ExecutionPanel` 并保留 app
panel。SDK `execution/history.ts` 使用 versioned external store，最多 200 条真实
尝试元数据；原始输入、输出和异常 message 不落历史，旧 key 原样保留但不导入。
不可序列化输出标记 OUTPUT_INVALID，不显示成功；卸载取消且忽略迟到 UI 更新。
SDK 67 tests、Chromium 50 tests 含实际异常/取消/卸载/序列化与恢复拒绝回归。
Windows Web production preview 和独立标识的真实 Tauri Debug WebView 通过
Base64、schema/JSON 拒绝、键盘运行、历史恢复与网络取消；截图已检查。详见
[宿主验收](./docs/validation/p0-execution-hosts.md)，不等同正式签名安装或跨平台验收。
P0.2c 的十二插件 smoke 逐项匹配 CLI inventory，通过真实编译入口/CLI SDK runner
调用 run()，网络返回与存储均为受控 capability。全部插件的 schema 拒绝和预取消
不产生新 request/storage 写入；实际 Todo 损坏数据及缺少 network 的异常跨端
返回 EXECUTION_FAILED。Todo run 优先使用面板共享 app store，CLI 保留并验证旧
todos key；没有自动合并旧 namespace。网站延迟 run/panel 使用 SDK request，
不再直接 fetch；这是可信 built-in 修正，不阻止同 realm 恶意代码自行请求。
该函数不提供 OS 隔离，不终止同 realm 的同步循环，也不撤销已发生的副作用；
旧 `withWatchdog` 的 cooperative-only 行为未被升级或重新宣称为强制终止。

安全设计与现状以 [ADR-0001](./docs/adr/0001-plugin-trust-boundaries.md)、
[ADR-0002](./docs/adr/0002-capability-and-package-policy.md) 和
[威胁模型](./docs/security/threat-model.md) 为准。T0/T1/T2/T3/TL 的目标边界已
固定，但独立执行、持久 grant、签名准入及恢复仍待实现；不得以文档或测试
通过推断这些控制已经生效。

根 `bun run test` 调用七个 workspace 的真实测试。SDK/CLI 覆盖核心契约与失败
路径；UI package 先构建再验证公开导出和独立消费者声明；plugins 使用构建时相同
的目录 inventory，并通过 CLI discovery、loader、schema 和 runner 验证执行链路；
Web Host 覆盖 app/tool 命令映射与非法 runtime 类型拒绝；UI consumer 在固定版本
Chromium 中验证组件交互；Rust repository 使用内存数据库验证隔离与错误路径。
Desktop 测试任务先验证 Cargo 测试清单非空，避免零测试返回成功。

Web manifest 元数据逐插件验证真实构建入口，冷加载与批量注册集成用例显式限定
30 秒；普通单元测试保留默认超时。加载错误、契约不匹配或超时仍失败，不自动
重试。这个 runner 容量预算不构成插件启动性能 SLA。

Turbo 的 `test` 依赖 `^test`，不缓存结果；package 合约测试自行构建产物，
消费者等待依赖测试完成，避免读取正在被清理的 `dist`。根 `test` 与 `build`
必须顺序执行，不能并发运行；单独的 `build` 图仍依赖 `^build`。
Turbo 保留严格环境模式，显式透传 Windows `PATHEXT` 和 build/test 使用的
`CARGO_TARGET_DIR`，避免 PowerShell 命令发现失败及原生缓存路径被过滤。
测试类型与浏览器生产类型隔离；这些验证不等同于
第三方插件隔离、签名安装或生产兼容认证，仍按生产路线图分阶段交付。

### Windows PR 质量执行链

依赖安装后首先验证 workspace 与 `docs:check` 文档契约。后者只检查核心文档、
ADR 目标/现状分离、威胁必填项、源码链接路径和 PR 安全评审字段；不验证运行时
隔离、远程 URL/anchor 或 reviewer 批准。capability/bridge 变更须在 PR 中给出
对应威胁 ID、scope/身份设计与拒绝路径证据，管理员强制评审配置另行验收。

`windows-quality.yml` 安装 packageManager 指定的 Bun 与 Rust 1.96.0，然后
调用 `scripts/check-ci.ps1`。该入口先 frozen install、安装固定 Chromium、
构建 package 声明消费产物、生成 Web/Desktop routes 与 Rust bindings，再顺序
执行七 workspace 的 lint/types/test/build
与 Rust fmt/check/clippy。Turbo gate 强制重新执行，缓存仅用于依赖下载及原生
编译；每条原生命令显式检查退出码，失败立即终止，工作树漂移仍由 always 步骤
检查。工作流使用只读权限和 Action SHA；远端成功与 required check 启用需独立
验收，不能由本地构建成功推断。

Desktop 的 runtime 与生成器共享命令注册构造器。`codegen` feature 下的
`export-bindings` console binary 使用 MockRuntime，只导出类型，不启动 WebView
或用户数据库；生产默认构建不包含该 binary。Windows build script 显式加入
Common Controls v6 manifest，覆盖 Tauri 应用资源未覆盖的 binding integration
test；linker directives 限定于 test target，避免与应用已有 manifest 重复。

## 2. Layered Model（分层模型）

```text
[ Plugins ]
  -> use @flowtools/sdk
[ CLI & Desktop Agent ]
  -> packages/cli (Commander) / Tauri Command bridge
[ Registry & Lifecycle ]
  -> PluginRegistry / CommandRegistry / PluginLoader / LifecycleManager
[ Runtime (Host App) ]
  -> ctx factory / permission gating / error boundary / watchdog
[ Capability Adapters ]
  -> fs / request / storage / native bridge
[ Platform ]
  -> Web APIs today, Tauri+Rust in target desktop host
```

依赖方向保持单向：

`plugin -> sdk -> cli/registry -> host runtime -> capability adapter -> platform`

## 3. Plugin Contract（SDK 契约）

插件统一通过 `definePlugin(...)` 声明。

- `type: 'app'`
  - 必须提供 `setup()`
  - `setup()` 返回 React 组件，由宿主渲染
  - 可选提供 `run(ctx, input)`，使 app 插件可被 CLI / 桌面 agent 无头调用
- `type: 'tool'`
  - 必须提供 `run(ctx, input)`
  - 无 UI 依赖，直接执行并返回结果

所有插件可选提供 `inputSchema`（Zod `z.object({...})`）：

- CLI 通过 `z.toJSONSchema()` 自动生成 `--flag` 参数定义
- 运行时用 `.safeParse()` 校验输入
- `z.infer<typeof inputSchema>` 推导 TypeScript 类型

`run()` 推荐返回 `result.*` 结构化结果（`@flowtools/sdk/result`）：

- `result.text(string)` / `result.json(value)` / `result.table(cols, rows)`
- `result.open(target)` / `result.multi(items)`

### 3.1 HTML Plugin Compatibility Contract

HTML 插件先通过 `normalizeHtmlPluginManifest(...)` 转为
`FlowToolsHtmlPluginManifest`，保留原始 `main`、`preload`、`features`、`cmds`
信息，并生成 FlowTools 可搜索的命令描述。

兼容分级：

- `webview`：有 `main` 或 `development.main`，可优先由 Tauri WebView 承载。
- `preload-bridge`：声明了 `preload`，需要实现旧版宿主 API。
- `native-bridge`：包含 `files`、`img`、`window` 等命令类型，需要 Tauri/Rust 原生能力。
- `metadata`：没有 UI 入口，可先作为索引或重写为 headless/tool 插件。

`bun run inspect:html-plugins` 会扫描本地 HTML 插件 checkout，输出插件兼容目录，供
desktop 启动器、插件市场和后续导入器复用。preload 类 HTML 插件会声明
`storage/native/clipboard/fs/network/notification/dialog/db` 等权限，再由 desktop
runtime 按声明裁剪 SDK capability。
扫描器会记录 HTML 插件的实际静态资源目录，并检测源码态 Vite HTML 入口；当
`main` 在当前 checkout 中不可直接运行时，desktop runner 会优先使用
`development.main`，避免把插件源码里的 `/src/main.ts` 解析到 FlowTools 自己的
Vite dev server 上。

`definePlugin` 会写入 `Symbol('__flow_tools__')` marker，并在类型层限制
`meta.permissions` 的重复声明（tuple 字面量可在编译期发现重复权限）。

## 4. Runtime Context 与 Hooks

ctx(context) 是 runtime 内部对象，代表“插件能力实例集合”。插件 UI 不直接接触 ctx，通常通过 hooks 获取能力。

ctx 的职责：

- 权限裁剪：按 permissions 注入能力
- 当前命名空间：storage/store key 与 pluginId 关联；DB raw query 尚未隔离
- 多平台适配：desktop/web 的实现不同，但 ctx contract 不变

`@flowtools/sdk` 暴露：

- Provider：`FlowToolRuntimeProvider`
- 通用能力：`useCapability(selector?)` — 支持 selector 模式按需取能力
- Plugin Store：`usePluginStore(selector?)` / `usePluginStoreApi()`
- 专用能力 hooks：`useEnv`、`useUI`、`useFS`、`useRequest`、`useStorage`、`useDB`、`useClipboard`、`useDialog`、`useNotification`、`useNative`
- 工厂函数：`createRequiredCapabilityHook(key)` / `createOptionalCapabilityHook(key)`
  - 专用能力 hooks 由工厂函数生成，插件也可自行创建自定义 hook

运行时通过 context 注入 `PluginRuntimeContextValue`：

- `env`：`pluginId`、`pluginType`、`platform`、`mode`
- `ui`：toast/openPanel/closePanel
- 可选能力：`fs/request/clipboard/dialog/notification/storage/db/native`
- `utils.now()`

若插件调用了未注入能力，对应 hook 会抛出 `MissingCapabilityError`。

## 5. Permission Model（权限模型）

权限集合（SDK）当前为：

- `fs`
- `network`
- `clipboard`
- `dialog`
- `notification`
- `storage`
- `db`
- `native`

Host runtime 用 `pickCapability(...)` 做权限裁剪：

- 已声明权限 -> 注入 capability 实现
- 未声明权限 -> 对应 capability 为 `undefined`

这是合作代码的 API 裁剪，不是用户 grant 或恶意代码边界。当前同 realm
外部代码可绕过 SDK，Desktop 原生 adapter 仍提供通用 invoke/raw SQL。
生产要求见 ADR-0002：由 Host 会话绑定 package identity，每次 Rust 操作重新
验证声明、持久 grant、scope 与撤销状态，拒绝不能只发生在前端。

## 6. Web Runtime Prototype（当前宿主实现）

`apps/web-vite/src/runtime/ctx.tsx` 提供当前 web 适配。

能力映射如下：

- `ui`：通过 `window.dispatchEvent` 发事件
  - `flowtools:toast`
  - `flowtools:panel-open`
  - `flowtools:panel-close`
  - `flowtools:tool-log`
- `network`：直接映射到浏览器 `fetch`
- `storage`：`localStorage`（key 前缀 `flowtools:{pluginId}:storage:`）
  - 提供 `storage.zustand(namespace?)` 适配器，供插件侧
    Zustand `persist/createJSONStorage` 使用
- `store`：host 管理的 Zustand vanilla store（每个 pluginId 单实例）
  - app 插件通过 `usePluginStore()` / `usePluginStoreApi()` 访问
  - 插件通过 `definePluginStore()` 声明 store 形态（初始状态 + actions），挂在 `AppPlugin.store`
  - 若声明了 `storage` 权限，store 会持久化到插件命名空间
- `fs`：`localStorage` 模拟文件（key 前缀 `flowtools:{pluginId}:fs:`）
- `clipboard`：浏览器 clipboard API
- `notification`：Notification API（不可用时降级为 toast 事件）
- `dialog`：web 未实现，调用抛错
- `db`：web 未实现，调用抛错
- `native`：web 未实现，调用抛错

## 6.1 Desktop Runtime（Tauri）

`apps/desktop` 当前是 Tauri 桌面壳，不再是默认模板页。它读取
`apps/desktop/src/data/html-plugin-catalog.json`，并合并内置 React/SDK 插件 manifest，展示插件启动、搜索、命令数量、分类和运行支持级别；`docs/html-plugin-catalog.json` 作为同源的人类可读目录副本。桌面端路由使用 TanStack Router，当前包括 `/`、`/settings`、`/plugins`、`/permissions` 和 `/run/$commandId`。其中 `/run/$commandId` 对 React/SDK app 插件会直接渲染 panel，对声明了 `main` 的 HTML 插件会启动 iframe 运行容器。

当前边界：

- 已实现：HeroUI + Tailwind 桌面首屏、React/SDK 插件和 HTML 插件共同驱动的启动器界面、设置/插件/权限路由、HTML `main` iframe 启动容器、workspace icon 复用。
- 已实现：Desktop SDK capability adapter，按插件 permissions 暴露 `fs/request/clipboard/dialog/notification/storage/db/native`。
- 已实现：通过 `bun tauri add` 安装并接入官方 Tauri 插件：`fs`、`dialog`、`clipboard-manager`、`notification`、`sql`、`store`、`opener`。
- 已实现：iframe 注入旧版宿主 API bridge，将常用旧 API 转回 SDK capability，再由 Tauri plugin 或 WebView API 执行。
- 已实现：Tauri/Rust 后端持久化插件元数据，提供 `get_plugins`、`get_plugin`、`add_plugin`、`update_plugin`、`enable_plugin`、`disable_plugin`、`remove_plugin` 命令；数据库中的 `state` 与 SDK registry 状态词保持一致。
- 待完善：更完整的旧版桌面 API 面覆盖、截图/窗口控制、插件安装与细粒度授权提示。

### 6.2 Desktop Plugin Metadata Database

`apps/desktop/src-tauri/src/models/plugin.rs` 定义桌面端插件元数据表。字段对齐
`PluginManifestEntry` / `PluginMeta` 的常用前端属性，并额外记录持久化
`state` 与 `created_at`。Rust repository 层负责 kebab-case `id`、`app/tool`
类型和 registry state 的输入校验。

新增或修改插件元数据时，前端通过 Tauri Specta 生成的 binding 调用后端命令；
后端返回 `PluginDto`，再由 desktop UI 同步到本地 registry/store。当前 schema
变更依赖 dev 模式重置数据库；发布环境需要单独补充版本化 migration。

## 7. Plugin Execution Flow

### 7.1 App Plugin

入口：`renderWebAppPlugin(plugin)`

1. 调用 `plugin.setup()` 获取 Panel 组件
2. 使用 `PluginErrorBoundary` 包裹（捕获渲染错误）
3. 使用 `WebPluginRuntimeProvider` 注入 runtime context
4. 在 React 树中渲染插件 Panel

### 7.2 Tool Plugin

入口：`runWebToolPlugin(plugin, input, options)`

1. 调用 `createWebToolContext(...)`
2. 注入 `signal` 与 `log`
3. 使用 `withWatchdog()` 包装（超时检测）
4. 执行 `plugin.run(ctx, input)` 并返回结果

### 7.3 CLI Execution Flow

入口：`bun run packages/cli/src/cli.ts run <plugin-id> [flags]`

1. `scanPlugins()` 扫描 `plugins/` 目录，检测 `run()` 和 `inputSchema`
2. 动态 `import()` 加载目标插件
3. 若有 `inputSchema`：
   - 解析 CLI flags → key-value 对象
   - 用 `z.safeParse()` 校验，失败则输出错误并退出
4. 若无 `inputSchema`：使用 `--input <json>` 传入原始 JSON
5. `runPluginAndPrint()` 创建 CLI ToolContext，执行 `plugin.run(ctx, input)`
6. `formatOutput()` 根据 `--format json|text` 输出结果
7. 返回 exit code（0 成功，1 失败）

### 7.4 Bootstrap Flow

应用启动时 `bootstrap(navigate)` 执行：

1. 创建 `PluginRegistry`、`CommandRegistry`、`PluginLoader` 实例
2. 注册所有内置插件清单（`builtInManifests`）
3. 加载并启用所有插件（`loadAll()` → `enableAll()`）
4. 初始化 `pluginRegistryStore`（Zustand 响应式）
5. 自动注册命令到 `CommandRegistry`
6. 同步状态到 UI

## 8. Commands 与 Command Palette

SDK 已定义命令与结果契约：

- Command mode：`panel` / `headless`
- `CommandDef`：统一描述命令入口
- `result` helpers：`text/json/table/open/multi`

### CommandRegistry

`packages/sdk/src/registry/command-registry.ts` 提供命令注册中心：

- 每个插件自动注册为命令（app → panel 命令，tool → headless 命令）
- 插件的 `commands` 字段中的命令也会被注册
- 支持搜索（匹配 title、description、keywords）
- 支持最近执行记录（最多 5 条）

### Command Palette UI

`packages/ui/src/components/command-palette/` 提供命令面板组件：

- 居中弹窗，`Cmd/Ctrl+K` 唤出
- 实时搜索过滤
- 键盘导航（`↑↓` 选择，`Enter` 执行，`Esc` 关闭）
- 最近执行的命令置顶

宿主集成在 `apps/web-vite/src/routes/__root.tsx`，
通过 `commandStore` 连接 `CommandRegistry`。

## 9. Lifecycle & Error Isolation（生命周期与错误隔离）

### PluginLifecycleManager

`packages/sdk/src/registry/lifecycle-manager.ts` 管理插件生命周期：

- `load()` → 调用 `onLoad`
- `unload()` → 调用 `onUnload`
- `activate()` → 调用 `onActivate`
- `deactivate()` → 调用 `onDeactivate`
- `isHealthy()` → 检查插件是否处于健康状态

状态机：`registered → loaded → enabled ↔ disabled`

### PluginErrorBoundary

`packages/sdk/src/registry/plugin-error-boundary.tsx` 捕获插件渲染错误：

- 包裹每个 app 插件的 Panel 渲染
- 捕获 React 渲染异常，显示降级 UI
- 提供"重新加载"按钮恢复

### withWatchdog

`packages/sdk/src/registry/watchdog.ts` 包装 tool 插件执行：

- 默认 30 秒超时
- 超时后触发 AbortSignal
- 提供 cooperative abort；不能终止同步循环或忽略 signal 的任务

## 10. Non-Goals / Current Limits

当前版本的缺口（不是生产非目标）：

- 没有经过安全认证的第三方隔离；现有 HTML iframe 不是生产 sandbox。
- Web 文件 loader 可在 Host realm 转译并执行 TS/TSX；生产目标禁止该路径。
- 没有强制 Rust plugin identity/grant broker，不能阻止同 realm 绕过 SDK。
- 生产准入、签名安装、scope 隔离与恢复尚未完成。

v1 的显式非目标：不宣称兼容全部 125 个 HTML 插件；不提供任意 shell、
raw invoke/SQL/绝对路径、外部 native binary 或 Node/Electron 私有 API；
不承诺抵抗已控制 OS/Host Rust 的攻击者。T1 可共用 React 树，T2/TL 必须独立
origin/session，T3 必须具备受限、可终止执行边界。普通 Worker/subprocess
不能自动获得 sandbox 资格，未验证的平台应拒绝第三方执行。

## 11. Path to Desktop (Tauri)

下一阶段应继续扩展 desktop host：

1. 新增 `apps/desktop` 作为主宿主
2. ~~将 `dialog/db/native/fs` 等能力切换到 Tauri + Rust 实现~~ ✅ 已通过官方 Tauri plugins + SDK adapter 建立基础实现
3. ~~建立命令注册中心（palette、历史、快捷键）~~ ✅ 已实现
4. ~~CLI 入口（插件无头调用、AI agent bridge）~~ ✅ 已实现（`packages/cli`）
5. 接入插件安装/加载策略（本地安装、版本管理、签名/权限提示）
6. 桌面端 AI agent 通过 `Command::new("flowtools")` 调用 CLI，解析 JSON 输出

这一路径与当前 SDK 契约兼容，重点是 host capability 实现迁移。

## 12. Stability & Recovery（稳定性策略）

由于同线程无沙箱，已实施以下稳定性策略：

- **PluginErrorBoundary**：包裹 app 插件 Panel，捕获渲染错误并显示降级 UI
- **withWatchdog**：tool 执行超时检测（默认 30s），超时触发 AbortSignal
- **LifecycleManager**：插件生命周期钩子隔离，单个插件的钩子失败不影响其他插件
- **Registry 状态管理**：插件错误状态通过 `registry.markError()` 记录，不影响宿主

待实施：

- 长任务建议走 native（Rust）或可取消的异步
- 性能预算：对渲染频繁组件做虚拟列表、分片计算

## 13. Native Performance（Rust/Tauri）

原则：插件不直接写 Rust，native 能力由宿主提供为 capability。标准桌面能力优先
通过 `bun run --cwd apps/desktop tauri add <plugin-name>` 安装官方 Tauri plugin，
再在 `apps/desktop/src/runtime/desktop-capabilities.ts` 中适配到 SDK contract。

- Flow Tool 内置 native capability（host layer）
- 插件通过 `ctx.native.xxx` / `useNative()` 调用
- 高性能需求通过宿主扩展能力模块满足：
  - 音视频处理（ffmpeg wrapper）
  - 压缩/加密
  - 大文件扫描/索引
  - 高性能解析

未来可扩展（高级/谨慎）：

- sidecar 模型（插件携带二进制并 IPC 通信）
- 需要签名/权限/审核，属于平台 v2/v3

## 14. State Model（Store）

原则：store 由宿主创建和托管，插件声明 store 形态，宿主实例化运行时。

### 插件侧：`definePluginStore`

插件通过 `definePluginStore()` 声明 store 的初始状态和 actions：

```ts
import {
  definePluginStore,
  type InferStoreState,
  type InferStoreActions,
} from '@flowtools/sdk'

const todoStore = definePluginStore({
  initialState: {
    todos: [] as TodoItem[],
  },
  actions: (set, get) => ({
    addTodo(item: TodoItem) {
      set(state => ({ todos: [...state.todos, item] }))
    },
    removeTodo(index: number) {
      set(state => ({
        todos: state.todos.filter((_, i) => i !== index),
      }))
    },
  }),
})

// 推导类型，无需手动维护
type TodoState = InferStoreState<typeof todoStore>
type TodoActions = InferStoreActions<typeof todoStore>
```

`definePluginStore` 是纯类型函数，不产生运行时开销。store 声明挂在 `AppPlugin.store`：

```ts
export default definePlugin({
  type: 'app',
  meta: { ... },
  store: todoStore,
  setup() { ... },
})
```

### 插件消费：hooks

- `usePluginStore<TState>()`：读取响应式状态（无 selector 时返回完整 state）
- `usePluginStoreApi<TState, TActions>()`：获取 store API，通过 `actions.xxx()` 更新状态

```ts
const { todos } = usePluginStore<TodoState>()
const {
  actions: { addTodo, removeTodo },
} = usePluginStoreApi<TodoState, TodoActions>()

// 调用声明式 actions
addTodo({ todo: 'Buy milk', deadline: '2026-05-10' })
```

### 宿主侧：实例化

宿主在 runtime context 中为每个 `pluginId` 创建一个 zustand vanilla store：

- 从 `plugin.store` 读取 `initialState` 传给 zustand `createStore`
- 从 `plugin.store.actions` 工厂函数创建 actions 对象，注入 `set/get`
- 通过 `PluginStoreCapability<TState, TActions>` 暴露给 hooks

持久化由宿主控制，命名空间仍按 `pluginId` 隔离。持久化后端（current/future）：

- web: localStorage（当前）
- desktop: sqlite 或其他宿主管理存储（规划）

Tool 插件默认不提供 store。

## 15. Database Model（DB）

原则：插件不能直接操作 sqlite 连接，平台托管 db。

- 统一 SQLite 数据库（desktop）
- 当前插件 adapter 共享连接并接受 raw SQL，尚未强制 namespace 授权
- 表名策略：
  - `${pluginId}__${tableName}`
- 迁移策略（future）：
  - 插件版本与 migration 绑定
  - 安装/升级触发 migration
  - 卸载清理 namespace（可选）

Web 端 db：

- 可使用 indexedDB 适配层（future）

## 16. Future Extensions（未来扩展点）

- Web runtime：能力适配（fs/db/native）
- Plugin marketplace：安装、评分、更新、签名
- 权限 UI：授权提示、细粒度控制
- Tool 插件 worker 化（隔离计算）
- 自动化系统：定时任务、监听剪贴板、文件监控等
- 外部插件动态加载（`import()` URL）
- 插件 `package.json` 规范化（独立 npm 包）
- Run History UI（Tool Detail 页 History Tab）
- 插件间通信机制（事件总线 / RPC）
- CLI `--schema` 输出 JSON Schema（供桌面端动态 UI 生成）
- CLI `--watch` 模式（监听输入变化重新执行）
- CLI `flowtools create` 脚手架（交互式创建新插件）
