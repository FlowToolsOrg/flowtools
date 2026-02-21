# Repository Guidelines

## Project Structure and Module Organization

This repository is a Bun + Turbo monorepo.

- `packages/ui`: shared React UI primitives (`src/button.tsx`, `src/card.tsx`).
- `packages/sdk`: plugin-facing SDK contract, hooks, runtime provider, and
  result helpers.
- `apps/web-vite`: current runnable host prototype (web runtime + routing).
- `apps/ui-test`: consumer app for manual/UI testing of `@flow-tool/ui`.
- `plugins/`: local plugin workspace (currently includes
  `plugin-example-hello-world`).
- `configs/tsdown`: shared package build config.
- `docs/` and root docs such as `README.md` and `architecture.md`: product and
  architecture references.

Planned but not yet present in this repo:

- `apps/desktop` (desktop-first Tauri host)
- `apps/docs`

Keep reusable logic in `packages/*`; keep host-specific behavior in `apps/*`.

## References

You should reference the following files when changing architecture or developer
workflows:

- [Architecture](./architecture.md)
- [README](./README.md)

## Build, Test, and Development Commands

Run from repository root:

- `bun run dev`: starts workspace `dev` tasks via Turbo.
- `bun run build`: builds workspaces (`turbo run build`).
- `bun run lint`: runs workspace lint tasks.
- `bun run check-types`: runs workspace type checks.
- `bun run test`: runs workspace tests (currently used by `apps/ui-test`).
- `bun run format`: formats tracked source/document files.

Useful app-level commands:

- `cd apps/web-vite && bun run dev`
- `cd apps/ui-test && bun run dev`
- `cd apps/ui-test && bun run test`

If dependencies change, run `bun install`.

## Coding Style and Naming Conventions

- TypeScript strict mode is enabled; keep code type-safe and avoid `any`.
- Formatting is enforced by `oxfmt` + Prettier conventions: 2 spaces,
  single quotes, no semicolons, max width 80.
- Linting is via `oxlint`; fix warnings before opening a PR.
- Use lowercase filenames for modules (for example `button.tsx`) and
  PascalCase for exported React components (for example `Button`).
- Keep plugin IDs and command IDs kebab-case (for example `hash-generator`).

## Testing Guidelines

Current quality gate:

1. `bun run lint`
2. `bun run check-types`
3. `bun run test`
4. Manual validation for changed flows (for web host, verify routes and plugin
   rendering in `apps/web-vite`)

When adding tests, prefer colocated `*.test.ts` / `*.test.tsx` files and
register the test task in `turbo.json`.

## Documentation Sync (Required)

When code includes major refactoring or important new features, update these
files in the same change:

- `README.md`
- `AGENTS.md`
- `architecture.md`

Do not merge architecture/runtime changes with stale docs.

## Commit and Pull Request Guidelines

Use Conventional Commits, for example:

- `feat(ui): add plugin card variants`
- `fix(sdk): guard missing capability`

PRs should include:

- clear summary and scope
- linked issue (if applicable)
- screenshots/GIFs for UI changes
- notes on plugin/runtime impact and validation steps
- documentation sync notes when architecture/runtime behavior changes
