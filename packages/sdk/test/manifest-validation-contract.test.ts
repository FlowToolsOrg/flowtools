import { expect, test } from 'bun:test'

import { z } from 'zod'

import {
  exportManifestValidationContract,
  parsePluginManifest,
} from '../src/manifest'
import { exportCheckedSchema } from '../src/manifest/validation-export'

import fixtures from './fixtures/manifest-admission-v1.json'

test('exported shape retains all explicit semantic checks and rejects unknown logic', () => {
  const contract = exportManifestValidationContract()
  const serialized = JSON.stringify(contract)
  for (const check of [
    'manifest-v1',
    'command-v1',
    'operation-v1',
    'dependencies-v1',
    'version',
    'range',
    'path',
    'unique',
    'bounded-json',
  ])
    expect(serialized).toContain(`"x-flowtools-check":"${check}"`)
  expect(exportManifestValidationContract()).toEqual(contract)
  for (const schema of [
    z.string().refine(() => true),
    z.date(),
    z.string().transform(value => value),
  ])
    expect(() => exportCheckedSchema(schema, new Map())).toThrow('Unmapped')
  const stacked = z
    .string()
    .refine(() => true)
    .refine(() => true)
  expect(() =>
    exportCheckedSchema(stacked, new Map([[stacked, 'known']]))
  ).toThrow('Unmapped')
})

for (const item of fixtures.cases)
  test(`native shared Manifest case: ${item.name}`, () => {
    const value = structuredClone(fixtures.base) as unknown as Record<
      string,
      unknown
    >
    for (const patch of item.patches) {
      const keys = patch.path.slice(1).split('/')
      let parent = value
      for (const key of keys.slice(0, -1))
        parent = parent[key] as Record<string, unknown>
      const key = keys.at(-1)!
      if ('remove' in patch) delete parent[key]
      else parent[key] = patch.value
    }
    const target = {
      ...fixtures.target,
      ...('target' in item ? item.target : {}),
    }
    const parse = () =>
      parsePluginManifest(value, {
        ...target,
        platform: target.platform as 'windows',
        arch: target.arch as 'x64',
      })
    if (item.accept) expect(parse()).toBeDefined()
    else expect(parse).toThrow()
  })
