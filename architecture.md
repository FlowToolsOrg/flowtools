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
  - `packages/sdk`：插件契约、hooks（工厂模式）、runtime provider、结果类型
  - `packages/ui`：共享 UI 组件库（HeroUI 基础）
  - `apps/web-vite`：web runtime 原型（用于验证 SDK 与运行时模型）
  - `plugins/plugin-example-hello-world`：app 插件示例
  - `plugins/plugin-example-run-hello`：tool 插件示例
  - `plugins/plugin-todo-list`：带 host-managed store 的 app 插件示例
- 目录已创建，尚未实现：
  - `apps/desktop`（Tauri 桌面宿主）
  - `apps/docs`
  - `apps/web`

结论：架构方向是 desktop-first，但当前可运行宿主是 web 原型。

## 2. Layered Model（分层模型）

```text
[ Plugins ]
  -> use @flow-tool/sdk
[ Runtime (Host App) ]
  -> loader / ctx factory / permission gating
[ Capability Adapters ]
  -> fs / request / storage / native bridge
[ Platform ]
  -> Web APIs today, Tauri+Rust in target desktop host
```

依赖方向保持单向：

`plugin -> sdk -> host runtime -> capability adapter -> platform`

## 3. Plugin Contract（SDK 契约）

插件统一通过 `definePlugin(...)` 声明。

- `type: 'app'`
  - 必须提供 `setup()`
  - `setup()` 返回 React 组件，由宿主渲染
- `type: 'tool'`
  - 必须提供 `run(ctx, input)`
  - 无 UI 依赖，直接执行并返回结果

`definePlugin` 会写入 `__flow_tool` marker，并在类型层限制
`meta.permissions` 的重复声明（tuple 字面量可在编译期发现重复权限）。

## 4. Runtime Context 与 Hooks

ctx(context) 是 runtime 内部对象，代表“插件能力实例集合”。插件 UI 不直接接触 ctx，通常通过 hooks 获取能力。

ctx 的职责：

- 权限裁剪：按 permissions 注入能力
- 命名空间隔离：store/db/cache 都与 pluginId 绑定
- 多平台适配：desktop/web 的实现不同，但 ctx contract 不变

`@flow-tool/sdk` 暴露：

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
  - `flow-tool:toast`
  - `flow-tool:panel-open`
  - `flow-tool:panel-close`
  - `flow-tool:tool-log`
- `network`：直接映射到浏览器 `fetch`
- `storage`：`localStorage`（key 前缀 `flow-tool:{pluginId}:storage:`）
  - 提供 `storage.zustand(namespace?)` 适配器，供插件侧
    Zustand `persist/createJSONStorage` 使用
- `store`：host 管理的 Zustand vanilla store（每个 pluginId 单实例）
  - app 插件通过 `usePluginStore()` / `usePluginStoreApi()` 访问
  - 可通过 `meta.store.initialState` 提供初始状态
  - 若声明了 `storage` 权限，store 会持久化到插件命名空间
- `fs`：`localStorage` 模拟文件（key 前缀 `flow-tool:{pluginId}:fs:`）
- `clipboard`：浏览器 clipboard API
- `notification`：Notification API（不可用时降级为 toast 事件）
- `dialog`：web 未实现，调用抛错
- `db`：web 未实现，调用抛错
- `native`：web 未实现，调用抛错

## 7. Plugin Execution Flow

### 7.1 App Plugin

入口：`renderWebAppPlugin(plugin)`

1. 调用 `plugin.setup()` 获取 Panel 组件
2. 使用 `WebPluginRuntimeProvider` 注入 runtime context
3. 在 React 树中渲染插件 Panel

### 7.2 Tool Plugin

入口：`runWebToolPlugin(plugin, input, options)`

1. 调用 `createWebToolContext(...)`
2. 注入 `signal` 与 `log`
3. 执行 `plugin.run(ctx, input)` 并返回结果

## 8. Commands 与 Result Model

SDK 已定义命令与结果契约：

- Command mode：`panel` / `headless`
- `CommandDef`：统一描述命令入口
- `result` helpers：`text/json/table/open/multi`

说明：命令契约已经在 SDK 中可用，但当前 web host 仍以示例路由直挂插件为主，
尚未形成完整 command palette / 命令分发中枢。

## 9. Lifecycle State（现状）

`PluginLifecycle` 类型已定义（`onLoad/onUnload/onActivate/onDeactivate`），
但当前 host 原型尚未系统性调度这些生命周期钩子。

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
3. 建立命令注册中心（palette、历史、快捷键）
4. 接入插件安装/加载策略（本地安装、版本管理、签名/权限提示）

这一路径与当前 SDK 契约兼容，重点是 host capability 实现迁移。

## 12. Stability & Recovery（稳定性策略）

由于同线程无沙箱，必须制定稳定性策略：

- 命令执行 watchdog（超时提示）
- 长任务建议走 native（Rust）或可取消的异步
- 插件错误隔离（try/catch + error boundary）
- 性能预算：对渲染频繁组件做 虚拟列表、分片计算

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

原则：store 由宿主创建和托管，插件只消费 store hooks。

- App 插件通过 `usePluginStore()` 订阅状态，通过 `usePluginStoreApi()` 更新状态
- 每个 `pluginId` 只创建一个 store（host registry）
- 插件可在 `meta.store.initialState` 声明初始状态结构
- 持久化由宿主控制，命名空间仍按 `pluginId` 隔离
- 持久化后端（current/future）：
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
