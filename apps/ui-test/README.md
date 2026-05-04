# @flowtools/ui-test

A Vite + Vitest consumer app for validating `@flowtools/ui`.

## Commands

- `bun run dev`: run the local testbed app
- `bun run test`: run Vitest once
- `bun run test:watch`: run Vitest in watch mode

## Notes

- `@flowtools/ui` is aliased to `../../packages/ui/src/index.ts` in
  `vite.config.ts`, so tests run against source code directly.
