# @flowtools/ui-test

A Vite consumer app for manual validation of `@flowtools/ui`.

Automated tests are deprecated for this project. The `test`, `test:watch`, and
`test:browser` scripts are compatibility no-ops that print a deprecation notice.

## Commands

- `bun run dev`: run the local validation app
- `bun run build`: verify the validation app builds

## Notes

- `@flowtools/ui` is aliased to `../../packages/ui/src/index.ts` in
  `vite.config.ts`, so manual validation runs against source code directly.
