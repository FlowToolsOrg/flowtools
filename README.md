# Flow Tool

> An extensible, cross-platform toolbox powered by a plugin runtime.

Flow Tool is a plugin-driven utility platform focused on capability injection,
permission gating, and a unified React UI runtime.

The target production host is desktop (Tauri + Rust).
This repository currently contains a web runtime prototype used to validate the
SDK, plugin contracts, and capability model before desktop host implementation.

## ✨ Vision

Flow Tool is not just a toolbox.

It is:

- 🧩 A plugin-driven computing platform
- ⚙️ A capability-based runtime
- 🖥 Cross-platform (Desktop first, Web-ready)
- 🔌 Extensible by design

Inspired by systems like:

- VSCode Extension Host
- Obsidian Plugin Architecture
- Raycast Command Model
- uTools Instant Tools

But purpose-built for utility workflows.

## 🏗 Architecture Overview

Flow Tool is built in layered form:

```
┌───────────────────────────────┐
│ Plugin Layer                  │
│ Tool Plugins  |  App Plugins  │
├───────────────────────────────┤
│ SDK & Runtime                 │
│ definePlugin | hooks | perms  │
├───────────────────────────────┤
│ Host Capability               │
│ FS | DB | Network | Native    │
├───────────────────────────────┤
│ Rust Core (Tauri)             │
└───────────────────────────────┘
```

Core principles:

- Single React tree
- Capability injection via runtime context
- Permission-gated resource access
- Namespaced storage & database
- Plugin lifecycle management

## Current Status (May 2026)

- Product direction: desktop-first, cross-platform ready.
- Current runnable host: `apps/web-vite`.
- Core packages under active development:
  - `packages/sdk` (`@flow-tool/sdk`) — plugin contract, hooks, registry, lifecycle
  - `packages/ui` (`@flow-tool/ui`) — shared UI components including CommandPalette
- Local plugin workspace with 8 built-in plugins:
  - `plugins/plugin-todo-list` (app with host-managed store)
  - `plugins/plugin-uuid-generator`, `plugin-text-ops`, `plugin-random-picker`,
    `plugin-image-base64`, `plugin-website-latency` (app)
- Planned (directory created, not yet implemented):
  - `apps/desktop`
  - `apps/docs`
  - `apps/web`

## Monorepo Layout

```text
apps/
  web-vite/    # web host prototype (router + registry + command palette)
  web/         # (planned)
  desktop/     # (planned) Tauri desktop host
  docs/        # (planned) documentation site
  ui-test/     # UI package consumer and browser test app
packages/
  sdk/         # plugin contract, hooks, registry, lifecycle, result helpers
  ui/          # shared React UI primitives (HeroUI-based)
plugins/
  plugin-example-hello-world/  # app plugin example
  plugin-example-run-hello/    # tool plugin example
  plugin-todo-list/            # app plugin with host-managed store
  plugin-uuid-generator/       # UUID generator (clipboard)
  plugin-text-ops/             # text set operations (clipboard)
  plugin-random-picker/        # random name picker
  plugin-image-base64/         # image ↔ base64 converter (clipboard)
  plugin-website-latency/      # website latency tester (network)
configs/
  tsdown/      # shared package build config
```

## Development

Run from repository root:

```bash
bun install
bun run dev
bun run build
bun run lint
bun run check-types
bun run test
bun run format
```

Useful local commands:

```bash
cd apps/web-vite && bun run dev
cd apps/ui-test && bun run dev
cd apps/ui-test && bun run test
```

## Plugin Model

Flow Tool supports two plugin categories:

- `app`: persistent panel plugins (`setup()` returns a React component)
- `tool`: instant execution plugins (`run(ctx, input)`)

Minimal app plugin example:

```tsx
import { definePlugin, useCapability } from '@flow-tool/sdk'

export default definePlugin({
  type: 'app',
  meta: {
    id: 'example-app',
    name: 'Example App',
    version: '0.1.0',
    permissions: ['storage'],
  },
  setup() {
    return function Panel() {
      const { storage } = useCapability()

      return <button onClick={() => storage.set('hello', 'world')}>Save</button>
    }
  },
})
```

## SDK Hooks

`@flow-tool/sdk` exposes:

- **`definePluginStore()`** — define typed store shape (initialState + actions)
- **`usePluginStore<TState>()`** — subscribe to host-managed plugin state
- **`usePluginStoreApi<TState, TActions>()`** — get plugin store API with actions
- **`InferStoreState<T>` / `InferStoreActions<T>`** — extract types from store shape
- **`useCapability()`** — read runtime capabilities with optional selector
- **Capability hooks** — `useEnv`, `useUI`, `useStorage`, `useRequest`,
  `useFS`, `useDB`, `useClipboard`, `useDialog`, `useNotification`, `useNative`

Individual capability hooks are generated via factory functions
`createRequiredCapabilityHook` / `createOptionalCapabilityHook`.
Plugins can also use these factories to create custom hooks.

Desktop-first remains the architecture direction.
Current implemented runtime in this repository is web-based:

- App plugin mount: `renderWebAppPlugin(...)`
- Tool plugin execution: `runWebToolPlugin(...)`
- Permission-based capability injection via runtime context provider

Web prototype capability behavior:

| Permission     | Web host behavior                           |
| -------------- | ------------------------------------------- |
| `network`      | Uses browser `fetch`                        |
| `storage`      | Namespaced `localStorage` + zustand adapter |
| `store`        | Host-managed Zustand store for app plugins  |
| `fs`           | Simulated file read/write on `localStorage` |
| `clipboard`    | Browser clipboard API                       |
| `notification` | Notification API with toast fallback        |
| `dialog`       | Declared but not yet supported in web host  |
| `db`           | Declared but not yet supported in web host  |
| `native`       | Declared but not yet supported in web host  |

## 🔐 Permission System

Plugins declare required capabilities:

```
permissions: ['fs', 'network', 'db']
```

Runtime enforces:

- Capability injection
- Namespace isolation
- Access restriction
- Future user authorization prompts

Plugins cannot directly access:

- Tauri APIs
- Node APIs
- Native bindings

All access must go through the Flow Tool runtime.

## 💾 State & Database

### Store

App plugins use a host-managed Zustand store with declarative shape:

- Define store shape with `definePluginStore(initialState, actions)`
- Read reactive state with `usePluginStore<TState>()`
- Update state via `usePluginStoreApi<TState, TActions>()` actions
- Host creates one store per `pluginId`
- If `storage` permission is granted, host persists store by plugin namespace

Example:

```tsx
import {
  definePlugin,
  definePluginStore,
  usePluginStore,
  usePluginStoreApi,
} from '@flow-tool/sdk'

interface CounterState {
  count: number
}

const counterStore = definePluginStore<CounterState>({
  initialState: { count: 0 },
  actions: set => ({
    increment() {
      set(state => ({ count: state.count + 1 }))
    },
  }),
})

export default definePlugin({
  type: 'app',
  meta: {
    id: 'counter',
    name: 'Counter',
    version: '0.1.0',
    permissions: ['storage'],
  },
  store: counterStore,
  setup() {
    return function CounterPanel() {
      const { count } = usePluginStore<CounterState>()
      const { actions } = usePluginStoreApi<CounterState>()

      return <button onClick={() => actions.increment()}>Count: {count}</button>
    }
  },
})
```

### Database

Flow Tool uses SQLite (via Drizzle ORM).

Each plugin:

- Has isolated table namespace
- Cannot access other plugins’ data
- Managed migrations (future roadmap)

## ⚡ Native Capabilities

Rust/Tauri commands are provided by the host layer.

Plugins do not compile native code directly.

Instead:

```
Plugin → SDK → Runtime → Rust Core
```

Examples:

- File operations
- Media processing
- Encryption
- High-performance computation

## 📦 Plugin Registry & Loading

Plugins are managed through a central registry system:

- **`PluginRegistry`** — single source of truth for plugin state
- **`PluginLoader`** — async loading via manifest `loader()` functions
- **`CommandRegistry`** — command registration and search
- **`PluginLifecycleManager`** — orchestrates lifecycle hooks
- **`PluginErrorBoundary`** — catches rendering errors from plugin panels
- **`withWatchdog`** — wraps tool execution with timeout detection

Plugin states: `registered` → `loaded` → `enabled` → `disabled`

Built-in plugins are declared in `apps/web-vite/src/plugin/manifests.ts`
and loaded at startup via `bootstrap()`. External plugins can be loaded
dynamically via `import()` URLs.

Flow Tool ensures:

- Single React instance
- Dependency stability
- Runtime validation
- Error isolation per plugin

## ⌨️ Command Palette

The host includes a `Cmd/Ctrl+K` command palette (Raycast/uTools style):

- **Keyboard shortcut**: `Cmd+K` (macOS) / `Ctrl+K` (Windows/Linux)
- **Search**: matches title, description, and keywords
- **Navigation**: `↑↓` arrows, `Enter` to execute, `Esc` to close
- **Recent commands**: recently used commands shown first
- **Auto-registration**: all enabled plugins register commands automatically

UI component: `packages/ui/src/components/command-palette/`

## 🌍 Platform Roadmap

Current Focus:

- Desktop (Tauri)
- Web runtime

Planned:

- Plugin marketplace
- Mobile adaptation
- Advanced permission UI
- Worker-based tool isolation

## 🎯 Design Philosophy

Flow Tool is built around:

- Capability-based architecture
- Runtime dependency injection
- Plugin lifecycle control
- Minimal core, extensible ecosystem

It aims to evolve into:

> A lightweight extensible computing platform.

## 🚧 Status

Flow Tool is currently under active architectural development.

The focus is on:

- Plugin registry and dynamic loading
- Command palette and command dispatch
- Lifecycle management and error isolation
- Run history and settings persistence
- SDK design and permission system

## Documentation

- Architecture details: [`architecture.md`](./architecture.md)
- Contributor/agent guide: [`AGENTS.md`](./AGENTS.md)
- Plugin development: [`plugin.md`](./docs/plugin.md)

## License

[Apache 2.0 License](./LICENSE)

## 👨‍💻 Author

[HM Suiji](https://github.com/HM-Suiji)

Built with architectural obsession and system-level thinking.
