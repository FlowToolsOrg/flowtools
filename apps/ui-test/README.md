# @flowtools/ui-test

A Vite consumer app for browser regression tests and manual validation of
`@flowtools/ui`. Tests run in headless Chromium with the workspace's pinned
Playwright version; manual visual and accessibility checks remain required.

## Commands

- `bun run dev`: run the local validation app
- `bun run build`: verify the validation app builds
- `bun run test:install-browser`: install the pinned Chromium runtime once
- `bun run test`: run all browser regression tests
- `bun run test:watch`: rerun browser tests during development
- `bun run test:browser`: run the same complete browser suite explicitly

## Notes

- `@flowtools/ui` is aliased to `../../packages/ui/src/index.ts` in
  `vite.config.ts`, so manual validation runs against source code directly.
