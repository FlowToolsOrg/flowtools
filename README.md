# Flow Tool

> An extensible, cross-platform toolbox powered by a plugin runtime.

Flow Tool is a desktop-first (Tauri-based) extensible tool platform that supports both instant “tool” plugins and long-running “app” plugins.  
It is designed around capability injection, permission control, and a unified React UI system.

Instead of shipping hundreds of built-in utilities, Flow Tool provides a stable runtime and SDK — tools are delivered as installable plugins.

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

<pre>
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
</pre>
	
Core principles:

- Single React tree
- Capability injection via runtime context
- Permission-gated resource access
- Namespaced storage & database
- Plugin lifecycle management

## 🧩 Plugin Model

Flow Tool supports two plugin types:

### 1️⃣ Tool Plugins (Instant Execution)

- Stateless
- No persistent runtime
- Optional UI
- Executes and exits

Example:

- Text conversion
- Sitemap extraction
- Hash calculation

### 2️⃣ App Plugins (Persistent UI)

- Long-running
- Store support
- Database access
- Lifecycle-managed

Example:

- Bookmark manager
- Clipboard history
- Task tracker

## 🛠 Plugin Development

Plugins are defined using the Flow Tool SDK:

```ts
import { definePlugin, useFS } from "@flow-tool/sdk"

export default definePlugin({
  type: "app",

  meta: {
    id: "example",
    name: "Example Plugin",
    version: "1.0.0",
    permissions: ["fs"]
  },

  setup() {
    return function Panel() {
      const fs = useFS()

      return <button onClick={() => fs.readFile()}>
        Read File
      </button>
    }
  }
})
```

### Tool Plugin Example

```ts
import { definePlugin } from '@flow-tool/sdk'

export default definePlugin({
	type: 'tool',

	meta: {
		id: 'hash',
		name: 'Hash Generator',
		version: '1.0.0',
	},

	async run(ctx, input) {
		return ctx.crypto.hash(input.text)
	},
})
```

## 🔐 Permission System

Plugins declare required capabilities:

```ts
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

Planned:

- Web runtime
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

## 📜 License

[Apache 2.0 License](./LICENSE)

## 👨‍💻 Author

Built with architectural obsession and system-level thinking.
