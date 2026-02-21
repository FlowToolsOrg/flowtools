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

## Current Status (February 2026)

- Product direction: desktop-first, cross-platform ready.
- Current runnable host: `apps/web-vite`.
- Core packages under active development:
  - `packages/sdk` (`@flow-tool/sdk`)
  - `packages/ui` (`@flow-tool/ui`)
- Local plugin workspace example:
  - `plugins/plugin-example-hello-world`
- Planned (not implemented yet in this repo):
  - `apps/desktop`
  - `apps/docs`

## Monorepo Layout

```text
apps/
  web-vite/    # web host prototype (router + runtime adapters)
  ui-test/     # UI package consumer and browser test app
packages/
  sdk/         # plugin contract, hooks, runtime provider, result helpers
  ui/          # shared React UI primitives
plugins/
  plugin-example-hello-world/
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
import { definePlugin, useStorage } from '@flow-tool/sdk'

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
      const storage = useStorage()

      return <button onClick={() => storage.set('hello', 'world')}>Save</button>
    }
  },
})
```

## Runtime Snapshot

Desktop-first remains the architecture direction.
Current implemented runtime in this repository is web-based:

- App plugin mount: `renderWebAppPlugin(...)`
- Tool plugin execution: `runWebToolPlugin(...)`
- Permission-based capability injection via runtime context provider

Web prototype capability behavior:

| Permission     | Web host behavior                           |
| -------------- | ------------------------------------------- |
| `network`      | Uses browser `fetch`                        |
| `storage`      | Namespaced `localStorage`                   |
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

App plugins may use scoped state:

- Namespaced per plugin
- Lifecycle-managed
- Optional persistence

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

## 📦 Plugin Loading

Plugins are:

- Installed locally
- Loaded dynamically (ESM)
- React-externalized
- SDK-externalized

Flow Tool ensures:

- Single React instance
- Dependency stability
- Runtime validation

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

- Stable plugin runtime
- SDK design
- Permission system
- State & DB isolation model

## Documentation

- Architecture details: [`architecture.md`](./architecture.md)
- Contributor/agent guide: [`AGENTS.md`](./AGENTS.md)

## License

[Apache 2.0 License](./LICENSE)

## 👨‍💻 Author

[HM Suiji](https://github.com/HM-Suiji)

Built with architectural obsession and system-level thinking.
