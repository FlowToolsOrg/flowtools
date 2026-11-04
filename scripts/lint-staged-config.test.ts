import { expect, test } from 'bun:test'
import { resolve } from 'node:path'

import { stagedCommands } from './lint-staged.config.mjs'
const root = resolve(import.meta.dir, '..')
test('an all-ignored signed/generated staged chunk schedules no formatter', () => {
  expect(
    stagedCommands(
      [
        'packages/runtime-core/assets/builtin-media/manifest.json',
        'packages/runtime-core/assets/builtin-media/package.dsse.json',
        'packages/runtime-client/src/wire-schema.json',
      ].map(p => resolve(root, p)),
      false
    )
  ).toEqual([])
  expect(
    stagedCommands([resolve(root, 'apps/desktop/src/utils/bindings.ts')], true)
  ).toEqual([])
})
test('ordinary staged source and docs keep their actual lint/format commands', () => {
  const source = resolve(root, 'apps/desktop/src/settings/media-settings.tsx')
  expect(
    stagedCommands(
      [source, resolve(root, 'packages/runtime-client/src/bindings.ts')],
      true
    )
  ).toEqual([
    `oxlint --fix "${source.replaceAll('\\', '/')}"`,
    `oxfmt "${source.replaceAll('\\', '/')}"`,
  ])
  const doc = resolve(root, 'docs/validation/with spaces.md')
  expect(
    stagedCommands(
      [
        doc,
        resolve(
          root,
          'packages/runtime-core/assets/builtin-media/root.dsse.json'
        ),
      ],
      false
    )
  ).toEqual([`oxfmt "${doc.replaceAll('\\', '/')}"`])
})
