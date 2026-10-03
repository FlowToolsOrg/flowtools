# Flow Tool Plugin 开发指南

本文面向 Flow Tool 开发者，目标是帮助你在当前仓库里快速开发并调试自己的
plugin。

> 当前阶段说明：Flow Tool 的产品方向是 desktop-first（Tauri）。
> `apps/web-vite` 仍用于验证 SDK/registry/runtime，`apps/desktop` 已提供
> 启动器、TanStack Router 路由、React/SDK 插件面板、HTML `main` 启动容器和 Tauri
> 官方插件驱动的 desktop capability adapter。

### 安全与支持边界

当前是 `prototype`，只适合开发评估，不是第三方生产 sandbox。内置 T1 可以
共享 React 树；T2 UI、T3 headless 和 TL legacy 的生产执行必须满足
[信任边界 ADR](./adr/0001-plugin-trust-boundaries.md) 与
[能力/包策略 ADR](./adr/0002-capability-and-package-policy.md)。签名不等于可信，
`permissions` 是请求，不等于用户授权；raw native/SQL/任意路径 adapter 不是
允许第三方使用的生产 API。实际缺口与后续验收见
[威胁模型](./security/threat-model.md) 和 [路线图](./production-roadmap.md)。

Catalog 索引、API 名称或构建成功不代表兼容认证；v1 不宣称兼容全部 125 个
HTML 插件，也不提供任意 shell/native binary 或 Node/Electron 私有 API。

## 1. 先理解插件模型

Flow Tool 目前支持两类插件：

- `app`：有长期 UI 面板，入口是 `setup() => ReactComponent`。也可以提供
  `run(ctx, input)` 使其可通过 CLI 或桌面端 AI agent 无 UI 调用。
- `tool`：即时执行型，入口是 `run(ctx, input)`

此外，SDK 提供 HTML `plugin.json` 兼容解析。它不会把 HTML 插件直接改写成
FlowTools 插件，而是先把 `main`、`preload`、`features`、`cmds` 归一化为可搜索
的插件描述，供 Tauri desktop 壳、iframe runner、兼容 bridge 和导入器使用。

统一通过 `definePlugin(...)` 声明，且必须有 `meta`：

- `id`：稳定唯一，命名方式使用 kebab-case
- `name`：展示名
- `version`：语义化版本
- `permissions`：声明所需能力（可选但强烈建议最小化）
- `description`: 插件描述（可选）
- `author`: 插件作者（可选）
- `link`: 插件链接（可选）
- `inputSchema`: Zod schema 声明输入参数（推荐），CLI 自动生成 flags 并做运行时校验
- `run(ctx, input)`: 执行入口，返回 `result.text/json/table/open/multi`

### 输入 Schema（Zod）

插件推荐通过 `inputSchema` 声明输入参数。使用 Zod `z.object({...})` 定义，
SDK 重新导出了 `z`，也可以从 `@flowtools/sdk` 直接导入：

```ts
import { z } from 'zod'
// or: import { z } from '@flowtools/sdk'

const inputSchema = z.object({
  count: z.number().default(1).describe('生成数量'),
  format: z.enum(['hex', 'base64']).default('hex'),
})
```

`inputSchema` 的作用：

- **CLI flags 自动生成**：`packages/cli` 解析 Zod schema 生成 `--count`、`--format` 等命令行参数
- **运行时校验**：CLI 调用时自动用 `.safeParse()` 校验输入，失败时输出错误信息
- **类型推导**：`z.infer<typeof inputSchema>` 推导 TypeScript 类型
- **JSON Schema 生成**：通过 `z.toJSONSchema()` 生成标准 JSON Schema（Zod 4 内置）

若未提供 `inputSchema`，CLI 仍可通过 `--input '{"key":"value"}'` 传入原始 JSON。

### 结果 helpers（`result.*`）

`run()` 推荐返回结构化的 `CommandResult`，使用 `result.*` helpers：

```ts
import { result } from '@flowtools/sdk/result'

result.text('Hello')           // 纯文本
result.json({ count: 3 })     // JSON 数据
result.table(columns, rows)   // 表格
result.open('https://...')    // 打开链接/文件
result.multi([...])           // 组合多个结果
```

### `run()` 与 `setup()` 共存

`app` 插件可以同时提供 `setup()`（UI）和 `run()`（CLI/无头调用）。
两者可以共享核心逻辑：

```ts
function generateItems(count: number): string[] {
  return Array.from({ length: count }, () => randomItem())
}

export default definePlugin({
  type: 'app',
  meta: { ... },
  inputSchema: z.object({ count: z.number().default(1) }),
  async run(_ctx, input) {
    return result.json({ items: generateItems(input.count) })
  },
  setup() {
    return function Panel() {
      // setup 内部也调用 generateItems() 实现 UI 逻辑
    }
  },
})
```

## 2. 本地开发准备

在仓库根目录执行：

```bash
bun install
cd apps/web-vite
bun run dev
```

如果你本地放了 legacy HTML 插件 checkout，可以从仓库根目录刷新插件兼容目录：

```bash
bun run inspect:html-plugins
```

该命令会写入 `apps/desktop/src/data/html-plugin-catalog.json`，desktop
启动器会直接读取这份目录；同时写入 `docs/html-plugin-catalog.json`
作为可读副本。

扫描器会区分源码 checkout 和可运行静态入口：如果 `main` 指向的 HTML 仍然引用
`/src/main.ts`、`/main.tsx` 等 Vite 源码入口，目录会把该静态入口标记为不可用。
Desktop runner 会改用 `development.main`；如果开发服务器没有启动，需要先在对应
HTML 插件目录执行构建或启动 dev server。

Desktop 端使用 TanStack Router 承载启动器、设置、插件、权限和命令运行页。
React/SDK 插件和 HTML 目录中的命令都会进入 `/run/$commandId`，内置设置类命令会进入对应页面。
React/SDK app 插件会直接渲染 panel；如果 HTML 插件声明了 `main`，该路由会启动 iframe 运行容器；没有 UI 入口的命令才进入 headless 执行状态视图。

Desktop runner 会在 iframe 里预注入旧版宿主 API。常用旧 API 会通过
`postMessage` 回到 desktop host，再走 SDK runtime context；实际原生能力由
`@tauri-apps/plugin-fs`、`plugin-dialog`、`plugin-clipboard-manager`、
`plugin-notification`、`plugin-sql`、`plugin-store`、`plugin-opener` 等官方
Tauri 插件提供。

Desktop host 的插件元数据正在迁移到 Rust + SQLite。后端记录与前端 manifest
保持同一批核心字段：`id`、`name`、`version`、`description`、`author`、
`link`、`type`、`permissions`、`tags`、`status`、`category`、`icon`、
`cliAvailable` 和 `state`。宿主侧新增/更新/删除/启用/禁用插件时，应通过
Tauri command binding 调用后端，不要绕过 Rust repository 直接改前端 registry。

### 2.1 HTML 插件兼容判断

`normalizeHtmlPluginManifest(...)` 会按运行时需求给 HTML 插件分级：

- `webview`：有 `main` 入口，优先作为 Tauri WebView 承载的 UI 插件。
- `preload-bridge`：存在 `preload`，需要实现旧版宿主 API 兼容层。
- `native-bridge`：包含 `files`、`img`、`window` 等命令类型，需要 Tauri/Rust 原生能力。
- `metadata`：没有 UI 入口，可先作为命令索引或重写为 FlowTools `tool` 插件。

兼容导入时优先保留 FlowTools 的 SDK/CLI/Zod 模型；只有当 HTML 插件数据结构更能表达实际插件入口时，才扩展 FlowTools 结构。

然后打开 Vite 输出的本地地址（默认通常是 `http://localhost:5173`）。

## 3. 创建插件目录

在 `plugins/` 下创建你的插件目录：

```text
plugins/
  plugin-my-first/
    index.tsx
```

建议命名规范：

- 目录名：`plugin-xxx`
- `meta.id`：与目录一致，使用 kebab-case

## 4. 开发一个 App 插件（有 UI + CLI 入口）

`plugins/plugin-my-first/index.tsx`：

```tsx
import { useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import { result } from '@flowtools/sdk/result'
import { Button, TextArea } from '@flowtools/sdk/ui'
import { z } from 'zod'

const inputSchema = z.object({
  initial: z.string().default('').describe('初始内容'),
})

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-my-first',
    name: 'My First Plugin',
    version: '0.1.0',
    permissions: ['storage', 'network', 'notification'],
  },
  inputSchema,
  async run(ctx, input: z.infer<typeof inputSchema>) {
    ctx.log('info', 'my-first executed', { initial: input.initial })
    return result.json({ status: 'ok', received: input.initial })
  },
  setup() {
    return function MyFirstPanel() {
      const { storage, request, ui, env } = useCapability()
      const [value, setValue] = useState(
        () => storage.get<string>('note') ?? ''
      )

      const save = () => {
        storage.set('note', value)
        ui.toast({
          title: 'Saved',
          message: `Saved in ${env.pluginId}`,
          level: 'success',
        })
      }

      const loadRemote = async () => {
        const res = await request(
          'https://jsonplaceholder.typicode.com/todos/1'
        )
        const data = (await res.json()) as { title?: string }
        setValue(data.title ?? '')
      }

      return (
        <section style={{ display: 'grid', gap: 8 }}>
          <h3>My First Plugin Panel</h3>
          <TextArea
            value={value}
            onChange={event => setValue(event.target.value)}
            rows={6}
          />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button onPress={save}>Save</Button>
            <Button onPress={loadRemote}>Load Remote Demo Data</Button>
          </div>
        </section>
      )
    }
  },
})
```

### 4.1 在 App 插件里使用 Zustand Store（推荐）

推荐 app plugin 使用 host 注入的 store hooks：

- `usePluginStore<TState>()`：读取响应式状态
- `usePluginStoreApi<TState, TActions>()`：获取 store API，通过 `actions.xxx()` 更新状态

插件通过 `definePluginStore()` 声明 store 形态（初始状态 + actions），
挂在 `AppPlugin.store`，host 会按 `pluginId` 创建并复用单实例 store。

```tsx
import type { FormEvent } from 'react'
import {
  definePlugin,
  definePluginStore,
  usePluginStore,
  usePluginStoreApi,
  type InferStoreState,
  type InferStoreActions,
} from '@flowtools/sdk'
import { Button } from '@flowtools/ui/plugin'

interface TodoItem {
  title: string
}

const todoStore = definePluginStore({
  initialState: {
    todos: [] as TodoItem[],
  },
  actions: set => ({
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

type TodoState = InferStoreState<typeof todoStore>
type TodoActions = InferStoreActions<typeof todoStore>

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-todo-list',
    name: 'Todo List',
    version: '0.1.0',
    permissions: ['storage'],
  },
  store: todoStore,
  setup() {
    return function TodoPanel() {
      const { todos } = usePluginStore<TodoState>()
      const {
        actions: { addTodo, removeTodo },
      } = usePluginStoreApi<TodoState, TodoActions>()

      const onAdd = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault()
        const formData = new FormData(event.currentTarget)
        const title = String(formData.get('title') ?? '').trim()
        if (!title) return

        addTodo({ title })
      }

      return (
        <>
          <form onSubmit={onAdd}>
            <input name="title" />
            <Button type="submit">Add</Button>
          </form>
          <div>Total: {todos.length}</div>
        </>
      )
    }
  },
})
```

## 5. 开发一个 Tool 插件（无常驻 UI）

`plugins/plugin-word-counter/index.ts`：

```ts
import { definePlugin } from '@flowtools/sdk'
import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

const inputSchema = z.object({
  text: z.string().describe('要统计的文本'),
})

export default definePlugin({
  type: 'tool',
  meta: {
    id: 'plugin-word-counter',
    name: 'Word Counter',
    version: '0.1.0',
    permissions: ['storage'],
  },
  inputSchema,
  async run(ctx, input: z.infer<typeof inputSchema>) {
    ctx.log('info', 'word-counter started', {
      inputLength: input.text.length,
    })

    const normalized = input.text.trim()
    if (!normalized) {
      return result.text('Input is empty')
    }

    const words = normalized.split(/\s+/).filter(Boolean).length
    ctx.storage?.set('last-input', normalized)

    return result.json({
      words,
      chars: normalized.length,
      timestamp: ctx.utils.now(),
    })
  },
})
```

说明：

- `ctx.signal` 可用于取消长任务
- `ctx.log(level, message, details)` 会进入宿主日志事件
- 结果可以返回任意结构，推荐用 `result.*` 统一结构

## 6. 在 Web 宿主中挂载调试

### 6.1 挂载 App 插件

修改 `apps/web-vite/src/routes/test.tsx`，替换示例插件：

```tsx
import { createFileRoute } from '@tanstack/react-router'

import myPlugin from '../../../../plugins/plugin-my-first'
import { renderWebAppPlugin } from '../runtime'

export const Route = createFileRoute('/test')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <div>
      <section>{renderWebAppPlugin(myPlugin)}</section>
    </div>
  )
}
```

然后访问 `/test` 即可看到你的插件 UI。

### 6.2 触发 Tool 插件执行

在任意页面临时调用：

```tsx
import toolPlugin from '../../../../plugins/plugin-word-counter'
import { runWebToolPlugin } from '../runtime'

const output = await runWebToolPlugin(toolPlugin, {
  text: 'hello flow tool',
})
console.log(output)
```

## 7. 从 CLI 调用插件

`packages/cli` 提供统一的 CLI 入口，可直接调用任何提供了 `run()` 的插件。

### 7.1 列出可调用插件

```bash
bun run packages/cli/src/cli.ts list
# 或 JSON 格式
bun run packages/cli/src/cli.ts list --format json
```

### 7.2 查看插件详情

```bash
bun run packages/cli/src/cli.ts info plugin-uuid-generator
```

### 7.3 执行插件

通过 schema flags（推荐）：

```bash
bun run packages/cli/src/cli.ts run plugin-uuid-generator --count 5
```

通过原始 JSON：

```bash
bun run packages/cli/src/cli.ts run plugin-uuid-generator \
  --input '{"count": 5}'
```

输出格式控制：

```bash
# 默认 JSON 格式
bun run packages/cli/src/cli.ts run plugin-uuid-generator --count 3

# 文本格式
bun run packages/cli/src/cli.ts run plugin-uuid-generator --count 3 \
  --format text
```

### 7.4 为桌面端 AI agent 预留

CLI 入口设计为机器可读：`--format json` 输出结构化结果，
桌面端 Tauri host 可通过 `Command::new("flowtools")` 调用 CLI，
将输出解析后反馈给 AI agent。

### 7.5 子路径导入

SDK 提供两个子路径，插件可以直接从这里导入：

```ts
// definePlugin 函数（单独导入，避免拉入整个 SDK）
import { definePlugin } from '@flowtools/sdk/definePlugin'

// result helpers
import { result } from '@flowtools/sdk/result'
```

## 8. 权限与 Capability 对照

### 8.1 Desktop Host

| permission     | SDK 能力                                 | desktop 当前行为                           |
| -------------- | ---------------------------------------- | ------------------------------------------ |
| `network`      | `useRequest()` / `ctx.request`           | WebView `fetch`                            |
| `storage`      | `useStorage()` / `ctx.storage`           | namespaced sync storage + Tauri Store 镜像 |
| `fs`           | `useFS()` / `ctx.fs`                     | `@tauri-apps/plugin-fs`                    |
| `clipboard`    | `useClipboard()` / `ctx.clipboard`       | `@tauri-apps/plugin-clipboard-manager`     |
| `notification` | `useNotification()` / `ctx.notification` | `@tauri-apps/plugin-notification`          |
| `dialog`       | `useDialog()` / `ctx.dialog`             | `@tauri-apps/plugin-dialog`                |
| `db`           | `useDB()` / `ctx.db`                     | `@tauri-apps/plugin-sql` SQLite            |
| `native`       | `useNative()` / `ctx.native`             | Tauri `invoke`                             |

新增桌面原生能力时，先在 `apps/desktop` 执行：

```bash
bun run tauri add <plugin-name>
```

然后在 `apps/desktop/src/runtime/desktop-capabilities.ts` 中把官方插件 API
适配到 SDK capability。业务插件不直接依赖 Tauri API。

### 8.2 Web Prototype

| permission     | SDK 能力                                   | web-vite 当前行为                                                            |
| -------------- | ------------------------------------------ | ---------------------------------------------------------------------------- |
| `network`      | `useRequest()` / `ctx.request`             | 浏览器 `fetch`                                                               |
| `storage`      | `useStorage()` / `ctx.storage`             | namespaced `localStorage` + zustand adapter                                  |
| `store`        | `usePluginStore()` / `usePluginStoreApi()` | host 管理的 Zustand store（按 pluginId 单例，通过 `definePluginStore` 声明） |
| `fs`           | `useFS()` / `ctx.fs`                       | `localStorage` 模拟文件系统                                                  |
| `clipboard`    | `useClipboard()` / `ctx.clipboard`         | 浏览器 clipboard API                                                         |
| `notification` | `useNotification()` / `ctx.notification`   | Notification API，失败时降级 toast                                           |
| `dialog`       | `useDialog()` / `ctx.dialog`               | 已注入但调用会抛 `Not supported`                                             |
| `db`           | `useDB()` / `ctx.db`                       | 已注入但调用会抛 `Not supported`                                             |
| `native`       | `useNative()` / `ctx.native`               | 已注入但调用会抛 `Not supported`                                             |

补充：

- `useUI()` 和 `useEnv()` 不依赖权限声明
- 若使用了 `useFS/useRequest/useStorage/...`，但未声明对应 permission，会抛
  `MissingCapabilityError`
- 上表是原型适配行为，不是 grant/scope 授权。DB raw query 未隔离插件数据；
  同 realm 中的代码可以绕过 SDK，官方插件自身权限也不等于每插件授权。

## 9. 常见错误与排查

### 9.1 `MissingRuntimeContextError`

原因：在 Flow Tool runtime provider 外调用了 SDK hooks。  
处理：确保插件 UI 通过宿主渲染（例如 `renderWebAppPlugin(plugin)`），不要在宿主
外裸用插件组件。

### 9.2 `MissingCapabilityError: Capability "xxx" is unavailable`

原因：插件代码调用了某 capability，但 `meta.permissions` 没声明。  
处理：内置开发插件核对声明后补充 permission，或删除不必要调用。第三方
不能通过修改声明获得生产授权；未来须由 broker 校验实际 grant 与 scope。

### 9.3 `[flowtools-web-runtime] [dialog/db/native] Not supported on web runtime`

原因：web 原型暂未实现这些能力。  
处理：本地 web 调试阶段避免依赖这些能力；等 desktop host 接入后再使用。

## 10. 推荐开发习惯

- 建议优先使用 capability selector 获取 Capability（如：`useCapability(_ => ({ storage: _.storage }))`），也可以使用专用 hooks（如 `useStorage()`）
- App 插件状态优先使用 `definePluginStore()` 声明 store 形态，通过 `AppPlugin.store` 挂载；消费时使用 `usePluginStore<TState>()` 和 `usePluginStoreApi<TState, TActions>().actions.xxx()`
- 推荐定义 `inputSchema`（Zod `z.object({...})`），使插件可被 CLI 调用并自动校验输入
- 推荐 `run()` 返回 `result.*` 结构化结果，便于 CLI `--format json` 和桌面端解析
- 推荐从 `@flowtools/sdk/definePlugin` 和 `@flowtools/sdk/result` 子路径导入，避免拉入整个 SDK
- `permissions` 仅声明需要的最小集合
- 插件 ID、命令 ID 使用 kebab-case
- 插件逻辑放 `plugins/*`，宿主集成逻辑放 `apps/*`

## 11. 提交前检查

新增/修改 capability 或 bridge 时，填写
[PR 模板](../.github/pull_request_template.md) 的安全评审部分：威胁 ID、实际
reviewer、Host 身份/scope、拒绝回归、撤销与恢复、残余风险。无安全边界影响
也要说明不适用理由。`docs:check` 只检查文档结构与本地链接路径，不代表安全
评审已经获批、远程 URL/anchor 可用或插件已获得兼容认证。

在仓库根目录执行：

```bash
bun run docs:check
bun run lint
bun run check-types
bun run test
bun run build
```

自动化测试是必需质量门。新机器先执行
`bun run --cwd apps/ui-test test:install-browser` 安装锁定版本 Chromium。
Web manifest 测试逐插件报告，真实 package 冷加载与批量注册用例限定 30 秒；
普通单元用例保留默认超时，加载错误、元数据差异或超时仍失败，不自动重试。
这个集成测试预算不是生产启动性能承诺。
插件目录 inventory 与构建入口必须一致；新增插件要更新合约清单，确保 metadata、
permissions、`run()`、`inputSchema` 和 CLI 执行链路都有对应验证。合约测试不得
访问外部网络或用户状态；第三方兼容、权限隔离和打包产物按生产路线图单独验收。

独立干净 Windows checkout 使用
`pwsh -NoProfile -File scripts/check-ci.ps1` 复现 PR 门禁；入口先构建 SDK/UI/CLI/
plugins 声明产物，再执行各 workspace 的完整验证。插件构建不得写入未忽略的
生成文件，也不得依赖开发机现有 `dist`、用户数据库或 .env 才能通过自动化。
工作流交付不等于远端 CI / 合并保护已验收，也不构成插件生产认证。
Desktop command DTO 来源必须是 Rust 生成器，不能维护第二份手写绑定；生成过程
不加载插件、不启动宿主窗口，也不初始化用户数据库。修改 Rust commands/DTO 时
重新生成并验证前端类型消费以及非交互生成回归。

并手动验证：

- 插件页面能正常渲染
- 权限声明与实际调用一致
- 不支持的 web 能力有清晰降级或提示
- CLI 可正常调用：`bun run packages/cli/src/cli.ts run <plugin-id> --format text`
