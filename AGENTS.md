# Repository Guidelines

## Project Structure & Module Organization

This repository is a Bun + Turbo monorepo.

- `packages/ui`: shared React UI primitives (`src/button.tsx`, `src/card.tsx`).
- `packages/sdk`: plugin-facing SDK surface for Flow Tool capabilities.
- `apps/desktop`, `apps/web`, `apps/docs`: app targets (currently scaffolded).
- `plugins/`: local plugin workspace (currently scaffolded).
- `docs/` and root docs like `README.md`, `architecture.md`: product and architecture references.

Keep reusable logic in `packages/*`; keep host-specific behavior in `apps/*`.

## References

You need to reference the following repositories:

- [Architecture](./architecture.md)
- [README](./README.md)

## Build, Test, and Development Commands

Run from repository root:

- `bun run dev`: starts all workspace `dev` tasks via Turbo.
- `bun run build`: builds all workspaces (`turbo run build`).
- `bun run lint`: runs workspace lint tasks.
- `bun run check-types`: runs TypeScript checks (`tsc --noEmit` in packages).
- `bun run format`: formats `*.ts`, `*.tsx`, `*.md` files.

If dependencies change, run `bun install`.

## Coding Style & Naming Conventions

- TypeScript strict mode is enabled; keep code type-safe and avoid `any`.
- Formatting is enforced by `oxfmt` + Prettier conventions: 2 spaces, single quotes, no semicolons, max width 80.
- Linting is via `oxlint`; fix warnings before opening a PR.
- Use lowercase filenames for modules (for example `button.tsx`) and PascalCase for exported React components (for example `Button`).
- Keep plugin IDs and command IDs kebab-case (for example `hash-generator`).

## Testing Guidelines

There is no wired test runner yet. Current minimum quality gate is:

1. `bun run lint`
2. `bun run check-types`
3. Manual validation of changed flows

When adding tests, prefer colocated `*.test.ts`/`*.test.tsx` files and register the test task in `turbo.json`.

## Commit & Pull Request Guidelines

The `main` branch currently has no commit history; use Conventional Commits from now on:

- `feat(ui): add plugin card variants`
- `fix(sdk): guard missing capability`

PRs should include:

- clear summary and scope
- linked issue (if applicable)
- screenshots/GIFs for UI changes
- notes on plugin/runtime impact and validation steps
