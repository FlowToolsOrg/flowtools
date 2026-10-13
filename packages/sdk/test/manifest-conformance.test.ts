import { expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'

import { z } from 'zod'

import { parsePluginManifest } from '../src/manifest'

const valueSchema = z.strictObject({
  id: z.string().min(1),
  manifest: z.unknown().optional(),
  rawManifest: z.string().optional(),
  target: z
    .strictObject({
      hostVersion: z.string(),
      sdkVersion: z.string(),
      platform: z.enum(['windows', 'macos', 'linux', 'web']),
      arch: z.enum(['x64', 'arm64', 'wasm32']),
    })
    .optional(),
  accepted: z.boolean(),
  normalized: z.unknown().optional(),
})
const corpus = z
  .strictObject({
    formatVersion: z.literal(1),
    cases: z.array(valueSchema).min(1),
  })
  .parse(
    JSON.parse(
      readFileSync(
        new URL(
          '../../runtime-core/manifest-conformance.json',
          import.meta.url
        ),
        'utf8'
      )
    )
  )

test('shared SDK Manifest corpus is complete JSON data with unique cases', () => {
  const ids = corpus.cases.map(item => item.id)
  expect(new Set(ids).size).toBe(ids.length)
  expect(ids).toContain('valid-base')
  expect(ids).toContain('valid-default-maturity')
  for (const item of corpus.cases) {
    expect(
      Object.hasOwn(item, 'manifest') !== Object.hasOwn(item, 'rawManifest')
    ).toBe(true)
    expect(Object.hasOwn(item, 'normalized')).toBe(item.accepted)
  }
  const fallback = corpus.cases.find(
    item => item.id === 'valid-default-maturity'
  )!
  expect(fallback.normalized).toMatchObject({ maturity: 'prototype' })
  expect(fallback.manifest).not.toHaveProperty('maturity')
  expect(
    corpus.cases.find(item => item.id === 'valid-base')!.normalized
  ).not.toHaveProperty('services')
})

for (const item of corpus.cases)
  test(`SDK source matches shared Manifest conformance: ${item.id}`, () => {
    const value: unknown =
      item.rawManifest === undefined
        ? item.manifest
        : JSON.parse(item.rawManifest)
    if (item.accepted)
      // The corpus carries normalized JSON; serialization canonicalizes -0 to 0.
      expect<unknown>(
        JSON.parse(JSON.stringify(parsePluginManifest(value, item.target)))
      ).toEqual(item.normalized)
    else expect(() => parsePluginManifest(value, item.target)).toThrow()
  })
