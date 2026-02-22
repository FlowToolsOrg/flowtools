# Flow Tool Plugin 开发指南

本文面向 Flow Tool 开发者，目标是帮助你在当前仓库里快速开发并调试自己的
plugin。

> 当前阶段说明：Flow Tool 的产品方向是 desktop-first（Tauri），但仓库里当前
> 可运行宿主是 `apps/web-vite`，本文以 web 原型为准。

## 1. 先理解插件模型

Flow Tool 目前支持两类插件：

- `app`：有长期 UI 面板，入口是 `setup() => ReactComponent`
- `tool`：即时执行型，入口是 `run(ctx, input)`

统一通过 `definePlugin(...)` 声明，且必须有 `meta`：

- `id`：稳定唯一，命名方式使用 kebab-case
- `name`：展示名
- `version`：语义化版本
- `permissions`：声明所需能力（可选但强烈建议最小化）
- `description`: 插件描述（可选）
- `author`: 插件作者（可选）
- `link`: 插件链接（可选）

## 2. 本地开发准备

在仓库根目录执行：

```bash
bun install
cd apps/web-vite
bun run dev
```

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

## 4. 开发一个 App 插件（有 UI）

`plugins/plugin-my-first/index.tsx`：

```tsx
import {
  definePlugin,
  useEnv,
  useRequest,
  useStorage,
  useUI,
} from '@flow-tool/sdk'
// or import { useEnv, useRequest, useStorage, useUI } from '@flow-tool/sdk/hooks'
import { useState } from 'react'

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-my-first',
    name: 'My First Plugin',
    version: '0.1.0',
    permissions: ['storage', 'network', 'notification'],
  },
  setup() {
    return function MyFirstPanel() {
      const env = useEnv()
      const ui = useUI()
      const storage = useStorage()
      const request = useRequest()
      // or const { env, ui, storage, request } = useCapabilities()
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
          <textarea
            value={value}
            onChange={event => setValue(event.target.value)}
            rows={6}
          />
          <div style={{ display: 'flex', gap: 8 }}>
            <button onClick={save}>Save</button>
            <button onClick={loadRemote}>Load Remote Demo Data</button>
          </div>
        </section>
      )
    }
  },
})
```

### 4.1 在 App 插件里使用 Zustand Store（推荐）

推荐每个 app plugin 维护一个 root store，然后通过 `useStorage()` 提供的
`storage.zustand(namespace?)` 接入持久化。

```tsx
import type { StorageCapability } from '@flow-tool/sdk'

import { definePlugin, useStorage } from '@flow-tool/sdk'
import { Button } from '@flow-tool/ui/plugin'
import { createJSONStorage, persist } from 'zustand/middleware'
import { useStore } from 'zustand'
import { createStore } from 'zustand/vanilla'

interface CounterState {
  count: number
  inc: () => void
}

type CounterStore = ReturnType<typeof createCounterStore>

let counterStore: CounterStore | undefined

function createCounterStore(storage: StorageCapability) {
  return createStore<CounterState>()(
    persist(
      set => ({
        count: 0,
        inc: () => set(state => ({ count: state.count + 1 })),
      }),
      {
        name: 'root',
        storage: createJSONStorage(() => storage.zustand('counter')),
      }
    )
  )
}

function useCounterStore<T>(selector: (state: CounterState) => T): T {
  const storage = useStorage()

  if (!counterStore) {
    counterStore = createCounterStore(storage)
  }

  return useStore(counterStore, selector)
}

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-counter',
    name: 'Counter',
    version: '0.1.0',
    permissions: ['storage'],
  },
  setup() {
    return function CounterPanel() {
      const count = useCounterStore(state => state.count)
      const inc = useCounterStore(state => state.inc)

      return <Button onPress={inc}>Count: {count}</Button>
    }
  },
})
```

## 5. 开发一个 Tool 插件（无常驻 UI）

`plugins/plugin-word-counter/index.ts`：

```ts
import { definePlugin, result } from '@flow-tool/sdk'

interface WordCounterInput {
  text: string
}

export default definePlugin({
  type: 'tool',
  meta: {
    id: 'plugin-word-counter',
    name: 'Word Counter',
    version: '0.1.0',
    permissions: ['storage'],
  },
  async run(ctx, input: WordCounterInput) {
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

## 7. 权限与 Capability 对照（Web 原型）

| permission     | SDK 能力                                 | web-vite 当前行为                           |
| -------------- | ---------------------------------------- | ------------------------------------------- |
| `network`      | `useRequest()` / `ctx.request`           | 浏览器 `fetch`                              |
| `storage`      | `useStorage()` / `ctx.storage`           | namespaced `localStorage` + zustand adapter |
| `fs`           | `useFS()` / `ctx.fs`                     | `localStorage` 模拟文件系统                 |
| `clipboard`    | `useClipboard()` / `ctx.clipboard`       | 浏览器 clipboard API                        |
| `notification` | `useNotification()` / `ctx.notification` | Notification API，失败时降级 toast          |
| `dialog`       | `useDialog()` / `ctx.dialog`             | 已注入但调用会抛 `Not supported`            |
| `db`           | `useDB()` / `ctx.db`                     | 已注入但调用会抛 `Not supported`            |
| `native`       | `useNative()` / `ctx.native`             | 已注入但调用会抛 `Not supported`            |

补充：

- `useUI()` 和 `useEnv()` 不依赖权限声明
- 若使用了 `useFS/useRequest/useStorage/...`，但未声明对应 permission，会抛
  `MissingCapabilityError`

## 8. 常见错误与排查

### 8.1 `MissingRuntimeContextError`

原因：在 Flow Tool runtime provider 外调用了 SDK hooks。  
处理：确保插件 UI 通过宿主渲染（例如 `renderWebAppPlugin(plugin)`），不要在宿主
外裸用插件组件。

### 8.2 `MissingCapabilityError: Capability "xxx" is unavailable`

原因：插件代码调用了某 capability，但 `meta.permissions` 没声明。  
处理：补充对应 permission，或删除 capability 调用。

### 8.3 `[flow-tool-web-runtime] [dialog/db/native] Not supported on web runtime`

原因：web 原型暂未实现这些能力。  
处理：本地 web 调试阶段避免依赖这些能力；等 desktop host 接入后再使用。

## 9. 推荐开发习惯

- 建议优先使用 capability selector 获取 Capability（如：`useCapabilities(_=>({storage: _.storage}))`），也可以使用专用 hooks（如 `useStorage()`），还可以使用 `useCapabilities()` 直接获取全部 capability
- `permissions` 仅声明需要的最小集合
- 插件 ID、命令 ID 使用 kebab-case
- 插件逻辑放 `plugins/*`，宿主集成逻辑放 `apps/*`

## 10. 提交前检查

在仓库根目录执行：

```bash
bun run lint
bun run check-types
bun run test
```

并手动验证：

- 插件页面能正常渲染
- 权限声明与实际调用一致
- 不支持的 web 能力有清晰降级或提示
