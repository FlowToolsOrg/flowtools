# UI Components Blueprint

## Goal

Build reusable UI wrappers in `packages/ui` with HeroUI + Tailwind CSS for Flow Tool host pages.

## Naming Convention

- Use lowercase and kebab-case for all folders/files.
- Keep reusable layout and data-display components inside `packages/ui/src/components`.

## Component Groups

### 1. Layout Group

- `tool-layout`
  - Responsibility: page shell, spacing system, two-column responsive layout.
  - Subcomponents:
    - `tool-layout`: root wrapper
    - `tool-layout-main`: main content container
    - `tool-layout-sidebar`: right panel container

### 2. Hero Group

- `hero-section`
  - Responsibility: page intro banner with title, description, quick stats, and actions.
  - Data model:
    - `hero-stat` (`id`, `label`, `value`)
  - Slots:
    - heading and summary
    - status chips
    - action area

### 3. Tool Discovery Group

- `tool-list`
  - Responsibility: display plugin/tool entries with status, tags, version, and install state.
  - Data model:
    - `tool-list-item` (`id`, `name`, `description`, `status`, `version`, `tags`, `isInstalled`, `isPinned`)
  - Behaviors:
    - selectable list
    - item action callback
    - empty state rendering

### 4. Settings Group

- `settings`
  - Responsibility: structured settings panel with switch/select controls.
  - Data model:
    - `settings-section`
    - `settings-item-switch`
    - `settings-item-select`
    - `settings-select-option`
  - Behaviors:
    - grouped rendering
    - controlled values via callbacks

## Public Exports

Expose from `packages/ui/src/index.ts`:

- `ToolLayout`
- `ToolLayoutMain`
- `ToolLayoutSidebar`
- `HeroSection`
- `ToolList`
- `Settings`
- related public types for tool list and settings

## Validation Plan

1. Update `apps/ui-test` demo app to exercise new components.
2. Add/adjust browser tests to assert key UI rendering and interactions.
3. Run:
   - `bun run check-types`
   - `bun run test` (or `cd apps/ui-test && bun run test`)

## Commit Plan

1. Commit A: add blueprint + new components in `packages/ui`.
2. Commit B: update `apps/ui-test` demo/tests and pass validation.
