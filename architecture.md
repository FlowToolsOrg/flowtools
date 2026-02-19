# Flow Tool Architecture

Flow Tool 是一个插件驱动的工具平台（Toolbox Platform），核心目标是：

- 用最小的宿主内核提供统一 UI 与能力（capabilities）
- 将大量功能以“本地安装插件”的形式交付
- 支持两种插件形态：即时工具（Tool）与长期应用（App）
- 插件不能直接调用 Tauri/Node/native，只能通过受控 SDK 能力访问

> 关键词：Capability Injection / Plugin Runtime / Permission Gating / Namespacing / Single React Tree

## 1. Non-Goals（明确不做）

- 不提供 iframe 沙箱隔离
- 不支持运行时动态编译插件（插件必须预构建）
- 不支持插件直接编写并动态注册 Rust commands（native 能力由宿主提供）
- 不追求“执行不可信插件”的强安全模型（插件需受信任；安全依赖审核/签名/权限提示等）

## 2. Core Decisions（已确定的关键决策）

- 插件安装方式：本地安装（plugins 目录）
- 运行模型：同线程（与宿主同一 JS runtime）
- UI：全 React；支持无 UI（headless）
- React 树：单 React Root（所有插件共享同一 React 树）
- 插件能力访问：通过 SDK hooks / ctx 注入，禁止直接访问 Tauri API
- 权限模型：声明式 permissions，运行时能力裁剪（capability gating）

## 3. Plugin Taxonomy（插件物种）

插件分为两种类型，运行模型不同，必须在第一天就区分。

### 3.1 Tool Plugin（即时型工具）

- 默认无长期状态
- 可以无 UI（headless）
- 执行即结束：run → return → destroy context
- 典型：sitemap 提取、hash 计算、格式转换

### 3.2 App Plugin（长期型应用）

- 可提供 UI 面板
- 可使用 store / db / settings（由宿主管理与隔离）
- 生命周期更长：load → mount UI → background → unmount UI（插件仍可保持资源）
- 典型：书签管理、剪贴板历史、任务管理

## 4. System Layers（系统分层）

[ Plugins ]
↓ use @flow-tool/sdk (types + definePlugin + hooks)
[ Runtime ]

- loader / registry / lifecycle / permissions / command system
- creates ctx (capability instances)
- mounts plugin UI under Provider (single React tree)
  [ Host (Desktop/Web) ]
- implements capabilities (fs/db/network/native bridge)
  [ Native (Rust/Tauri) ]
- provides high-performance primitives via invoke/commands

依赖方向必须单向：
plugin → sdk → runtime → capabilities → native

## 5. Plugin Contract（插件契约）

插件通过 `definePlugin()` 声明。`definePlugin` 是契约入口，非组件本身。

### 5.1 App Plugin Contract

- `setup()` 返回 React Function Component（Panel）
- React hooks 从 `react` 导入使用
- 平台能力通过 `@flow-tool/sdk` 提供的 hooks 获取（内部读 runtime context）

示例：

```ts
export default definePlugin({
  type: "app",
  meta: {...},
  setup() {
    return function Panel() {
      const fs = useFS()
      return (...)
    }
  }
})
```

### 5.2 Tool Plugin Contract

- `run(ctx, input)` 执行
- 无 React provider，因此必须显式使用 ctx
- 支持返回结构化输出，供宿主展示结果/复制等

示例：

```ts
export default definePlugin({
  type: "tool",
  meta: {...},
  commands: {...},
  async run(ctx, input) { ... }
})
```

## 6. Command System（命令系统）

命令是插件对外暴露的“唯一入口”（Single Entry Principle）。

- 所有插件必须通过 commands 暴露能力
- 命令支持两种 mode：
  - `panel`: 打开 UI 面板
  - `headless`: 执行并返回结果

最小 command schema：

- id（命令 id）
- title（展示名）
- mode（panel/headless）
- handler（render/run）

宿主提供统一 Command Palette（类似 uTools/Raycast）：

- 搜索命令
- 触发命令
- 记录历史
- 绑定快捷键（后续）

## 7. Runtime Context（ctx）与 SDK Hooks

### 7.1 ctx 的定位

ctx 是 runtime 内部对象，代表“插件能力实例集合”。插件 UI 不直接接触 ctx，通常通过 hooks 获取能力。

ctx 的职责：

- 权限裁剪：按 permissions 注入能力
- 命名空间隔离：store/db/cache 都与 pluginId 绑定
- 多平台适配：desktop/web 的实现不同，但 ctx contract 不变

### 7.2 SDK hooks 的原理

SDK hooks（如 useFS/useDB）是桥接函数：

- 运行时：从 React Context 读取 ctx
- 类型：提供 TS 类型提示
- 实体能力：来自 runtime 注入的 ctx.fs/ctx.db

> useFS 不是“只有类型”，而是运行时读取入口。

## 8. Permissions（权限模型）

插件在 meta 中声明 permissions：

- 例如：["fs", "network", "db", "clipboard", "dialog", "notification", "storage"]

运行时创建 ctx 时按权限裁剪：

- 允许：注入真实 capability 实例
- 不允许：注入 denied proxy（访问即抛错）

后续可扩展：

- 首次运行弹窗授权
- 权限细粒度（fs.read vs fs.write）
- 用户可在设置中撤销

## 9. State Model（Store）

原则：插件不能随意创建全局 store，平台托管 store。

- App 插件可使用 `usePluginStore()` / `ctx.store` 申请 scoped store
- store 以 pluginId 命名空间隔离
- 支持持久化（future）：
  - desktop: sqlite
  - web: indexedDB/localStorage

Tool 插件默认不提供 store。

## 10. Database Model（DB）

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

## 11. Native Performance（Rust/Tauri）

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

## 12. Stability & Recovery（稳定性策略）

由于同线程无沙箱，必须制定稳定性策略：

- 命令执行 watchdog（超时提示）
- 长任务建议走 native（Rust）或可取消的异步
- 插件错误隔离（try/catch + error boundary）
- 性能预算：对渲染频繁组件做 memo、虚拟列表、分片计算

## 13. Packaging（插件打包原则）

- 插件必须预构建为 ESM
- external 依赖必须固定：
  - react / react-dom
  - @flow-tool/sdk
  - @flow-tool/ui

- 禁止插件直接依赖宿主内部私有包
- 插件包可携带 assets（icons, wasm 等）

## 14. Future Extensions（未来扩展点）

- Web runtime：能力适配（fs/db/native）
- Plugin marketplace：安装、评分、更新、签名
- 权限 UI：授权提示、细粒度控制
- Tool 插件 worker 化（隔离计算）
- 自动化系统：定时任务、监听剪贴板、文件监控等
