# Flow Tools 架构设计规范

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

Desktop (基于 Tauri): 已有 `apps/desktop` 壳，用于承载 HeroUI + Tailwind 桌面启动器、TanStack Router 桌面路由、ZTools 风格插件目录和 iframe 插件运行容器。原生能力通过官方 Tauri plugins 安装，再由 desktop SDK adapter 暴露为标准 capability。

### ZTools 兼容导入层

- `packages/sdk/src/compat/ztools.ts`: ZTools `plugin.json` 类型、命令解析、兼容支持分级。
- `scripts/inspect-ztools-plugins.ts`: 扫描本地 `ZTools-plugins/plugins`，生成 runtime 目录和 docs 副本。
- `apps/desktop/src/data/plugin-catalog.ztools.json`: desktop 启动器直接读取的 ZTools 插件目录。
- `docs/plugin-catalog.ztools.json`: 同源的人类可读目录副本，供后续插件市场/导入器参考。
- `apps/desktop/src/runtime/desktop-capabilities.ts`: 将 SDK `fs/network/clipboard/dialog/notification/storage/db/native` capability 映射到 Tauri plugins 或 WebView API。
- `apps/desktop/src/runtime/ztools-bridge.ts`: 在 ZTools iframe 中注入 `window.ztools/window.utools`，并通过 `postMessage` 回到 desktop SDK runtime。

兼容级别：

- `webview`: 可优先进入 Tauri WebView 承载。
- `preload-bridge`: 需要补齐 `window.ztools` API bridge。
- `native-bridge`: 需要 Tauri/Rust 原生能力配合。
- `metadata`: 仅完成索引或适合重写为 FlowTools headless/tool 插件。

新增 Tauri 原生能力时，优先在 `apps/desktop` 内执行
`bun run tauri add <plugin-name>`，再把插件 API 适配到 SDK capability；避免让业务插件直接依赖 Tauri API。

## 6. 插件捆绑包结构 (Plugins Bundle)

Plugins Bundle Metadata (捆绑包元数据)

Presets (预设基座): 插件的预设配置, 做到开箱即用。

Instances (插件实例): 独立打包的 Plugin A, Plugin B 等。
