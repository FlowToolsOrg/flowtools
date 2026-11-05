import type { ToolContext } from '../src/types/ctx'

import { expect, test } from 'bun:test'
import { createHash } from 'node:crypto'

import {
  parsePluginManifest,
  parseVersionedPluginManifest,
  exportManifestValidationContract,
  exportManifestV2ValidationContract,
  executeManifestCommand,
  type PluginManifestV2,
} from '../src/manifest'

import { manifestFixture, manifestTarget } from './fixtures/manifest-v1'

export function manifestV2Fixture() {
  const base = manifestFixture()
  return {
    ...base,
    formatVersion: 2,
    capabilityProfile: 'flowtools-native-v2',
    commands: base.commands.map(command => ({
      ...command,
      capabilityProfile: 'flowtools-native-v2',
      effects: ['file-read'],
      permissions: [
        {
          capability: 'fs',
          operations: ['read'],
          scopes: [{ kind: 'resource', slot: 'source' }],
        },
      ],
    })),
  } as unknown as PluginManifestV2
}

test('versioned parsing is explicit and the original v1 parser remains closed', () => {
  const v1 = manifestFixture()
  expect(parseVersionedPluginManifest(v1, manifestTarget)).toEqual(
    parsePluginManifest(v1, manifestTarget)
  )
  const v2 = manifestV2Fixture()
  expect(parseVersionedPluginManifest(v2, manifestTarget)).toEqual(v2)
  expect(() => parsePluginManifest(v2, manifestTarget)).toThrow()
  expect(() =>
    parseVersionedPluginManifest({ ...v1, formatVersion: 2 })
  ).toThrow()
  expect(() =>
    parseVersionedPluginManifest({ ...v2, formatVersion: 3 })
  ).toThrow()
  expect(() =>
    parseVersionedPluginManifest({ ...v2, capabilityProfile: 'other' })
  ).toThrow()
  expect(() =>
    parseVersionedPluginManifest(v2, {
      ...manifestTarget,
      sdkVersion: '999.0.0',
    })
  ).toThrow()
})

test('v2 refuses legacy scopes, unknown nested fields and mismatched operations/effects', () => {
  const base = manifestV2Fixture()
  const variants: unknown[] = [
    {
      ...base,
      commands: [{ ...base.commands[0], capabilityProfile: undefined }],
    },
    { ...base, commands: [{ ...base.commands[0], effects: [] }] },
    {
      ...base,
      commands: [{ ...base.commands[0], effects: ['native-invoke'] }],
    },
  ]
  for (const permissions of [
    [{ capability: 'fs', operations: ['read'], scopes: ['handle:source'] }],
    [
      {
        capability: 'fs',
        operations: ['read'],
        scopes: [{ kind: 'resource', slot: 'source', path: 'C:/secret' }],
      },
    ],
    [
      {
        capability: 'fs',
        operations: ['execute'],
        scopes: [{ kind: 'resource', slot: 'source' }],
      },
    ],
    [
      {
        capability: 'fs',
        operations: ['read'],
        scopes: [
          { kind: 'resource', slot: 'source' },
          { slot: 'source', kind: 'resource' },
        ],
      },
    ],
    [{ capability: 'native', operations: ['invoke'], scopes: [] }],
  ])
    variants.push({ ...base, commands: [{ ...base.commands[0], permissions }] })
  for (const value of variants)
    expect(() => parseVersionedPluginManifest(value)).toThrow()
})

test('v2 structural export stays separate from the immutable v1 semantics', () => {
  expect(
    createHash('sha256')
      .update(JSON.stringify(exportManifestValidationContract()) + '\n')
      .digest('hex')
  ).toBe('1146cf6c8eb2e4517bdddb754ca53a57562c809a8f3e597adcfc5abaef70a333')
  expect(exportManifestValidationContract().semantics).toBe(
    'flowtools-manifest-v1'
  )
  const contract = exportManifestV2ValidationContract()
  expect(contract.formatVersion).toBe(2)
  expect(contract.semantics).toBe('flowtools-manifest-v2')
  expect(contract.capabilityProfile).toBe('flowtools-native-v2')
  expect(JSON.stringify(contract)).not.toContain('"native"')
})

test('v2 pure execution works and unavailable effects refuse before calling plugin code', async () => {
  const manifest = manifestV2Fixture()
  let calls = 0
  const plugin = {
    meta: { id: manifest.id, version: manifest.version },
    run: () => {
      calls++
      return 'ok'
    },
  }
  const ctx: ToolContext = {
    env: {
      pluginId: manifest.id,
      pluginType: 'tool',
      platform: 'unknown',
      mode: 'test',
    },
    ui: { toast() {}, openPanel() {}, closePanel() {} },
    signal: new AbortController().signal,
    log() {},
    utils: { now: Date.now },
  }
  const denied = await executeManifestCommand(
    manifest,
    manifest.commands[0]!.id,
    plugin,
    { text: 'x' },
    ctx,
    manifestTarget
  )
  expect(denied.success).toBe(false)
  expect(calls).toBe(0)
  manifest.commands[0]!.effects = []
  manifest.commands[0]!.permissions = []
  const pure = await executeManifestCommand(
    manifest,
    manifest.commands[0]!.id,
    plugin,
    { text: 'x' },
    ctx,
    manifestTarget
  )
  expect(pure.success).toBe(true)
  expect(calls).toBe(1)
})
