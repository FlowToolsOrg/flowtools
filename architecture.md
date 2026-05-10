# Flow Tool Architecture

Flow Tool 的产品方向是 **desktop-first（Tauri）** 的插件化工具平台。
当前仓库代码处于“先验证插件运行时，再落地桌面宿主”的阶段。

- 目标：以最小宿主内核 + 可扩展插件生态交付能力
- 核心机制：Capability Injection / Permission Gating / Single React Tree
- 插件形态：`app`（长期 UI）与 `tool`（即时执行）

> 关键词：Capability Injection / Plugin Runtime / Permission Gating / Namespacing / Single React Tree

## 1. Reality Check（当前实现状态）

截至 2026-05，仓库中的实现状态：

- 已实现：
  - `packages/sdk`：插件契约、hooks（工厂模式）、runtime provider、结果类型、Zod-based `inputSchema`
  - `packages/sdk/src/registry`：PluginRegistry、CommandRegistry、PluginLoader、PluginLifecycleManager、PluginErrorBoundary、withWatchdog
  - `packages/ui`：共享 UI 组件库（HeroUI 基础），含 CommandPalette 组件
  - `packages/cli`：统一 CLI 入口，`flowtools list/info/run` 子命令，Zod schema 自动生成 flags，`result.*` 结构化输出
  - `apps/web-vite`：web runtime 原型，含命令面板、插件注册中心、bootstrap 启动流程
  - `apps/web-vite/src/stores`：pluginRegistryStore、commandStore、runHistoryStore、settingsStore
  - 12 个内置插件（全部为 app 类型，均提供 `setup()` + `run()` + `inputSchema`）
- 目录已创建，尚未实现：
  - `apps/desktop`（Tauri 桌面宿主）
  - `apps/docs`
  - `apps/web`

结论：架构方向是 desktop-first，当前 web 原型已具备注册中心、命令面板、生命周期管理、错误隔离等核心机制。
CLI 入口已就绪，桌面端可通过 `Command::new("flowtools")` 调用插件。

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

`definePlugin` 会写入 `Symbol('__flow_tools__')` marker，并在类型层限制
`meta.permissions` 的重复声明（tuple 字面量可在编译期发现重复权限）。

## 4. Runtime Context 与 Hooks

ctx(context) 是 runtime 内部对象，代表“插件能力实例集合”。插件 UI 不直接接触 ctx，通常通过 hooks 获取能力。

ctx 的职责：

- 权限裁剪：按 permissions 注入能力
- 命名空间隔离：store/db/cache 都与 pluginId 绑定
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
- 防止长时间运行的 tool 阻塞宿主

## 10. Non-Goals / Current Limits

当前版本明确限制：

- 不做 iframe/worker 沙箱隔离（同线程模型）
- 不支持运行时编译插件（插件需预构建）
- 不允许插件直接调用宿主私有 API
- 不追求“运行不可信插件”的强安全模型

## 11. Path to Desktop (Tauri)

下一阶段应将 web 适配层替换/扩展为 desktop host：

1. 新增 `apps/desktop` 作为主宿主
2. 将 `dialog/db/native/fs` 等能力切换到 Tauri + Rust 实现
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

原则：插件不直接写 Rust，native 能力由宿主提供为 capability。

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
- 插件数据通过 pluginId namespace 隔离
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
