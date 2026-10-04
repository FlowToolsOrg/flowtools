# Flow Tool

> An extensible, cross-platform toolbox powered by a plugin runtime.
> Plugin Runtime for AI Age

Flow Tool is a plugin-driven utility platform focused on capability injection,
permission gating, and a unified React UI runtime.

The target production host is desktop (Tauri + Rust).
This repository currently contains a web runtime prototype and a desktop host
prototype. Neither is a production sandbox for untrusted plugins.

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

- Single React tree for trusted built-ins, not third-party isolation
- Capability injection via runtime context
- Permission-gated resource access
- Namespaced storage conventions; enforced database isolation remains planned
- Plugin lifecycle management

## Current Status (October 2026)

Current maturity is `prototype`. The accepted design separates Host/built-ins
from third-party UI, headless and legacy execution; its security controls are
not yet implemented. See [trust boundaries](./docs/adr/0001-plugin-trust-boundaries.md),
[capability/package policy](./docs/adr/0002-capability-and-package-policy.md),
[open threat register](./docs/security/threat-model.md) and the
[production roadmap](./docs/production-roadmap.md).
The SDK now exports `executePlugin()` for real app/tool `run()` calls with
schema validation/defaults, versioned execution metadata, stable failure codes,
cancellation and bounded asynchronous waiting. CLI, Web and Desktop use this
same boundary (P0.2b). Web/Desktop retain app panels and provide a shared
`ExecutionPanel` for actual JSON runs, errors and cancellation. Versioned
history stores at most 200 metadata-only attempts, including failures; inputs,
output values and exception messages remain transient. Unverified legacy keys
are left untouched and excluded from the new history. See the
[Windows host acceptance record](./docs/validation/p0-execution-hosts.md).
Native validation uses isolated test identities and a root-route query marker;
the harness checks initial launcher rendering before navigating. Maintainer
development-mode results and standalone packaged-app acceptance are recorded
separately; a successful build or URL-resolution test is not visual sign-off.
This helper does not isolate code or stop synchronous loops. Execution
metadata records input shape only, never raw input values or field names.
`bun run smoke:plugins` builds package prerequisites and exercises all twelve
real compiled entries through the CLI SDK runner with controlled in-memory
storage and scoped fixture responses, not public network or user data. It also
checks schema rejection, cancellation and actual plugin exceptions.
Todo JSON runs share the host app store with its panel (CLI retains its existing
validated `todos` key). Website latency uses SDK request with no raw fetch fallback.
The SDK's maturity vocabulary is `prototype`, `experimental`, `beta`,
`production`; missing metadata means `prototype`. All twelve built-ins and both
host manifests explicitly declare `prototype`, also shown by CLI list/info.
Compatibility evidence (`indexed` through `production-certified`) is separate
and never grants execution or proves maturity. Shared UI, Web and Desktop read
actual metadata through common maturity badges; missing values display Prototype,
never Stable. Compatibility evidence is displayed independently, not inferred
from maturity, support classification or catalog membership.
The version-1 portable HTML catalog stores package identity, relative paths and
scan hashes, never a checkout root or development URL. Its 125 prototype entries
contain 47 `entry-resolved` file receipts and 78 `indexed` records; these are not
runtime/API/security certification. `bun run verify:plugin-catalog` checks both
catalog copies and the controlled fixture digest without reading a local checkout.
Legacy local preview requires an explicit `VITE_HTML_PLUGIN_ROOT` in a development
server; the published catalog does not locate or authorize installed packages.
The HTML catalog is discovery evidence only, not a claim that 125 plugins are
compatible, secure or production-ready. Signed third-party code remains untrusted.

P0.3b1 closes the SDK/Web source entry by default: the ordinary SDK
`PluginFileLoader` always refuses with `EXTERNAL_CODE_DISABLED`, and no longer
exports transpilation/import-map injection. Web source preview requires both a
development server and explicit `VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW=1`; its unsafe
implementation is dynamically imported from `@flowtools/sdk/development` and
absent from the Web production bundle. The UI warns that unsigned code shares
the host realm and is not installed, isolated or granted. Old IndexedDB source
records are preserved, never automatically restored in any mode.

P0.3b2 binds CLI discovery/loading to a tracked, generated built-in inventory,
embedded in the CLI build and generated alongside Web/Desktop manifests. Runtime
directory additions and caller paths cannot add entries. Only fixed regular-file
`plugins/dist/<built-in-id>.js` artifacts are loaded; missing/broken artifacts,
directory junctions and inconsistent metadata fail closed. `list/info/run/help`
report missing builds instead of importing TSX or rewriting headless source.
Regenerate all three inventories with `bun run generate:manifests`, then run
`bun run build:packages` after changing built-ins. This is T1 build consistency,
not package authentication, TOCTOU protection or a third-party sandbox. Desktop
entry closure is completed by P0.3b3; the reusable artifact gate remains P0.3b4.
See the
[compiled CLI acceptance record](./docs/validation/p0-cli-compiled-inventory.md).

P0.3b3 closes Desktop HTML/Legacy execution by default, before activation,
HTML fetching or iframe creation. Saved enabled metadata and catalog evidence
cannot unlock it. The runner and bridge are development-only, dynamically loaded
behind DEV + `VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW=1`, with a visible unsigned,
shared-realm warning. Ordinary bridge APIs are deny-only. Even unsafe preview
does not expose raw invoke, SQL, arbitrary files or opener calls; fetch failure
does not fall back to opening a raw remote page. Built-in React/SDK flows remain.
All root gates, the production opt-in artifact comparison and three real catalog
route denials passed. The dedicated r3 native package also passed maintainer
manual acceptance, separately recorded from development screenshots. See the
[Desktop entry record](./docs/validation/p0-desktop-external-gate.md).

P0.3b4 adds `bun run verify:production-entrypoints` after the root build in CI.
It checks actual Web/Desktop files, known unsafe fingerprints and certification
syntax, then rebuilds with child-only development/path/URL/synthetic-key probes.
All bytes must match the ordinary production baseline; it is not signing or
security certification. The current 45 host artifacts passed; full milestone
verification is recorded in [the artifact gate](./docs/validation/p0-production-artifacts.md).

- Product direction: desktop-first, cross-platform ready.
- Current runnable hosts:
  - `apps/web-vite`: web runtime prototype.
  - `apps/desktop`: Tauri desktop shell with a HeroUI + Tailwind powered
    launcher surface, TanStack Router desktop routes, React/SDK plugin panels,
    and an explicitly enabled development-only HTML `main` preview; production
    external execution is disabled.
    Desktop native capabilities are installed through official Tauri plugins
    and exposed to plugins through the existing SDK capability contract.
- Core packages under active development:
  - `packages/sdk` (`@flowtools/sdk`) — plugin contract, hooks, registry, lifecycle, Zod-based inputSchema
  - `packages/ui` (`@flowtools/ui`) — shared UI components including CommandPalette
  - `packages/cli` (`@flowtools/cli`) — unified CLI entry (`flowtools list/info/run`), auto-generates flags from Zod schema
- HTML plugin compatibility foundation:
  - SDK types and normalizers for legacy HTML `plugin.json` metadata.
  - `bun run inspect:html-plugins` scans a local legacy HTML plugin checkout
    and writes `apps/desktop/src/data/html-plugin-catalog.json` for the
    launcher and `docs/html-plugin-catalog.json` as the readable catalog copy.
    The scan records the resolved static asset directory and marks source-only
    Vite entries as unavailable so desktop can fall back to `development.main`
    instead of loading `/src/main.ts` from the host app.
  - The current local scan found 125 HTML plugins and 723 commands.
  - `/run/$commandId` launches HTML `main` entries and pre-injects legacy host
    API globals expected by those plugins. Bridge calls are handled by the
    desktop SDK runtime context and then mapped to Tauri plugins where native
    access is required.
- Local plugin workspace with 12 built-in plugins (all app type, all CLI-compatible via `run()` + `inputSchema`):
  - `plugins/plugin-todo-list` — 待办清单（host-managed store）
  - `plugins/plugin-uuid-generator` — UUID 生成器
  - `plugins/plugin-hash-generator` — 哈希生成器
  - `plugins/plugin-text-ops` — 文本集合运算
  - `plugins/plugin-json-formatter` — JSON 格式化/压缩
  - `plugins/plugin-base64-encoder` — Base64 编解码
  - `plugins/plugin-timestamp-converter` — 时间戳转换
  - `plugins/plugin-color-converter` — 颜色格式转换
  - `plugins/plugin-random-picker` — 随机选取器
  - `plugins/plugin-regex-tester` — 正则表达式测试
  - `plugins/plugin-image-base64` — 图片 ↔ Base64
  - `plugins/plugin-website-latency` — 网站延迟测试
- Planned (directory created, not yet implemented):
  - `apps/docs`
  - `apps/web`

## Monorepo Layout

```text
apps/
  web-vite/    # web host prototype (router + registry + command palette)
  web/         # (planned)
  desktop/     # Tauri desktop shell, launcher, settings/run routes
    src/data/  # generated HTML plugin catalog consumed by the launcher
  docs/        # (planned) documentation site
  ui-test/     # UI package consumer with browser tests and manual validation
packages/
  sdk/         # plugin contract, hooks, registry, lifecycle, result helpers
  ui/          # shared React UI primitives (HeroUI-based)
  cli/         # unified CLI entry (flowtools list/info/run)
plugins/       # plugin workspace
configs/
  tsdown/      # shared package build config
```

## Development

Run from repository root:

```bash
bun install
bun run dev
bun run dev:desktop
bun run build
bun run lint
bun run check-types
bun run test
bun run format
bun run inspect:html-plugins
```

Useful local commands:

```bash
bun run dev --filter=@flowtools/web-vite
cd apps/ui-test && bun run dev
bun run --cwd apps/desktop tauri dev
bun run --cwd apps/desktop tauri add <plugin-name>

# CLI commands
bun run packages/cli/src/cli.ts list
bun run packages/cli/src/cli.ts run <plugin-id> --format text
```

## Validation

Run `bun run docs:check` to validate the ten core/design/review documents, their
inline local link paths and required threat fields. This read-only check does
not certify security, check remote URLs/Markdown anchors, or approve reviewers.
Capability/bridge changes must complete the security section in the
[PR template](./.github/pull_request_template.md) with actual review and
rejection-test evidence; branch protection is a separate administrator gate.

Automated tests are required. After installing dependencies, install the pinned
Chromium runtime once with `bun run --cwd apps/ui-test test:install-browser`.
On a fresh checkout, run `bun run build:packages` and `bun run generate:hosts`
before lint/type checks to generate their declaration, route, and IPC inputs.
Run `bun run lint`, `bun run check-types`, `bun run test`, and `bun run build`
from the repository root. `bun run verify:workspace-tasks` checks that all seven
workspaces expose the standard gates and reject empty-test success flags.

The uncached test graph waits for dependency tests before running consumers;
package contract tests build their own artifacts. Run root tests and builds
sequentially because package tests may clean their output. Coverage includes
SDK and CLI contracts, generated UI exports and
consumer declarations, built-in plugin inventory and CLI execution, Web Host
commands, headless Chromium UI interactions, and in-memory Rust persistence.
Manual routing, rendering, accessibility, and visual validation remain required
for changed UI flows. Passing these gates does not advance production maturity
without the remaining security, packaging, and recovery evidence in the roadmap.

Web manifest contracts run once per built-in plugin with a bounded 30-second
cold-import budget; bulk registration uses the same integration-test budget.
Ordinary unit tests keep their default timeout. Loading errors and contract
mismatches still fail without retries; these tests are not startup benchmarks.

Windows PR validation is defined in `.github/workflows/windows-quality.yml`.
From a clean checkout, run `pwsh -NoProfile -File scripts/check-ci.ps1` for the
same frozen install, browser setup, package/host prerequisites, seven-workspace gates,
Rust format/check/clippy, and clean-worktree checks. Package prerequisites are
also available as `bun run build:packages`; declaration consumers require these
artifacts before lint/type checks on a fresh checkout. `bun run generate:hosts`
generates Web/Desktop route trees and Rust-derived Desktop bindings without
launching a window or initializing user data. Host builds run their generators
before TypeScript checks. Bun is pinned by
`packageManager`; CI uses Rust 1.96.0. Actions use immutable commit references,
read-only permissions, and dependency/native compilation caches, not JS build
outputs or Turbo results. A repository administrator must separately require
the `Windows quality gates` check; a workflow file alone does not block merging.
Turbo keeps strict environment filtering, with explicit `PATHEXT` and native
build/test `CARGO_TARGET_DIR` passthrough for Windows tool discovery and caches.

## Plugin Model

Flow Tool supports two plugin categories:

- `app`: persistent panel plugins (`setup()` returns a React component), optionally with `run()` for CLI/headless invocation
- `tool`: instant execution plugins (`run(ctx, input)`)

Flow Tool also has an HTML plugin compatibility model for absorbing legacy
`plugin.json` metadata. HTML plugins are first normalized into searchable
FlowTools descriptors, then classified by required runtime support:

- `webview`: `main` can be hosted by a Tauri WebView shell.
- `preload-bridge`: requires a legacy host API / preload bridge.
- `native-bridge`: requires Tauri native capability work for files, images,
  windows, screenshots, or other host-level APIs.
- `metadata`: useful for indexing or headless rewrite, but no UI entry exists.

Desktop HTML execution is a compatibility layer, not a second plugin model:
it is disabled by default. Only explicit unsafe development preview uses an
iframe and a limited UI/clipboard/dialog/notification bridge. Raw native,
SQL, filesystem and opener calls remain disabled even there. The broader SDK
capability adapter is for host-bound built-ins, not third-party authorization.
Local preview requires a built static entry and `VITE_HTML_PLUGIN_ROOT`;
portable catalogs do not publish or automatically load development URLs.

All plugins can declare `inputSchema` (Zod `z.object({...})`) for:

- CLI auto-generated flags
- Runtime input validation
- TypeScript type inference

Minimal app plugin example:

```tsx
import { definePlugin, useCapability } from '@flowtools/sdk'
import { result } from '@flowtools/sdk/result'
import { Button } from '@flowtools/ui'
import { z } from 'zod'

const inputSchema = z.object({
  initial: z.string().default('').describe('Initial value'),
})

export default definePlugin({
  type: 'app',
  meta: {
    id: 'example-app',
    name: 'Example App',
    version: '0.1.0',
    permissions: ['storage'],
  },
  inputSchema,
  async run(_ctx, input) {
    return result.json({ received: input.initial })
  },
  setup() {
    return function Panel() {
      const { storage } = useCapability()

      return <Button onPress={() => storage.set('hello', 'world')}>Save</Button>
    }
  },
})
```

## CLI

`packages/cli` provides a unified CLI entry for invoking plugins without UI:

```bash
# List all CLI-compatible plugins
bun run packages/cli/src/cli.ts list

# Show plugin details
bun run packages/cli/src/cli.ts info plugin-uuid-generator

# Execute a plugin with auto-generated flags
bun run packages/cli/src/cli.ts run plugin-uuid-generator --count 5

# Execute with raw JSON input
bun run packages/cli/src/cli.ts run plugin-uuid-generator \
  --input '{"count": 5}'

# Output as text (flags default to text; --input defaults to JSON)
bun run packages/cli/src/cli.ts run plugin-uuid-generator --count 3 \
  --format text
```

CLI flags are auto-generated from each plugin's `inputSchema` (Zod).
Input is validated with `z.safeParse()` before execution.
CLI JSON now returns the shared execution envelope: success has `data` containing
the actual `CommandResult`; failure has `error.code/message`, is printed as JSON
to stdout and exits with status 1. Both include real plugin version/timing and
safe input-shape metadata. This is a prototype API change from bare result JSON;
text output keeps its formatter. Non-serializable JSON output fails explicitly
with `OUTPUT_INVALID`. `@flowtools/sdk/execution` is a non-React runtime subpath.
The CLI context exposes only declared built-in storage/network capabilities,
uses SDK `remove/zustand`, does not create its own execution timer, and does not
emit raw plugin logs. Existing valid storage filenames remain unchanged; invalid
path/reserved-device keys fail. This is not third-party authorization/isolation.
Desktop Tauri host can invoke CLI via `Command::new("flowtools")` and parse JSON output for AI agent integration.

## SDK Hooks

`@flowtools/sdk` exposes:

- **`definePluginStore()`** — define typed store shape (initialState + actions)
- **`usePluginStore<TState>()`** — subscribe to host-managed plugin state
- **`usePluginStoreApi<TState, TActions>()`** — get plugin store API with actions
- **`InferStoreState<T>` / `InferStoreActions<T>`** — extract types from store shape
- **`useCapability()`** — read runtime capabilities with optional selector
- **Capability hooks** — `useEnv`, `useUI`, `useStorage`, `useRequest`,
  `useFS`, `useDB`, `useClipboard`, `useDialog`, `useNotification`, `useNative`
- **`z`** — re-exported from Zod (convenience for plugin `inputSchema`)

Subpath imports:

- **`@flowtools/sdk/definePlugin`** — standalone `definePlugin` import
- **`@flowtools/sdk/result`** — `result.text/json/table/open/multi` helpers

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

Desktop capability behavior:

| Permission     | Desktop host behavior                                   |
| -------------- | ------------------------------------------------------- |
| `fs`           | `@tauri-apps/plugin-fs`                                 |
| `clipboard`    | `@tauri-apps/plugin-clipboard-manager`                  |
| `dialog`       | `@tauri-apps/plugin-dialog`                             |
| `notification` | `@tauri-apps/plugin-notification`                       |
| `storage`      | SDK-sync `localStorage` adapter mirrored to Tauri Store |
| `db`           | `@tauri-apps/plugin-sql` SQLite adapter                 |
| `network`      | browser/WebView `fetch`                                 |
| `native`       | `@tauri-apps/api/core.invoke`                           |

## 🔐 Permission System

Plugins declare required capabilities:

```
permissions: ['fs', 'network', 'db']
```

The current runtime provides cooperative SDK capability injection and storage
key prefixes based on manifest declarations. It does not enforce per-package
user grants or protect the host realm against hostile plugin code.

The production design requires isolated execution plus Rust-side identity,
grant and scope validation on every sensitive operation. Raw `native.invoke`,
SQL and path adapters currently present are prototype gaps, not supported
third-party production APIs. SDK-only imports are a development convention;
they do not prevent same-realm code from bypassing the SDK.

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
} from '@flowtools/sdk'

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

Flow Tool uses SQLite for desktop-hosted plugin metadata and plugin-facing
database capability work.

The current plugin-facing adapter shares a SQLite connection and accepts raw
SQL; cross-plugin database isolation is not enforced. Host metadata repository
tests do not certify plugin data access. Enforced namespaces, versioned
migrations, backup and recovery are required future gates.

Desktop plugin metadata is moving into the Tauri/Rust backend. The Rust side
stores FlowTools-compatible manifest fields such as `id`, `name`, `version`,
`type`, `permissions`, `tags`, `category`, `icon`, `cliAvailable`, and lifecycle
`state`. Tauri commands currently expose plugin list/get/add/update/remove plus
enable/disable operations; frontend callers should use the generated
`apps/desktop/src/utils/bindings.ts` command helpers when available.

## ⚡ Native Capabilities

Rust/Tauri commands are provided by the host layer.
For standard desktop features, prefer official Tauri plugins installed with
`bun run --cwd apps/desktop tauri add <plugin-name>` and adapt them through the
SDK capability context before adding custom Rust commands.

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

Desktop persists plugin records in the Rust backend database. The persisted
`state` uses the same state vocabulary as the SDK registry
(`registered`, `loading`, `loaded`, `enabled`, `disabled`, `error`) so UI state
and backend state can converge without type translation.

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
- CLI entry for plugin invocation and AI agent bridge

## Documentation

- Architecture details: [`architecture.md`](./architecture.md)
- Contributor/agent guide: [`AGENTS.md`](./AGENTS.md)
- Plugin development: [`plugin.md`](./docs/plugin.md)
- Security baseline: [threat model](./docs/security/threat-model.md) and
  [production roadmap](./docs/production-roadmap.md)

## License

[Apache 2.0 License](./LICENSE)

## 👨‍💻 Author

[HM Suiji](https://github.com/HM-Suiji)

Built with architectural obsession and system-level thinking.
