# Flow Tool Plugin 开发指南

G5 P2.5a 作者可从 `@flowtools/sdk/manifest` 使用
`packageDescriptorSchema`、`serializePackageDescriptor` 与
`packagePreAuthenticationEncoding`，生成并签署独立 DSSE 描述的原始字节。
内嵌 Manifest v1 的 unsigned 字段不授予信任；真实可信根、publisher 范围、
撤销与过期由 Rust 核对。当前产品没有第三方安装/执行入口或 production
pins；tool 描述也不授权执行 native binary。格式、限额与回滚约束见
[ADR-0003](./adr/0003-signed-package-protocol.md)、
[P2.5a 验证](./validation/g5-package-protocol.md)。

本文面向 Flow Tool 开发者，目标是帮助你在当前仓库里快速开发并调试自己的
plugin。

### 下一阶段插件契约（设计，未实现）

维护者已确定 React 为主要插件开发方式，每项业务能力通过同一命令契约供
GUI 与外部 agents 调用。目标包分离 commands、可选 React UI 与服务入口；
Manifest v1 声明输入/输出 Schema、效果、headless/冷启动支持、资源预算、
服务和工具依赖。代码库随包构建，服务通过 Host RPC，FFmpeg 等工具由宿主
集中准入和共享版本文件；共享依赖不共享授权。覆盖/删除/发送需明确授权。
低代码创建以后也生成正常插件包，当前只作为未来展望。
详细顺序与拟新增 API 见 [实施设计](./next-milestones.md)；本指南后文仍描述
当前 API，不能把设计字段当作现有 SDK 支持或第三方执行许可。

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

下一阶段允许签名准入、宿主集中管理的二进制工具依赖与后台内核；仍禁止任意
shell、未经准入 executable path 和插件自建常驻进程。新能力验收前维持当前拒绝。

P0.3b1 的普通 SDK `PluginFileLoader` 一律抛 `EXTERNAL_CODE_DISABLED`，不读取
源码、不注册外部对象；普通 SDK 不再导出 transpile/setupImportMap 等注入工具。
Web 默认及生产构建均不可导入源码；受控开发评估须同时使用 Vite dev server 和
显式 `VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW=1`。危险实现从
`@flowtools/sdk/development` 在 DEV 分支动态导入，并显示未签名、同 realm 风险，
不得在宿主静态导入该子入口或当作安装/隔离/grant。已有 IndexedDB 源码保留，
任何模式都不自动恢复；需要预览时重新主动选择审阅过的文件，仅使用可丢弃数据。
P0.3b2 CLI 只执行构建内嵌清单中的内置 compiled artifact，运行时目录新增项、
任意路径、直接 TSX 与 headless rewrite 不再支持。更新内置 metadata 后运行
`bun run generate:manifests`，随后 `bun run build:packages`；list/info/run/help
缺编译产物会失败，不回退源码。这是 T1 构建一致性，不是包签名或 sandbox。
P0.3b3 Desktop 默认也拒绝 HTML/Legacy，普通 bridge API 一律拒绝；危险 runner
仅在 DEV + 精确 opt-in 下动态导入。开发 bridge 也不能调用 raw invoke、SQL、
任意 FS 或 opener；失败不直接打开远程 src。根 gate、实际生产产物与前端
拒绝已通过；独立 r3 包的实际 Base64 与 HTML 默认拒绝人工清单已通过，
运行路径/身份另行核实。不能解释为已完成签名/隔离或独立安全验收。

P0.3b4 生产 artifact gate 在 root build 后检查两端固定构建树与已知危险/
certification syntax，再用 opt-in=1 和 synthetic path/URL/key canaries
实际重建，要求所有字节与普通产物一致。它不执行插件、不删除旧数据，
不是签名或 publisher 认证；CLI compiled inventory 与服务拒绝另由真实回归覆盖。
见 [发布产物验收](./validation/p0-production-artifacts.md)。

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
- `maturity`：`prototype` / `experimental` / `beta` / `production`，缺失时为
  prototype。原 `PluginMeta.status: stable/deprecated` 改用本字段；beta/production
  只能在路线图记录客观验收后推进，不能由声明自行证明安全/兼容/授权。
- `permissions`：声明所需能力（可选但强烈建议最小化）
- `description`: 插件描述（可选）
- `author`: 插件作者（可选）
- `link`: 插件链接（可选）
- `inputSchema`: Zod schema 声明输入参数（推荐），CLI 自动生成 flags 并做运行时校验
- `run(ctx, input)`: 执行入口，返回 `result.text/json/table/open/multi`

### 输入 Schema（Zod）

SDK 从 `@flowtools/sdk/types` 导出 `pluginMaturitySchema`、`resolvePluginMaturity`
和独立 `compatibilityEvidenceStatusSchema`。内置插件、两端 manifest 与 CLI
list/info 当前都为 prototype；生成两端 metadata 使用 `bun run generate:manifests`。
Catalog 与 UI 已使用相同 maturity 词表；ToolStatus/ToolMarketStatus 是 SDK 类型
别名，原 status prop 仅表示成熟度。共享 PluginMaturityBadge 缺省显示 Prototype，
PluginCompatibilityBadge 单独显示证据；桥接需求、元数据保存和生产授权都不是
maturity。Portable Catalog formatVersion 1 记录 logical source、package
identity/相对路径与扫描 hash，不保存本机根路径或 development URL。重新扫描使用
`bun run inspect:html-plugins [checkout-path]`，随后运行 `bun run verify:plugin-catalog`。
当前仅接受 indexed/entry-resolved；后者是入口文件存在/hash，不认证资源依赖、
runtime、bridge、平台、签名或安全，也不允许生产执行。47 项 entry-resolved、
78 项 indexed，全部 prototype；API/production 认证仍待 P3.4。
本地预览需在开发服务器显式配置 VITE_HTML_PLUGIN_ROOT 指向自己的 checkout，
不能把该路径写回目录或嵌入发布产物；该配置不是隔离/授权。

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

SDK 共享执行入口为 `executePlugin(plugin, input, toolContext, options)`，对 app
和 tool 的实际 `run()` 应用 schema 默认值与校验，返回 `PluginExecutionResult`。
结果含插件 ID/版本、真实开始/完成时间、耗时和仅类型/大小的输入摘要；失败含
稳定 `error.code`，如 `INPUT_INVALID`、`EXECUTION_FAILED`、`ABORTED`、`TIMEOUT`。
默认异步等待上限 30 秒，可传 `signal` 与正整数 `timeoutMs`；取消通知插件并
丢弃迟到结果，但插件必须合作停止副作用，同步死循环无法在共享 realm 内终止。
Host 必须注入与插件 ID 匹配的 context。CLI/Web/Desktop 已接入（P0.2b）。
非 React 调用可从 `@flowtools/sdk/execution` 导入执行器。Web 工具页 Run tab
与 Desktop React app/tool runner 使用共享 `ExecutionPanel`；app panel 保留。
输入 JSON/schema 错误、实际异常、取消与输出序列化失败都显示稳定失败码。
历史使用 SDK `createExecutionHistory()`，formatVersion 1、最多 200 条 metadata，
仅保存 ID/名称/版本、输入类型/大小、真实时间/耗时、status/resultType/errorCode。
原始输入、输出和异常 message 只展示在本次结果中，不持久化。新 key 与旧未验证
历史分离，旧 key 不删除；恢复损坏或矛盾记录时显示提示，不伪造成功。
Windows 验收范围与限制见 [宿主验收](./validation/p0-execution-hosts.md)。
测试实例必须初始进入真实首页，不能以跳转到插件页掩盖启动 NotFound。
专用验收配置使用根路由查询标记；开发模式手工通过与独立包通过分开记录。

内置插件的 `bun run smoke:plugins` 使用真实编译实现和受控 capability，不访问
公网/用户数据，也不替换 run()。Todo run 在 app host 读写 panel 共享 store；CLI
继续使用原 `todos` key，数据先校验、损坏时失败且不覆盖；没有隐式跨 namespace
迁移。网站延迟 run/panel 都使用 SDK request，缺少能力时实际失败，不回退 global
fetch。批量测量中单个站点错误仍是返回的诊断数据，并非整个 run() 抛出异常。

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
当前 portable catalog 不发布 development URL，不自动回退源码态服务。受控
开发预览须指定 VITE_HTML_PLUGIN_ROOT 并提供已构建的静态入口；该路径不是授权。

Desktop 端使用 TanStack Router 承载启动器、设置、插件、权限和命令运行页。
React/SDK 插件和 HTML 目录中的命令都会进入 `/run/$commandId`，内置设置类命令会进入对应页面。
React/SDK app 插件会直接渲染 panel；HTML/Legacy 默认显示 EXTERNAL_CODE_DISABLED，
仅显式 DEV opt-in 启动危险预览 iframe，不因安装/启用 metadata 或 main 声明放行。

显式开发 runner 会在 iframe 里预注入有限旧版宿主 API。UI、clipboard、dialog、
notification 经开发 bridge 的固定方法列表调用，身份 context 来自 Host command。
raw native/SQL/FS/opener 方法仍禁用；不是独立 session、用户 grant 或 sandbox。
有限旧 API 会通过
`postMessage` 回到 desktop host，再走 SDK runtime context。宿主的 T1 内置
插件 adapter 另外使用官方 `plugin-fs`、`plugin-dialog`、
`plugin-clipboard-manager`、`plugin-notification`、`plugin-sql`、
`plugin-store`、`plugin-opener`；安装这些依赖不向 Legacy bridge 开放对应 API。

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

`packages/cli` 提供统一的 CLI 入口，只调用固定 Host inventory 中提供 `run()`
的内置编译插件；增加源码目录本身不会开放 CLI 执行。

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

JSON 输出现在是 `PluginExecutionResult`：成功读取 `data` 中的真实 `CommandResult`，
失败读取 `error.code/message`，两者均有真实版本、时间和输入形状摘要。失败同时
输出 JSON 到 stdout、稳定码到 stderr，退出码 1；无效输出序列化为 `OUTPUT_INVALID`。
text formatter 保留现有展示。此为 prototype 接口变更，不再输出裸结果 JSON。
CLI adapter 不自动记录原始插件日志；storage/network 仅按内置插件声明提供，
不是第三方用户授权。存储使用 SDK `remove/zustand`，保留合法现有键的文件位置，
路径和设备保留名键会拒绝；canonical/symlink 强制隔离仍待安全阶段。

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

## P0.3c catalog and permission presentation

Desktop market actions save built-in configuration only. HTML entries display
不可安装 and saved records cannot grant execution. Removing a record is not a
package uninstall. The permission center shows capability declarations and
explicitly records that per-plugin grants and isolation are not implemented.
Legacy persisted DTO status values remain metadata for compatibility.
All maturity labels stay prototype; SEC-001–SEC-012 remain open.

## P1.1a 序列化操作协议

插件函数对象不属于包 Manifest。用 [Manifest v1](./manifest-v1.md) 的纯数据
命令声明表达 Schema、效果、预算与授权请求；runtimeValidation=required 必须
提供真实校验器。旧元数据迁移须显式补齐 publisher、包文件和操作信息。当前
T1 run/setup 接口仍保持兼容。十二插件已拆为 commands.ts 与 UI index.tsx，
UI 显式复用 commandPlugin，Todo setup/run 使用同一 host store。
commands 不导入 React/GUI/Tauri；完整包 hash 由 build-manifests.ts 从实际 dist
生成。修改命令后重新 build:packages；UI watch 不替代 CLI 包重建。
CLI 的命令级 capability 请求独立于 UI metadata 请求，均不等于用户 grant。

P1.1c：用 `commands`/`describe <id> run --format json` 获取纯 operation contract，
help/flags 和 [生成文档](./builtin-commands.md) 同源。具体错误、false/负数/数组、
JSON 与顺序 batch 规则见 [CLI v1](./cli-contract-v1.md)。三端 JSON run 都通过
Manifest executor；UI run capability 取命令请求和 metadata 声明交集，panel
继续使用既有 provider/store。Web/Desktop 固定 loader 在 UI import 前验证契约，
外部执行仍 deny-only。操作修改后 build:packages 并 generate:command-docs。

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

## Plugin change delivery

Run `build:packages` before lint/types in fresh checkouts; it also builds the
Runtime client consumed through declared package exports by native validation.
Git pins the three Rust-derived Runtime source artifacts to LF; generation must
retain the final clean-worktree gate without staging or accepting content drift.

Completed plugin tasks follow the automatic feature-branch push and PR workflow
authorized on 2026-10-05 in [AGENTS.md](../AGENTS.md). Include applicable plugin
validation and security-review evidence in the PR; pending reviews remain
explicit. Merging, deployment and repository settings require separate
authorization.

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

### G3 P1.3b durable execution

Windows managed T1 jobs now commit accepted metadata before receipt and preserve
caller-bound idempotency across restart. Actual Todo uses Host-owned async data
and broker-checked CAS; raw task payloads use separate DPAPI private storage.
Interrupted non-idempotent writes require review. [Evidence](validation/g3-durable-jobs.md).
Independent CLI distribution and GUI integration remain P1.4a/P1.4b; all maturity
labels stay prototype and independent security review remains pending.

### G3 P1.4a standalone CLI

Windows x64 CLI-only bundle and bounded, authenticated cold-start coordination
are implemented for the fixed T1 inventory. See [standalone CLI evidence](validation/g3-standalone-cli.md).
The package pins relative compiled artifacts and supports explicit GUI-free init,
runtime start/status/stop and granted bundle execution. Source CLI/GUI integration
and operational diagnostics remain P1.4b/P1.5b. Maturity remains prototype;
signed releases, third-party sandbox and independent security review are pending.

### G3 P1.4b shared Host clients

Desktop and source/standalone CLI execute through the same authenticated T1 Host.
`jobs submit/list/status/watch/cancel/lookup` expose durable receipts and bounded
metadata; listings exclude results. Same-user CLI/Desktop sessions can cancel
one another's tasks; validation callers and T0 management roles retain their
separate rejection rules. Lost acknowledgements query the original key, never
create a new effect. The wire client is now exactly 0.2.0; 0.1.0 clients fail the
handshake before business IO. Protocol major and SQLite schema are unchanged.

The source Desktop prototype pins the build-owned Runtime binary and verifies
it before native bootstrap. Its main native window and configured DEV origin
own sessions; credentials/endpoint/executable selectors never enter JavaScript.
Release Desktop distribution remains a later platform gate. Initialization,
grants, cold-start changes and full stop require an actual native confirmation;
revocation is immediate. The GUI shows active task count before full stop.
Closing GUI disconnects foreground work while explicitly granted background
work belongs to Runtime. Native Todo uses async revisions/CAS and no client
persistent store; original local prototype data remains for deliberate import.

See [shared client evidence](validation/g3-shared-clients.md). All scopes stay
prototype. Windows native consent and recovery UI fixes have actual
[acceptance evidence](validation/g3-desktop-acceptance-fixes.md);
independent security review remains pending.

### G3 P1.5b diagnostics and offline recovery

Fixed Windows T1 runs now expose `jobs diagnose <runId>` and a native Desktop
metadata summary/export. Identity, package/dependency lock, grant epoch, state,
sequence, time and stable failure code are allowlisted; inputs, outputs, paths,
credentials and arbitrary exception messages are excluded. Wire client is exactly
0.3.0; older clients fail before business IO. Protocol major and DB schema stay 1/2.

Offline `runtime storage list/create/restore <backup-id>/retry` reserves the same
current-user profile and first native pipe before SQLite IO. Restore/retry require
CLI `--confirm` or actual native confirmation. Logical UUIDs select same-profile
backups; clients cannot choose backup paths. Pending recovery blocks Host startup.
Restoration stages SQLite data, revokes grants, disables cold start, interrupts
unfinished jobs and quarantines original DB/private payloads. Retry continues the
same journal; it never replays business work. Old result metadata remains queryable
through diagnosis, while quarantined outputs expire; new runs retain normal results.
Missing post-backup keys return ACCEPTANCE_UNKNOWN and require explicit review.

See [diagnostic/recovery evidence](validation/g3-diagnostics-recovery.md). Scope remains prototype. Independent
security review, full blind-user workflow and other platform acceptance are pending;
these checks do not authorize production.

Authenticated T0 stop immediately cancels queued/running business work, then
delivers a bounded stopping receipt before process drain. It neither retries work
nor treats disconnection as success. Desktop window zoom shortcuts are enabled.
[Terminal verification](validation/g3-terminal-validation.md) records regressions
and fresh Native harness results, plus actual Windows keyboard/native consent,
200% zoom/reflow and NVDA Speech Viewer output. Independent security approval
remains pending; human listening, a full blind-user workflow and other platforms
are not certified.
