# Repository Guidelines

## Project Structure and Module Organization

This repository is a Bun + Turbo monorepo.

- `packages/sdk`: plugin-facing SDK contract, hooks, runtime provider,
  registry system, result helpers, lifecycle management, and Zod-based
  `inputSchema` for CLI flag generation.
- `packages/ui`: shared React UI primitives (HeroUI-based), including
  `CommandPalette`, `ToolCard`, `RunPanel`, `SettingsCenter`, etc.
- `packages/cli`: unified CLI entry (`flowtools list/info/run`), Commander-based,
  auto-generates flags from plugin `inputSchema` (Zod), validates input at runtime,
  outputs structured `result.*` in JSON or text format.
- `apps/web-vite`: runnable host prototype (web runtime + routing + command
  palette + plugin registry).
- `apps/desktop`: Tauri desktop shell. It hosts the HeroUI + Tailwind
  launcher, uses TanStack Router for desktop pages, renders React/SDK plugin
  panels, and reads the generated HTML plugin catalog. Desktop native
  capabilities should be added with official Tauri plugins and adapted through
  the SDK runtime context. Desktop plugin metadata persistence lives in
  `src-tauri/src/models/plugin.rs`, `repositories/plugin_repository.rs`, and
  `commands/plugin_commands.rs`; keep Rust DTO fields aligned with frontend
  plugin manifest/state types.
- `apps/ui-test`: consumer app for manual validation of `@flowtools/ui`.
  Manual UI validation remains required and complements automated coverage; it
  does not replace it.
- `plugins/`: local plugin workspace with 12 built-in plugins (all app type,
  all CLI-compatible via `run()` + `inputSchema`).
- `configs/tsdown`: shared package build config.
- `docs/` and root docs such as `README.md` and `architecture.md`: product and
  architecture references.
- `scripts/inspect-html-plugins.ts`: scans a local HTML plugin checkout and
  writes `apps/desktop/src/data/html-plugin-catalog.json` and
  `docs/html-plugin-catalog.json`.

Planned but not yet present in this repo:

- `apps/docs`

Keep reusable logic in `packages/*`; keep host-specific behavior in `apps/*`.

## References

You should reference the following files when changing architecture or developer
workflows:

- [Structure](./docs/structure.md)
- [Architecture](./architecture.md)
- [README](./README.md)
- [Plugin](./docs/plugin.md)
- [Production Roadmap](./docs/production-roadmap.md)
- [Ordered Implementation Plan](./docs/next-milestones.md)
- [Trust Boundaries ADR](./docs/adr/0001-plugin-trust-boundaries.md)
- [Capability and Package Policy ADR](./docs/adr/0002-capability-and-package-policy.md)
- [Threat Model](./docs/security/threat-model.md)

## Build, Validation, and Development Commands

Run from repository root:

- `pwsh -NoProfile -File scripts/check-ci.ps1`: run the Windows CI gates from a
  clean checkout; each native nonzero exit terminates the task and generated
  tracked/untracked file drift fails validation.
- `bun run build:packages`: bootstrap SDK/UI/CLI/plugins/Runtime client artifacts
  before lint/type checks in a fresh checkout.
- `bun run generate:hosts`: generate Web/Desktop route trees and Desktop Rust
  command bindings before lint/type checks in a fresh checkout. Never replace
  Rust-derived bindings with handwritten DTO copies. Bindings generation uses
  a codegen-only binary and mock runtime, not desktop launch or user databases.
- Runtime client consumers must declare their workspace dependency and use its
  public exports. Bootstrap its artifacts before lint/types in a fresh checkout.
  The three Rust-derived Runtime source artifacts have explicit LF Git attributes
  so Windows CRLF checkout plus LF codegen leaves the worktree clean. Preserve
  the content-drift gate; do not refresh/stage generated files to bypass it.
  Package-specific uncached native build tasks must explicitly retain `^build`
  and `CARGO_TARGET_DIR` passthrough; Turbo overrides do not inherit them.
- `bun run dev`: starts workspace `dev` tasks via Turbo.
- `bun run build`: builds workspaces (`turbo run build`).
- `bun run lint`: runs workspace lint tasks.
- `bun run check-types`: runs workspace type checks.
- `bun run test`: runs required workspace automated tests via Turbo.
- `bun run smoke:plugins`: builds prerequisites and validates all twelve actual
  compiled entries with controlled request/storage, schema rejection and abort.
- `bun run docs:check`: read-only core/ADR/threat/PR-template contracts and inline
  local link path checks; no remote URL, anchor or security certification.
- `bun run format`: formats tracked source/document files.
- `bun run inspect:html-plugins`: scans a local HTML plugin checkout and
  regenerates `apps/desktop/src/data/html-plugin-catalog.json` plus
  `docs/html-plugin-catalog.json`.
- `bun run verify:plugin-catalog`: read-only portable version-1 catalog, identity,
  path, fixture digest and duplicate-copy gate; no live checkout or runtime/API/
  security certification. It runs before lint in Windows CI.
- `bun run verify:production-entrypoints`: run after root build; inspect actual
  Web/Desktop output, then rebuild with child-only opt-in and synthetic canaries.
  Require byte-identical artifacts, no known unsafe fingerprints/certification
  switches or canary leakage. No raw artifact import, native launch or user DB.
  This rebuild gate is not signing, publisher verification or sandbox evidence.
- `cd apps/desktop && bun run tauri add <plugin-name>`: install official Tauri
  plugins for desktop native capability work before adding host-side adapters.

Useful app-level commands:

- `cd apps/web-vite && bun run dev`
- `cd apps/ui-test && bun run dev`
- `cd apps/desktop && bun run dev`

Every workspace must expose scripts named exactly `lint`, `check-types`,
`build`, and `test`, even when a task is intentionally lightweight. Root Turbo
tasks must invoke those names consistently across the monorepo. Do not use
workspace-only aliases such as `check:types` as substitutes for the standard
task names.

CLI discovery/loading is bound to generated `packages/cli/src/builtin-manifests.ts`,
embedded in the CLI build. Never restore runtime source scans, arbitrary ID/path
imports, TSX fallback or regex headless rewriting. Only fixed regular-file
`plugins/dist/<known-id>.commands.js` artifacts are accepted; unknown IDs fail before IO,
missing/broken entries fail without source execution, and junction/symlink
redirection is rejected. This T1 consistency check is not signing, a sandbox or
protection against replacing trusted compiled files. CLI tests build their own
CLI artifacts and run disposable compiled-entry rejection fixtures. Build the
plugin artifacts before using `list/info/run/help`; missing builds are errors.

CLI commands (from repo root):

CLI `--format json` returns `PluginExecutionResult`, not bare `CommandResult`;
read actual results from `data` and stable failures from `error.code`. Failures
must emit JSON to stdout and exit nonzero. Preserve text formatter behavior.
Use `@flowtools/sdk/execution` for non-React execution; CLI context construction
must not own a timeout or expose undeclared built-in capabilities. Declarations
are not user grants; do not describe this adapter as third-party isolation.

- `bun run packages/cli/src/cli.ts list` — list CLI-compatible plugins
- `bun run packages/cli/src/cli.ts info <plugin-id>` — show plugin details
- `bun run packages/cli/src/cli.ts run <plugin-id> --format text` — execute a plugin

If dependencies change, run `bun install`.

Before running browser tests on a new machine, run
`bun run --cwd apps/ui-test test:install-browser`. The Playwright version is
pinned; do not substitute an arbitrary system browser for the regression gate.
Keep browser test files serial (`fileParallelism: false`) to bound Windows
page/transform memory; every test file and assertion still runs.
Root Turbo tests depend on dependency tests (`^test`); package contract tests
build their own artifacts before assertions. Tests are uncached so a successful
cached result cannot hide missing build output. Run root `test` and `build`
sequentially, never concurrently: package tests may clean their own `dist`.
Test scripts must fail when no tests are collected; never use
`--pass-with-no-tests` or equivalent flags. Keep Bun test globals in test-only
type configurations so browser production sources cannot silently use Bun APIs.

Windows PR CI lives in `.github/workflows/windows-quality.yml`. Keep Actions
pinned to full commit SHAs, PR permissions read-only, and gate failures fatal.
Do not use `pull_request_target` for executing contributor code. Never cache
`node_modules`, JavaScript `dist`, or `.turbo` results as a substitute for gates.
CI uses the Bun version in root `packageManager` and pinned Rust 1.96.0. Changes
to triggers, tooling, caches, failure propagation, or drift checks require
regression coverage in `scripts/ci-contracts.test.ts`, run by Desktop tests.
Validate workflow expression context availability as well as YAML syntax:
`runner` is unavailable in job-level `env`; use step-level `env` for paths
derived from `runner.temp`. Check workflow changes with actionlint before push.
Keep Turbo strict environment mode. Pass through Windows `PATHEXT` for native
command discovery and `CARGO_TARGET_DIR` for build/test native cache placement;
do not pass through host secrets or switch to loose mode to fix tool discovery.
Remote CI acceptance and required status-check settings are separate roadmap
work; local commit permission alone does not authorize pushes or repository
settings. The standing project authorization below permits feature-branch pushes
and PR creation; repository settings still require separate authorization.
Windows MSVC binding integration tests require the Common Controls v6 manifest
directives in `src-tauri/build.rs`; Tauri's app manifest does not cover them.
Keep these directives test-target scoped to avoid duplicate app manifests.
Production artifact refusal is a fatal post-build CI gate, before the final
clean-worktree check. Preserve standard test/build order, strict Turbo env and
all service-level rejection/mode tests; fingerprint scans cannot replace them.

## Production Maturity Labels

G3 P2.3a's T1 policy broker is in
`packages/runtime-core/src/broker.rs`. Identity comes from the authenticated Host
caller and embedded catalog, never operation payloads. Rust generates operation
descriptors, schemas and rejection fixtures. The broker's direct approve/revoke
APIs stay internal to T0 Rust; clients use the separate durable management policy,
never run flags or metadata grants. Preserve
manifest AND grant scopes, exact package identity, epochs, expiry and per-run call
budgets. Check runner sessions before launch, during execution and at each effect
commit; async adapters cannot reuse a cached approval. P2.6a/P2.4a now supply the
single writer and durable management policy for fixed Windows/T1 prototype
execution: pure commands and the typed Todo data adapter. File/tool descriptors
are not IO adapters and network origin matching is not DNS/redirect enforcement.
This does not enable third-party or production execution.
See [P2.3a evidence](./docs/validation/g3-capability-broker.md).

### Accepted next-stage product and runtime design

Follow `docs/next-milestones.md` in G0–G8 dependency order. The target audience is
non-specialist knowledge/office users; external agents use the same command
contracts as the GUI. Independently installable CLI, headless/background Host,
versioned plugin services and centrally managed shared binary tools are planned,
not implemented production controls. Low-code and built-in AI/model plugins stay
future scope. Do not silently implement them while closing current milestones.

Keep React UI and command execution in separate artifacts. Cold-start support
is a declaration; it does not grant unattended execution or file/network/tool
access. Overwrite, delete and send require explicit approval/grants. Runtime
owns data, tasks and policy; Desktop/CLI clients must not create duplicate stores.
Preserve metadata-only history; durable job payloads need separate private storage.
Resolve service/tool dependencies to Host-generated immutable locks. Share tool
files, never grants, environment, working directories or plugin identities.
Do not expose arbitrary shell, executable paths, raw argv or self-managed daemons.
Runtime/tool side-loading remains denied until package and platform gates pass.
Keep planned module paths distinguishable from the current project structure.

Use these labels consistently in manifests, catalogs, documentation, UI, and
the production roadmap:

- `prototype`: validates an idea; interfaces and data may change without a
  migration path, and production use is unsupported.
- `experimental`: runnable for controlled evaluation, with explicit known
  limitations and incomplete compatibility or hardening.
- `beta`: feature-complete for the declared scope, with automated regression
  coverage and documented upgrade, security, and recovery constraints.
- `production`: passes all required automated and manual gates, has secure
  defaults, compatible data migrations, operational diagnostics, and a
  documented support policy.

Catalog presence, parsed metadata, command discovery, or a successful build is
not evidence of compatibility or production readiness. Advance a maturity
label only when the declared scope has objective validation evidence.

Use SDK `PluginMeta.maturity` / `PluginManifestEntry.maturity` and
`pluginMaturitySchema` from `@flowtools/sdk/types`; omission resolves to
`prototype` via `resolvePluginMaturity`, never stable. The old PluginMeta
`status: stable/deprecated` vocabulary is removed. Compatibility evidence uses
its own schema and does not authorize execution or certify security.
`bun run generate:manifests` regenerates Web/Desktop manifests and CLI inventory
from the same built-in metadata in sorted order. Desktop directly depends on
the plugins workspace; retain that dependency so types/tests do not rely on an
undeclared resolution side effect. Catalog/UI consume this SDK maturity contract.
`ToolStatus` and `ToolMarketStatus` are aliases of `PluginMaturity`; their existing
status prop represents only maturity. Use shared `PluginMaturityBadge` (omission
displays Prototype) and `PluginCompatibilityBadge` for independent evidence.
Support/bridge requirements are not evidence or security approval. Never restore
hardcoded stable labels. Keep real metadata, default and evidence UI regressions.
`bun run scripts/generate-manifests.ts --check` is a read-only generated-content
gate; the generator formats output before comparing or writing all three outputs.

## Coding Style and Naming Conventions

- TypeScript strict mode is enabled; keep code type-safe and avoid `any`.
- Formatting is enforced by `oxfmt` + Prettier conventions: 2 spaces,
  single quotes, no semicolons, max width 80.
- Linting is via `oxlint`; fix warnings before opening a PR.
- Use lowercase filenames for modules (for example `button.tsx`) and
  PascalCase for exported React components (for example `Button`).
- Keep plugin IDs and command IDs kebab-case (for example `hash-generator`).
- For plugin state in app plugins, use `definePluginStore()` to declare typed
  store shape (initialState + actions), then attach via `AppPlugin.store`.
  Derive types with `InferStoreState<T>` / `InferStoreActions<T>`.
  Consume state with `usePluginStore<TState>()` and dispatch actions via
  `usePluginStoreApi<TState, TActions>().actions.xxx()`.

## Plugin Registry & Loading

Plugins are managed through a central registry system:

- `PluginRegistry`: single source of truth for plugin state
  (`registered` → `loaded` → `enabled` → `disabled`).
- `PluginLoader`: handles async loading via manifest `loader()` functions.
- `CommandRegistry`: registers and searches commands from all plugins.
- `PluginLifecycleManager`: orchestrates `onLoad/onUnload/onActivate/onDeactivate`.
- `PluginErrorBoundary`: catches rendering errors from plugin panels.
- `withWatchdog`: wraps tool execution with timeout detection.
- `executePlugin`: shared app/tool schema-validation and execution envelope;
  use it for new run adapters instead of fabricating success or timing. It
  bounds asynchronous waiting and forwards cancellation, but is not a sandbox
  or a hard stop for synchronous code. Persist only safe input-shape metadata.

HTML plugin compatibility is handled as an import/compatibility layer, not as direct
execution bypassing the SDK:

- Use `packages/sdk/src/compat/html-plugin.ts` to parse and normalize HTML
  `plugin.json`.
- Treat `webview`, `preload-bridge`, `native-bridge`, and `metadata` as the
  support levels.
- Desktop `/run/$commandId` refuses HTML/Legacy execution by default, before
  activation or loading. Only DEV + explicit unsafe opt-in may preview a `main`
  through the development runner/bridge; catalog and saved metadata never grant.
- The generated HTML catalog records whether a static `main` is actually
  runnable from the scanned checkout. Source-only Vite entries such as
  `/src/main.ts` or `/main.tsx` should fall back to `development.main` or a
  built artifact instead of being loaded from the FlowTools dev server root.
- Catalog metadata is discovery evidence only. Do not describe a plugin as
  compatible, runnable, secure, or production-ready merely because it appears
  in `apps/desktop/src/data/html-plugin-catalog.json` or
  `docs/html-plugin-catalog.json`. Verify each declared runtime, bridge,
  capability, platform, and packaged artifact before making compatibility
  claims.

### Third-Party Plugin Security Defaults

SDK `PluginFileLoader` is now deny-only (`EXTERNAL_CODE_DISABLED`) before file
reads, registry writes or lifecycle calls. Do not re-enable it with metadata,
certification flags or a caller-supplied mode. Source transpilation/import-map
injection is not exported by the ordinary SDK. The unsafe development subpath
`@flowtools/sdk/development` requires DEV plus the exact opt-in
`VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW=1`; Web imports it dynamically inside a DEV
guard and displays the unsigned, same-realm risk. Never statically import that
subpath into a host. Web does not automatically restore external source in any
mode; preserve the old IndexedDB records without executing or deleting them.
Service-level rejection and build-mode regressions are required, not only a
hidden upload button. CLI accepts only its fixed compiled T1 inventory (P0.3b2);
Desktop HTML/Legacy runner and bridge follow the same default denial (P0.3b3).
Keep their implementations in development-only dynamic imports behind DEV and
the exact opt-in. Ordinary HTML bridge APIs always refuse before metadata or
payload reads. Saved enabled metadata, catalog evidence, query flags and claimed
certification never authorize an iframe, preload, fetch or native operation.
Unsafe preview must visibly warn about unsigned/shared-realm code; raw invoke,
SQL, arbitrary FS and opener bridge methods remain unavailable even there.
Fetch failure must not fall back to unbridged remote src. Retain independent
runner/bridge build-mode and rejection tests; this is not an isolated session.
`test/html-mode-build.ts` owns each actual compilation in a fresh process with
the Desktop working directory; repeated in-process Bun.build after SDK imports
has a reproduced Windows file-cache failure. Keep all assertions and normal
unit-test timeouts; this compiler subprocess is not a plugin sandbox.

Portable catalogs use `@flowtools/sdk/compat/catalog`, logical source identity,
package-relative paths and scan SHA-256 values. All current HTML entries remain
prototype. The implemented evidence gate accepts only indexed or entry-resolved
file evidence; it rejects api-verified/production-certified claims until their
real certification protocol exists. Controlled fixture text hashes normalize
CRLF to LF; scanned artifact hashes retain actual bytes. Never publish checkout
roots or development URLs. Local preview needs an explicit development-only
`VITE_HTML_PLUGIN_ROOT`; this path setting is not a sandbox or package grant.

ADR-0001/0002 are accepted production designs, not evidence that isolation,
grants, signed installation or recovery already work. The threat register tracks
current gaps and their implementation owners. Keep threat IDs stable; add source
and rejection-test evidence when changing an entry point. Do not close a risk or
advance maturity merely because documentation or CI passes.

- T1 built-ins may share the main React tree. T2 UI and TL legacy code require
  isolated origins and sessions; T3 requires a restricted terminable runner.
  Workers and ordinary child processes are not automatically OS sandboxes.
- Derive plugin identity from host-bound sessions, not request payloads. A
  signature proves package/publisher identity, not trust or authorization.
- Treat third-party plugins as untrusted and deny capabilities by default.
  Grant only explicit, user-approved, least-privilege scopes.
- Production installation and execution require signed packages plus verified
  integrity, publisher provenance, and host-version compatibility. Unsigned
  plugins are limited to an explicit development mode with a visible warning.
- Never expose raw `native.invoke`, raw SQL, or unscoped filesystem access to a
  plugin. Use typed allowlisted capability adapters, parameterized and
  plugin-namespaced database operations, and canonicalized scoped paths.
- Isolate untrusted plugin code from host secrets and privileged APIs. A
  manifest capability declaration is a request, not proof of authorization.
- Persist grants per plugin identity and package version, support revocation,
  and record security-relevant decisions in an auditable log.

Built-in plugins are declared in `apps/web-vite/src/plugin/manifests.ts`
and loaded at startup via `bootstrap()`.

Host UI reads plugin/command/settings state from Zustand stores:

- `pluginRegistryStore`: reactive plugin list
- `commandStore`: command palette items + execution
- `settingsStore`: persisted host settings

Execution UI uses `ExecutionPanel` from `@flowtools/ui`, bound to the real
host adapter and `executePlugin()` from `@flowtools/sdk/execution`. Use
`createExecutionHistory()` for stable external-store snapshots, formatVersion 1,
bounded metadata-only records and visible persistence errors. Web uses
`flowtools-web-run-history-v1`; Desktop uses `flowtools-desktop-run-history-v1`.
Never persist raw input/output or exception messages. Do not import/delete
unverified legacy history; preserve the old key for deliberate recovery.

Desktop routes are owned by `apps/desktop` and should use TanStack Router. The
current routes are `/`, `/settings`, `/plugins`, `/permissions`, and
`/run/$commandId`. For React/SDK app plugins, `/run/$commandId` must render the
plugin panel through the SDK runtime provider. For HTML plugins with a `main`
entry, `/run/$commandId` denies execution by default and explains why. Only DEV
plus explicit unsafe opt-in may dynamically launch the development runner;
catalog or saved activation state cannot grant production execution.

Desktop native capability work should prefer official Tauri plugins installed
with `bun tauri add` (`fs`, `dialog`, `clipboard-manager`, `notification`,
`sql`, `store`, `opener`, etc.). Expose those APIs to plugins only through
`PluginRuntimeContextValue` capability adapters such as
`apps/desktop/src/runtime/desktop-capabilities.ts`.

Desktop Rust plugin database commands should keep plugin ids kebab-case, plugin
types limited to `app` / `tool`, and persisted state aligned with SDK
`PluginState` (`registered`, `loading`, `loaded`, `enabled`, `disabled`,
`error`). Prefer repository-layer validation before exposing new Tauri commands.

## Command Palette

The host includes a `Cmd/Ctrl+K` command palette:

- UI component: `packages/ui/src/components/command-palette/`
- Keyboard shortcut: `Cmd+K` (macOS) / `Ctrl+K` (Windows/Linux)
- Commands auto-registered from all enabled plugins
- Search matches title, description, and keywords
- Recent commands shown first

## Testing Guidelines

Web built-in manifest contracts must report each plugin separately. Actual
package imports and bulk registration have a bounded 30-second integration-test
timeout to accommodate cold Windows runners. Keep normal unit-test timeouts;
do not skip assertions, retry failures, or treat this budget as a startup SLA.

Automated tests are a required production quality gate. Add regression coverage
with every behavior change or defect fix in these areas:

- SDK contracts, schemas, result helpers, runtime providers, and capability
  adapters
- CLI parsing, generated flags, validation, output formats, exit codes, and
  failure behavior
- plugin and command registries, lifecycle transitions, loading races,
  concurrency, cancellation, and watchdog timeouts
- permission enforcement and security boundaries, including path traversal,
  scope isolation, unsafe bridge calls, and injection attempts
- persisted state, schema and data migrations, rollback or recovery behavior,
  and compatibility across supported versions

Manual UI validation remains required for visual behavior, accessibility,
desktop and web routing, and plugin rendering. It complements automated tests
and is not a substitute for them.

Current validation gate:

1. `bun run lint`
2. `bun run check-types`
3. `bun run test`
4. `bun run build` or the relevant app/package build command
5. Manual validation for changed flows (for web host, verify routes and plugin
   rendering in `apps/web-vite`)

The reproducible Web/Tauri execution acceptance harness is
`apps/ui-test/scripts/validate-execution-hosts.ts` (run with Node, not Bun).
Follow `docs/validation/p0-execution-hosts.md`. Never launch the default Debug
desktop against user data. Startup now preserves its database; use the
dedicated validation config, fresh test identity/profile and loopback-only
child-process CDP; never persist remote debugging in production configuration.
Screenshot inspection and keyboard checks do not certify NVDA or other platforms.
For maintainer manual acceptance, use `tauri.manual-validation.conf.json` in
`apps/desktop`, verify its test identity and title, and use fixture data only.
It is visible, adds no remote debugging, and does not change production config.
Validation URLs must use `/?execution-validation=...`, not
`index.html?execution-validation=...`: the latter enters an unmatched route.
The native harness must check initial launcher rendering before navigation.

## Documentation Sync (Required and !Important)

When code includes major refactoring or important new features, update these
files in the same change:

- `README.md`
- `AGENTS.md`
- `architecture.md`
- `docs/structure.md`
- `docs/plugin.md`
- `docs/production-roadmap.md`

## Commit and Pull Request Guidelines

The maintainer authorized automatic project PR delivery on 2026-10-05. After
completing each requested task and its applicable checks, make focused commits,
push the `codex/` feature branch to `origin`, and create or update a PR against
the verified default branch using the repository template. Return its URL and
current validation/review status without asking again for push or PR permission.
Record incomplete acceptance or independent security review explicitly; use a
draft when implementation or required validation remains incomplete. This
authorization does not include merging, deployment, repository settings changes
or bypassing checks. Later explicit user instructions take precedence.

Use Conventional Commits, for example:

- `feat(ui): add plugin card variants`
- `fix(sdk): guard missing capability`

Complete production-roadmap milestones incrementally. Each completed milestone
must be independently validated and recorded in one focused Conventional
Commit before work begins on the next milestone. Do not bundle multiple
completed roadmap milestones into one commit.

Phase 0 continuation uses P0.2a/b/c and P0.3a/b/c from the roadmap. Implement
the shared SDK executor before changing host execution, then validate built-in
smoke fixtures before maturity/catalog and production-entry gates. Execution
tests must call the real plugin implementation; controlled capability adapters
are permitted, replacing `run()` with synthetic success is not. Keep maturity
separate from compatibility evidence and do not mark Phase 0 done early.

Keep `plugins/test/smoke-fixtures.ts` exactly aligned with CLI discovery and
built-in entries; missing fixtures fail the smoke gate. Network fixtures must
use the SDK request capability and reserved `.invalid` URLs, not global fetch
or live websites. Todo run and setup share the declared host store; the CLI
legacy storage key is validated before writes, never silently overwritten on
corruption. Do not claim this is automatic migration or a persistent grant.

PRs should include:

- clear summary and scope
- linked issue (if applicable)
- screenshots/GIFs for UI changes
- notes on plugin/runtime impact and validation steps
- documentation sync notes when architecture/runtime behavior changes

Use `.github/pull_request_template.md`. Capability, bridge/IPC, manifest/package,
isolation, persistence, grant, file/network and update changes require threat
IDs, ADR impact, host-bound identity/scope, rejection tests, revocation/recovery,
residual risk and an actual security reviewer/date/conclusion. Explain concrete
non-applicability for changes outside these boundaries. `docs:check` validates
the template and threat fields, not reviewer approval or branch protection;
those require separate maintainer setup and acceptance. Run the document gate
before lint/type checks; Windows shared CI does the same.

<!-- HEROUI-REACT-AGENTS-MD-START -->

[HeroUI React v3 Docs Index]|root: ./.heroui-docs/react|STOP. What you remember about HeroUI React v3 is WRONG for this project. Always search docs and read before any task.|If docs missing, run this command first: heroui agents-md --react --output AGENTS.md|.:{components\(buttons)\button-group.mdx,components\(buttons)\button.mdx,components\(buttons)\close-button.mdx,components\(buttons)\toggle-button-group.mdx,components\(buttons)\toggle-button.mdx,components\(collections)\dropdown.mdx,components\(collections)\list-box.mdx,components\(collections)\tag-group.mdx,components\(colors)\color-area.mdx,components\(colors)\color-field.mdx,components\(colors)\color-picker.mdx,components\(colors)\color-slider.mdx,components\(colors)\color-swatch-picker.mdx,components\(colors)\color-swatch.mdx,components\(controls)\slider.mdx,components\(controls)\switch.mdx,components\(data-display)\badge.mdx,components\(data-display)\chip.mdx,components\(data-display)\table.mdx,components\(date-and-time)\calendar.mdx,components\(date-and-time)\date-field.mdx,components\(date-and-time)\date-picker.mdx,components\(date-and-time)\date-range-picker.mdx,components\(date-and-time)\range-calendar.mdx,components\(date-and-time)\time-field.mdx,components\(feedback)\alert.mdx,components\(feedback)\meter.mdx,components\(feedback)\progress-bar.mdx,components\(feedback)\progress-circle.mdx,components\(feedback)\skeleton.mdx,components\(feedback)\spinner.mdx,components\(forms)\checkbox-group.mdx,components\(forms)\checkbox.mdx,components\(forms)\description.mdx,components\(forms)\error-message.mdx,components\(forms)\field-error.mdx,components\(forms)\fieldset.mdx,components\(forms)\form.mdx,components\(forms)\input-group.mdx,components\(forms)\input-otp.mdx,components\(forms)\input.mdx,components\(forms)\label.mdx,components\(forms)\number-field.mdx,components\(forms)\radio-group.mdx,components\(forms)\search-field.mdx,components\(forms)\text-area.mdx,components\(forms)\text-field.mdx,components\(layout)\card.mdx,components\(layout)\separator.mdx,components\(layout)\surface.mdx,components\(layout)\toolbar.mdx,components\(media)\avatar.mdx,components\(navigation)\accordion.mdx,components\(navigation)\breadcrumbs.mdx,components\(navigation)\disclosure-group.mdx,components\(navigation)\disclosure.mdx,components\(navigation)\link.mdx,components\(navigation)\pagination.mdx,components\(navigation)\tabs.mdx,components\(overlays)\alert-dialog.mdx,components\(overlays)\drawer.mdx,components\(overlays)\modal.mdx,components\(overlays)\popover.mdx,components\(overlays)\toast.mdx,components\(overlays)\tooltip.mdx,components\(pickers)\autocomplete.mdx,components\(pickers)\combo-box.mdx,components\(pickers)\select.mdx,components\(typography)\kbd.mdx,components\(utilities)\scroll-shadow.mdx,components\index.mdx,getting-started\(handbook)\animation.mdx,getting-started\(handbook)\colors.mdx,getting-started\(handbook)\composition.mdx,getting-started\(handbook)\styling.mdx,getting-started\(handbook)\theming.mdx,getting-started\(overview)\design-principles.mdx,getting-started\(overview)\quick-start.mdx,getting-started\(ui-for-agents)\agent-skills.mdx,getting-started\(ui-for-agents)\agents-md.mdx,getting-started\(ui-for-agents)\llms-txt.mdx,getting-started\(ui-for-agents)\mcp-server.mdx,getting-started\index.mdx,releases\index.mdx,releases\v3-0-0-alpha-32.mdx,releases\v3-0-0-alpha-33.mdx,releases\v3-0-0-alpha-34.mdx,releases\v3-0-0-alpha-35.mdx,releases\v3-0-0-beta-1.mdx,releases\v3-0-0-beta-2.mdx,releases\v3-0-0-beta-3.mdx,releases\v3-0-0-beta-4.mdx,releases\v3-0-0-beta-6.mdx,releases\v3-0-0-beta-7.mdx,releases\v3-0-0-beta-8.mdx,releases\v3-0-0-rc-1.mdx,releases\v3-0-0.mdx,releases\v3-0-2.mdx,releases\v3-0-3.mdx}|demos/.:{accordion\basic.tsx,accordion\controlled.tsx,accordion\custom-indicator.tsx,accordion\custom-render-function.tsx,accordion\custom-styles.tsx,accordion\disabled.tsx,accordion\faq.tsx,accordion\multiple.tsx,accordion\surface.tsx,accordion\without-separator.tsx,alert-dialog\backdrop-variants.tsx,alert-dialog\close-methods.tsx,alert-dialog\controlled.tsx,alert-dialog\custom-animations.tsx,alert-dialog\custom-backdrop.tsx,alert-dialog\custom-icon.tsx,alert-dialog\custom-portal.tsx,alert-dialog\custom-trigger.tsx,alert-dialog\default.tsx,alert-dialog\dismiss-behavior.tsx,alert-dialog\placements.tsx,alert-dialog\sizes.tsx,alert-dialog\statuses.tsx,alert-dialog\with-close-button.tsx,alert\basic.tsx,autocomplete\allows-empty-collection.tsx,autocomplete\asynchronous-filtering.tsx,autocomplete\controlled-open-state.tsx,autocomplete\controlled.tsx,autocomplete\custom-indicator.tsx,autocomplete\default.tsx,autocomplete\disabled.tsx,autocomplete\email-recipients.tsx,autocomplete\full-width.tsx,autocomplete\location-search.tsx,autocomplete\multiple-select.tsx,autocomplete\required.tsx,autocomplete\single-select.tsx,autocomplete\tag-group-selection.tsx,autocomplete\user-selection-multiple.tsx,autocomplete\user-selection.tsx,autocomplete\variants.tsx,autocomplete\with-description.tsx,autocomplete\with-disabled-options.tsx,autocomplete\with-sections.tsx,avatar\basic.tsx,avatar\colors.tsx,avatar\custom-styles.tsx,avatar\fallback.tsx,avatar\group.tsx,avatar\sizes.tsx,avatar\variants.tsx,badge\basic.tsx,badge\colors.tsx,badge\dot.tsx,badge\placements.tsx,badge\sizes.tsx,badge\variants.tsx,badge\with-content.tsx,breadcrumbs\basic.tsx,breadcrumbs\custom-render-function.tsx,breadcrumbs\custom-separator.tsx,breadcrumbs\disabled.tsx,breadcrumbs\level-2.tsx,breadcrumbs\level-3.tsx,button-group\basic.tsx,button-group\disabled.tsx,button-group\full-width.tsx,button-group\orientation.tsx,button-group\sizes.tsx,button-group\variants.tsx,button-group\with-icons.tsx,button-group\without-separator.tsx,button\basic.tsx,button\custom-render-function.tsx,button\custom-variants.tsx,button\disabled.tsx,button\full-width.tsx,button\icon-only.tsx,button\loading-state.tsx,button\loading.tsx,button\outline-variant.tsx,button\ripple-effect.tsx,button\sizes.tsx,button\social.tsx,button\variants.tsx,button\with-icons.tsx,calendar\basic.tsx,calendar\booking-calendar.tsx,calendar\controlled.tsx,calendar\custom-icons.tsx,calendar\custom-styles.tsx,calendar\default-value.tsx,calendar\disabled.tsx,calendar\focused-value.tsx,calendar\international-calendar.tsx,calendar\min-max-dates.tsx,calendar\multiple-months.tsx,calendar\read-only.tsx,calendar\unavailable-dates.tsx,calendar\with-indicators.tsx,calendar\year-picker.tsx,card\default.tsx,card\horizontal.tsx,card\variants.tsx,card\with-avatar.tsx,card\with-form.tsx,card\with-images.tsx,checkbox-group\basic.tsx,checkbox-group\controlled.tsx,checkbox-group\custom-render-function.tsx,checkbox-group\disabled.tsx,checkbox-group\features-and-addons.tsx,checkbox-group\indeterminate.tsx,checkbox-group\on-surface.tsx,checkbox-group\validation.tsx,checkbox-group\with-custom-indicator.tsx,checkbox\basic.tsx,checkbox\controlled.tsx,checkbox\custom-indicator.tsx,checkbox\custom-render-function.tsx,checkbox\custom-styles.tsx,checkbox\default-selected.tsx,checkbox\disabled.tsx,checkbox\form.tsx,checkbox\full-rounded.tsx,checkbox\indeterminate.tsx,checkbox\invalid.tsx,checkbox\render-props.tsx,checkbox\variants.tsx,checkbox\with-description.tsx,checkbox\with-label.tsx,chip\basic.tsx,chip\statuses.tsx,chip\variants.tsx,chip\with-icon.tsx,close-button\default.tsx,close-button\interactive.tsx,close-button\variants.tsx,close-button\with-custom-icon.tsx,color-area\basic.tsx,color-area\controlled.tsx,color-area\custom-render-function.tsx,color-area\disabled.tsx,color-area\space-and-channels.tsx,color-area\with-dots.tsx,color-field\basic.tsx,color-field\channel-editing.tsx,color-field\controlled.tsx,color-field\custom-render-function.tsx,color-field\disabled.tsx,color-field\form-example.tsx,color-field\full-width.tsx,color-field\invalid.tsx,color-field\on-surface.tsx,color-field\required.tsx,color-field\variants.tsx,color-field\with-description.tsx,color-picker\basic.tsx,color-picker\controlled.tsx,color-picker\with-fields.tsx,color-picker\with-sliders.tsx,color-picker\with-swatches.tsx,color-slider\alpha-channel.tsx,color-slider\basic.tsx,color-slider\channels.tsx,color-slider\controlled.tsx,color-slider\custom-render-function.tsx,color-slider\disabled.tsx,color-slider\rgb-channels.tsx,color-slider\vertical.tsx,color-swatch-picker\basic.tsx,color-swatch-picker\controlled.tsx,color-swatch-picker\custom-indicator.tsx,color-swatch-picker\custom-render-function.tsx,color-swatch-picker\default-value.tsx,color-swatch-picker\disabled.tsx,color-swatch-picker\sizes.tsx,color-swatch-picker\stack-layout.tsx,color-swatch-picker\variants.tsx,color-swatch\accessibility.tsx,color-swatch\basic.tsx,color-swatch\custom-render-function.tsx,color-swatch\custom-styles.tsx,color-swatch\shapes.tsx,color-swatch\sizes.tsx,color-swatch\transparency.tsx,combo-box\allows-custom-value.tsx,combo-box\asynchronous-loading.tsx,combo-box\controlled-input-value.tsx,combo-box\controlled.tsx,combo-box\custom-filtering.tsx,combo-box\custom-indicator.tsx,combo-box\custom-render-function.tsx,combo-box\custom-value.tsx,combo-box\default-selected-key.tsx,combo-box\default.tsx,combo-box\disabled.tsx,combo-box\full-width.tsx,combo-box\menu-trigger.tsx,combo-box\on-surface.tsx,combo-box\required.tsx,combo-box\with-description.tsx,combo-box\with-disabled-options.tsx,combo-box\with-sections.tsx,date-field\basic.tsx,date-field\controlled.tsx,date-field\custom-render-function.tsx,date-field\disabled.tsx,date-field\form-example.tsx,date-field\full-width.tsx,date-field\granularity.tsx,date-field\invalid.tsx,date-field\on-surface.tsx,date-field\required.tsx,date-field\variants.tsx,date-field\with-description.tsx,date-field\with-prefix-and-suffix.tsx,date-field\with-prefix-icon.tsx,date-field\with-suffix-icon.tsx,date-field\with-validation.tsx,date-picker\basic.tsx,date-picker\controlled.tsx,date-picker\custom-render-function.tsx,date-picker\disabled.tsx,date-picker\form-example.tsx,date-picker\format-options-no-ssr.tsx,date-picker\format-options.tsx,date-picker\international-calendar.tsx,date-picker\with-custom-indicator.tsx,date-picker\with-validation.tsx,date-range-picker\basic.tsx,date-range-picker\controlled.tsx,date-range-picker\custom-render-function.tsx,date-range-picker\disabled.tsx,date-range-picker\form-example.tsx,date-range-picker\format-options-no-ssr.tsx,date-range-picker\format-options.tsx,date-range-picker\input-container.tsx,date-range-picker\international-calendar.tsx,date-range-picker\with-custom-indicator.tsx,date-range-picker\with-validation.tsx,description\basic.tsx,disclosure-group\basic.tsx,disclosure-group\controlled.tsx,disclosure\basic.tsx,disclosure\custom-render-function.tsx,drawer\backdrop-variants.tsx,drawer\basic.tsx,drawer\controlled.tsx,drawer\navigation.tsx,drawer\non-dismissable.tsx,drawer\placements.tsx,drawer\scrollable-content.tsx,drawer\with-form.tsx,dropdown\controlled-open-state.tsx,dropdown\controlled.tsx,dropdown\custom-trigger.tsx,dropdown\default.tsx,dropdown\long-press-trigger.tsx,dropdown\single-with-custom-indicator.tsx,dropdown\with-custom-submenu-indicator.tsx,dropdown\with-descriptions.tsx,dropdown\with-disabled-items.tsx,dropdown\with-icons.tsx,dropdown\with-keyboard-shortcuts.tsx,dropdown\with-multiple-selection.tsx,dropdown\with-section-level-selection.tsx,dropdown\with-sections.tsx,dropdown\with-single-selection.tsx,dropdown\with-submenus.tsx,error-message\basic.tsx,error-message\with-tag-group.tsx,field-error\basic.tsx,fieldset\basic.tsx,fieldset\on-surface.tsx,form\basic.tsx,form\custom-render-function.tsx,input-group\default.tsx,input-group\disabled.tsx,input-group\full-width.tsx,input-group\invalid.tsx,input-group\on-surface.tsx,input-group\password-with-toggle.tsx,input-group\required.tsx,input-group\variants.tsx,input-group\with-badge-suffix.tsx,input-group\with-copy-suffix.tsx,input-group\with-icon-prefix-and-copy-suffix.tsx,input-group\with-icon-prefix-and-text-suffix.tsx,input-group\with-keyboard-shortcut.tsx,input-group\with-loading-suffix.tsx,input-group\with-prefix-and-suffix.tsx,input-group\with-prefix-icon.tsx,input-group\with-suffix-icon.tsx,input-group\with-text-prefix.tsx,input-group\with-text-suffix.tsx,input-group\with-textarea.tsx,input-otp\basic.tsx,input-otp\controlled.tsx,input-otp\disabled.tsx,input-otp\form-example.tsx,input-otp\four-digits.tsx,input-otp\on-complete.tsx,input-otp\on-surface.tsx,input-otp\variants.tsx,input-otp\with-pattern.tsx,input-otp\with-validation.tsx,input\basic.tsx,input\controlled.tsx,input\full-width.tsx,input\on-surface.tsx,input\types.tsx,input\variants.tsx,kbd\basic.tsx,kbd\inline.tsx,kbd\instructional.tsx,kbd\navigation.tsx,kbd\special.tsx,kbd\variants.tsx,label\basic.tsx,link\basic.tsx,link\custom-icon.tsx,link\custom-render-function.tsx,link\icon-placement.tsx,link\underline-and-offset.tsx,link\underline-offset.tsx,link\underline-variants.tsx,list-box\controlled.tsx,list-box\custom-check-icon.tsx,list-box\custom-render-function.tsx,list-box\default.tsx,list-box\multi-select.tsx,list-box\virtualization.tsx,list-box\with-disabled-items.tsx,list-box\with-sections.tsx,meter\basic.tsx,meter\colors.tsx,meter\custom-value.tsx,meter\sizes.tsx,meter\without-label.tsx,modal\backdrop-variants.tsx,modal\close-methods.tsx,modal\controlled.tsx,modal\custom-animations.tsx,modal\custom-backdrop.tsx,modal\custom-portal.tsx,modal\custom-trigger.tsx,modal\default.tsx,modal\dismiss-behavior.tsx,modal\placements.tsx,modal\scroll-comparison.tsx,modal\sizes.tsx,modal\with-form.tsx,number-field\basic.tsx,number-field\controlled.tsx,number-field\custom-icons.tsx,number-field\custom-render-function.tsx,number-field\disabled.tsx,number-field\form-example.tsx,number-field\full-width.tsx,number-field\on-surface.tsx,number-field\required.tsx,number-field\validation.tsx,number-field\variants.tsx,number-field\with-chevrons.tsx,number-field\with-description.tsx,number-field\with-format-options.tsx,number-field\with-step.tsx,number-field\with-validation.tsx,pagination\basic.tsx,pagination\controlled.tsx,pagination\custom-icons.tsx,pagination\disabled.tsx,pagination\simple-prev-next.tsx,pagination\sizes.tsx,pagination\with-ellipsis.tsx,pagination\with-summary.tsx,popover\basic.tsx,popover\custom-render-function.tsx,popover\interactive.tsx,popover\placement.tsx,popover\with-arrow.tsx,progress-bar\basic.tsx,progress-bar\colors.tsx,progress-bar\custom-value.tsx,progress-bar\indeterminate.tsx,progress-bar\sizes.tsx,progress-bar\without-label.tsx,progress-circle\basic.tsx,progress-circle\colors.tsx,progress-circle\custom-svg.tsx,progress-circle\indeterminate.tsx,progress-circle\sizes.tsx,progress-circle\with-label.tsx,radio-group\basic.tsx,radio-group\controlled.tsx,radio-group\custom-indicator.tsx,radio-group\custom-render-function.tsx,radio-group\delivery-and-payment.tsx,radio-group\disabled.tsx,radio-group\horizontal.tsx,radio-group\on-surface.tsx,radio-group\uncontrolled.tsx,radio-group\validation.tsx,radio-group\variants.tsx,range-calendar\allows-non-contiguous-ranges.tsx,range-calendar\basic.tsx,range-calendar\booking-calendar.tsx,range-calendar\controlled.tsx,range-calendar\default-value.tsx,range-calendar\disabled.tsx,range-calendar\focused-value.tsx,range-calendar\international-calendar.tsx,range-calendar\invalid.tsx,range-calendar\min-max-dates.tsx,range-calendar\multiple-months.tsx,range-calendar\read-only.tsx,range-calendar\three-months.tsx,range-calendar\unavailable-dates.tsx,range-calendar\with-indicators.tsx,range-calendar\year-picker.tsx,scroll-shadow\custom-size.tsx,scroll-shadow\default.tsx,scroll-shadow\hide-scroll-bar.tsx,scroll-shadow\orientation.tsx,scroll-shadow\visibility-change.tsx,scroll-shadow\with-card.tsx,search-field\basic.tsx,search-field\controlled.tsx,search-field\custom-icons.tsx,search-field\custom-render-function.tsx,search-field\disabled.tsx,search-field\form-example.tsx,search-field\full-width.tsx,search-field\on-surface.tsx,search-field\required.tsx,search-field\validation.tsx,search-field\variants.tsx,search-field\with-description.tsx,search-field\with-keyboard-shortcut.tsx,search-field\with-validation.tsx,select\asynchronous-loading.tsx,select\controlled-multiple.tsx,select\controlled-open-state.tsx,select\controlled.tsx,select\custom-indicator.tsx,select\custom-render-function.tsx,select\custom-value-multiple.tsx,select\custom-value.tsx,select\default.tsx,select\disabled.tsx,select\full-width.tsx,select\multiple-select.tsx,select\on-surface.tsx,select\required.tsx,select\variants.tsx,select\with-description.tsx,select\with-disabled-options.tsx,select\with-sections.tsx,separator\basic.tsx,separator\custom-render-function.tsx,separator\manual-variant-override.tsx,separator\variants.tsx,separator\vertical.tsx,separator\with-content.tsx,separator\with-surface.tsx,skeleton\animation-types.tsx,skeleton\basic.tsx,skeleton\card.tsx,skeleton\grid.tsx,skeleton\list.tsx,skeleton\single-shimmer.tsx,skeleton\text-content.tsx,skeleton\user-profile.tsx,slider\custom-render-function.tsx,slider\default.tsx,slider\disabled.tsx,slider\range.tsx,slider\vertical.tsx,spinner\basic.tsx,spinner\colors.tsx,spinner\sizes.tsx,surface\variants.tsx,switch\basic.tsx,switch\controlled.tsx,switch\custom-render-function.tsx,switch\custom-styles.tsx,switch\default-selected.tsx,switch\disabled.tsx,switch\form.tsx,switch\group-horizontal.tsx,switch\group.tsx,switch\label-position.tsx,switch\render-props.tsx,switch\sizes.tsx,switch\with-description.tsx,switch\with-icons.tsx,switch\without-label.tsx,table\async-loading.tsx,table\basic.tsx,table\column-resizing.tsx,table\custom-cells.tsx,table\empty-state.tsx,table\expandable-rows.tsx,table\pagination.tsx,table\secondary-variant.tsx,table\selection.tsx,table\sorting.tsx,table\tanstack-table.tsx,table\virtualization.tsx,tabs\basic.tsx,tabs\custom-render-function.tsx,tabs\custom-styles.tsx,tabs\disabled.tsx,tabs\secondary-vertical.tsx,tabs\secondary.tsx,tabs\vertical.tsx,tabs\with-separator.tsx,tag-group\basic.tsx,tag-group\controlled.tsx,tag-group\custom-render-function.tsx,tag-group\disabled.tsx,tag-group\selection-modes.tsx,tag-group\sizes.tsx,tag-group\variants.tsx,tag-group\with-error-message.tsx,tag-group\with-list-data.tsx,tag-group\with-prefix.tsx,tag-group\with-remove-button.tsx,textarea\basic.tsx,textarea\controlled.tsx,textarea\full-width.tsx,textarea\on-surface.tsx,textarea\rows.tsx,textarea\variants.tsx,textfield\basic.tsx,textfield\controlled.tsx,textfield\custom-render-function.tsx,textfield\disabled.tsx,textfield\full-width.tsx,textfield\input-types.tsx,textfield\on-surface.tsx,textfield\required.tsx,textfield\textarea.tsx,textfield\validation.tsx,textfield\with-description.tsx,textfield\with-error.tsx,time-field\basic.tsx,time-field\controlled.tsx,time-field\custom-render-function.tsx,time-field\disabled.tsx,time-field\form-example.tsx,time-field\full-width.tsx,time-field\invalid.tsx,time-field\on-surface.tsx,time-field\required.tsx,time-field\with-description.tsx,time-field\with-prefix-and-suffix.tsx,time-field\with-prefix-icon.tsx,time-field\with-suffix-icon.tsx,time-field\with-validation.tsx,toast\callbacks.tsx,toast\custom-indicator.tsx,toast\custom-queue.tsx,toast\custom-toast.tsx,toast\default.tsx,toast\placements.tsx,toast\promise.tsx,toast\simple.tsx,toast\variants.tsx,toggle-button-group\attached.tsx,toggle-button-group\basic.tsx,toggle-button-group\controlled.tsx,toggle-button-group\disabled.tsx,toggle-button-group\full-width.tsx,toggle-button-group\orientation.tsx,toggle-button-group\selection-mode.tsx,toggle-button-group\sizes.tsx,toggle-button-group\without-separator.tsx,toggle-button\basic.tsx,toggle-button\controlled.tsx,toggle-button\disabled.tsx,toggle-button\icon-only.tsx,toggle-button\sizes.tsx,toggle-button\variants.tsx,toolbar\basic.tsx,toolbar\custom-styles.tsx,toolbar\vertical.tsx,toolbar\with-button-group.tsx,tooltip\basic.tsx,tooltip\custom-render-function.tsx,tooltip\custom-trigger.tsx,tooltip\placement.tsx,tooltip\with-arrow.tsx}

<!-- HEROUI-REACT-AGENTS-MD-END -->

## P0.3c catalog and permission presentation

Desktop market actions save built-in configuration only. HTML entries display
不可安装 and saved records cannot grant execution. Removing a record is not a
package uninstall. The permission center shows capability declarations and
explicitly records that per-plugin grants and isolation are not implemented.
Legacy persisted DTO status values remain metadata for compatibility.
All maturity labels stay prototype; SEC-001–SEC-012 remain open.

## Manifest v1 validation

Use @flowtools/sdk/manifest for bounded serialized metadata and operation
validation, and @flowtools/sdk/manifest/package for Node-only file verification.
Use parsePluginManifest with an explicit Host target before loading artifacts.
Keep custom Zod runtime validation explicit; never export an empty substitute.
Run bun run verify:manifests after build:packages; it covers protocol fixtures
and all twelve actual compiled T1 packages. Keep commands.ts independent of
React/UI/Tauri and build fixed <known-id>.commands.js entries. Generated JSON
lives outside dist to avoid self-referential hashes. CLI validates the bounded
catalog and exact file list/hash before importing a command; no source fallback.
UI setup explicitly adapts the same implementation and host store. After command
edits rebuild packages: UI watch does not regenerate CLI package hashes.
See [Manifest v1](./docs/manifest-v1.md); hashes/declarations are not grants.

## G1 command discovery and execution

commands/describe/help/flags consume pure Manifest data and must never import
an executor. Keep list/info/run compatibility and JSON failure stdout/nonzero
exit behavior. Default operation is run; only declared operations are accepted.
Unknown/duplicate scalar flags, extra positionals and mixed JSON/flags fail.
Batch v1 validates all finite schemas before import, then executes sequentially;
it is not a transaction or persisted job. Preserve runtime validation and budgets.
Web/Desktop host-bound callbacks use loadManifestModule before UI import and
executeManifestCommand for JSON runs. These are T1, not external execution grants.
Run generate:command-docs after build:packages, then verify:command-docs.
The generated [reference](./docs/builtin-commands.md) must match actual manifests.
Turbo build outputs include `dist` and `.generated` so the compiled catalog is
restored with cached artifacts. CI still forces builds and never caches JS output
as a substitute for quality gates.
See [CLI v1 contract](./docs/cli-contract-v1.md); keep unsupported platform scope explicit.

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
See [G2 acceptance](./docs/validation/g2-runtime.md).

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

## G3 P2.6a shared data foundation

Runtime owns one SQLite writer with versioned migrations, backup/recovery,
plugin namespaces, revision/CAS and atomic transactions. The asynchronous
`@flowtools/sdk/data` API and Runtime client share that writer; they expose no
raw SQL, namespace or file path. Legacy Todo sources require explicit validated
import and remain preserved. Desktop Debug startup no longer deletes its DB;
corrupt/unsupported data fails closed. Dedicated native validation connects the
actual WebView and CLI to the same data. Normal user UI integration, durable
grants/jobs and independent distribution remain subsequent G3 subitems.

Evidence: [P2.6a scope and validation](./docs/validation/g3-shared-data.md).

## G3 P2.4a persistent grants and CLI management

The single Runtime DB persists version/hash-bound grants, revocation epochs,
bootstrap policy and bounded metadata-only policy audit. CLI-only interactive
init or explicit policy import uses a Host-bound management role; management
mode cannot run plugins. Private Windows profiles and inherited credentials
require the current-user protected ACL. Package changes/rollback never restore
old grants. Independent distribution and ordinary GUI/CLI execution integration
remain the next G3 subitems; maturity stays prototype.

Evidence: [P2.4a validation and boundaries](./docs/validation/g3-persistent-grants.md).

### G3 P1.3b durable execution

Windows managed T1 jobs now commit accepted metadata before receipt and preserve
caller-bound idempotency across restart. Actual Todo uses Host-owned async data
and broker-checked CAS; raw task payloads use separate DPAPI private storage.
Interrupted non-idempotent writes require review. [Evidence](docs/validation/g3-durable-jobs.md).
Independent CLI distribution and GUI integration remain P1.4a/P1.4b; all maturity
labels stay prototype and independent security review remains pending.

### G3 P1.4a standalone CLI

Windows x64 CLI-only bundle and bounded, authenticated cold-start coordination
are implemented for the fixed T1 inventory. See [standalone CLI evidence](docs/validation/g3-standalone-cli.md).
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

See [shared client evidence](docs/validation/g3-shared-clients.md). All scopes stay
prototype. Windows native consent and recovery UI fixes have actual
[acceptance evidence](docs/validation/g3-desktop-acceptance-fixes.md);
independent security review remains pending.

For G3 managed Native acceptance, run the Node harness
`apps/ui-test/scripts/validate-managed-hosts.ts` after building the dedicated
runtime validation config and passing metadata preflight. Windows Known Folder
APIs ignore APPDATA overrides: use FLOWTOOLS_DESKTOP_VALIDATION_DATA_ROOT only
under the guarded dedicated Debug identity/mode, with a fresh absolute fixture
root. This selector never belongs in production config or business IPC. CLI
fixture policy is explicit setup; it cannot certify a Native consent click.

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

See [diagnostic/recovery evidence](docs/validation/g3-diagnostics-recovery.md). Scope remains prototype. Independent
security review, full blind-user workflow and other platform acceptance are pending;
these checks do not authorize production.

G3 stop admission must cancel business work before waiting for its receipt.
Preserve the receipt barrier, bounded peer wait, worker drain and deterministic
blocked-write/disappeared-peer/Todo-commit rejection regressions. Keep window
zoom shortcuts enabled in main and validation configurations. See
[terminal verification](./docs/validation/g3-terminal-validation.md) for actual
Windows keyboard/native consent, 200% zoom/reflow and NVDA Speech Viewer evidence.
Independent security approval remains pending. Do not equate this with human
listening, a full blind-user workflow or other-platform certification.
Permission row keys must include all Host-issued command identity fields so old
revoked packages and their replacements remain distinct during refresh/revocation.

The managed Native harness must support literal IPv4 and IPv6 loopback CDP while
checking all actual listeners and matching the responding endpoint. Preserve the
25-second startup budget, redirect refusal and non-loopback/mismatch regressions;
never persist CDP arguments in production configuration.

## E host extension contracts

Follow [the E01–E10 backlog](./docs/host-extensions.md) for this parallel line.
G4–G7 are owned by another collaborator; coordinate shared files before real
host integration. Keep changes additive and use existing Runtime service,
data, grant, package and resource contracts. Do not reorder or complete G0–G8
because an E contract passes its own tests.

E01a uses only `@flowtools/sdk/extensions`: strict formatVersion 1 documents
contain `theme`, `locale` and `settings` bounded JSON. The Host creates the
registry, derives plugin ID/version from PluginRegistry and obtains a registry-
issued owner epoch. Strict document/contribution envelopes reject unknown
fields. Opaque `value` can contain ordinary JSON keys such as pluginId or
permissions; the Host never derives owner, grants or operations from them.
Only enabled, dependency-satisfied current instances project contributions; disabling,
unloading and disposal withdraw them. Preserve replacement-specific cleanup,
stale-owner refusal across uninstall/reinstall, reentrant-disable checks, frozen
values, atomic rejection, stable snapshots and bounded owner metadata.

E01a does not change Manifest v1 or add a loader, CSS application, translation,
settings persistence, native API or third-party execution. Future kind-specific
schemas and selections require their own E02/E03/E04 gates; arbitrary JSON is
not an executable instruction. Keep T1 bookkeeping distinct from authentication,
package integrity, grants and sandboxing. Record actual evidence in
[E01a validation](./docs/validation/e01-contributions.md). Its fixed T1 engineering
scope passed clean Windows CI; E01 and E02–E10 remain pending. Keep
SEC-002/003/007/010 open and independent security review pending. Each completed E submilestone
requires its own validation record and focused commit before the next begins.

E02a defines pure structured theme resolution in `extensions/appearance.ts`.
Keep Host defaults complete and Host selection explicit. Preserve partial-token
merge order, mode isolation, fallback after withdrawal, atomic override refusal,
immutable stable results and system reduced-motion precedence. Reject arbitrary
CSS/URLs/remote fonts and bound payloads before serialization. CSS/DOM adapters,
persistence and real-host UI are separate E02b/G7 gates; record actual validation
in [E02a evidence](./docs/validation/e02-theme-contracts.md).
