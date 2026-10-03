/// <reference types="bun-types" />

import type { ToolContext } from '@flowtools/sdk'

import { describe, expect, test } from 'bun:test'
import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { loadPlugin, runPlugin, scanPlugins } from '@flowtools/cli'
import { PLUGIN_MARKER } from '@flowtools/sdk'

import base64Plugin from '../plugin-base64-encoder/index.tsx'
import colorPlugin from '../plugin-color-converter/index.tsx'
import { getPluginEntries } from '../plugin-entries.ts'
import hashPlugin from '../plugin-hash-generator/index.tsx'
import imageBase64Plugin from '../plugin-image-base64/index.tsx'
import jsonPlugin from '../plugin-json-formatter/index.tsx'
import randomPickerPlugin from '../plugin-random-picker/index.tsx'
import regexPlugin from '../plugin-regex-tester/index.tsx'
import textOpsPlugin from '../plugin-text-ops/index.tsx'
import timestampPlugin from '../plugin-timestamp-converter/index.tsx'
import todoPlugin from '../plugin-todo-list/index.tsx'
import uuidPlugin from '../plugin-uuid-generator/index.tsx'
import websiteLatencyPlugin from '../plugin-website-latency/index.tsx'

const builtInPlugins = [
  base64Plugin,
  colorPlugin,
  hashPlugin,
  imageBase64Plugin,
  jsonPlugin,
  randomPickerPlugin,
  regexPlugin,
  textOpsPlugin,
  timestampPlugin,
  todoPlugin,
  uuidPlugin,
  websiteLatencyPlugin,
]

const pluginsDir = fileURLToPath(new URL('..', import.meta.url))
const pluginEntryIds = Object.keys(getPluginEntries(pluginsDir)).sort()

const fakeContext: ToolContext = {
  env: {
    pluginId: 'contract-test',
    pluginType: 'app',
    platform: 'unknown',
    mode: 'test',
  },
  ui: {
    toast: () => {},
    openPanel: () => {},
    closePanel: () => {},
  },
  signal: new AbortController().signal,
  log: () => {},
  utils: { now: () => 0 },
}

function isSuccessfulParse(value: unknown): value is {
  success: true
  data: Record<string, unknown>
} {
  if (typeof value !== 'object' || value === null) return false

  const candidate = value as { success?: unknown; data?: unknown }
  return (
    candidate.success === true &&
    typeof candidate.data === 'object' &&
    candidate.data !== null &&
    !Array.isArray(candidate.data)
  )
}

describe('built-in plugin contracts', () => {
  test('matches every tsdown-discovered plugin entry exactly once', () => {
    const ids: string[] = builtInPlugins.map(plugin => plugin.meta.id).sort()

    expect(ids).toEqual(pluginEntryIds)
    expect(new Set(ids).size).toBe(ids.length)
    for (const id of ids) {
      expect(id).toMatch(/^[a-z0-9]+(?:-[a-z0-9]+)*$/)
    }
  })

  test('provides stable metadata and definePlugin markers', () => {
    for (const plugin of builtInPlugins) {
      expect(['app', 'tool']).toContain(plugin.type)
      expect(plugin.meta.id).toStartWith('plugin-')
      expect(plugin.meta.name.trim().length).toBeGreaterThan(0)
      expect(plugin.meta.version).toMatch(/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/)
      expect(plugin[PLUGIN_MARKER]).toBe(true)
    }
  })

  test('keeps run and inputSchema consistently CLI-compatible', () => {
    for (const plugin of builtInPlugins) {
      const hasRun = typeof plugin.run === 'function'
      const hasInputSchema = plugin.inputSchema !== undefined

      expect(hasRun).toBe(hasInputSchema)
      expect(hasRun).toBe(true)
      expect(typeof plugin.inputSchema?.safeParse).toBe('function')
    }
  })

  test('declares each permission at most once', () => {
    for (const plugin of builtInPlugins) {
      const permissions = plugin.meta.permissions ?? []
      expect(new Set(permissions).size).toBe(permissions.length)
    }
  })
})

describe('CLI integration contract', () => {
  test('imports every built entry through its public package subpath', async () => {
    for (const pluginId of pluginEntryIds) {
      const { default: plugin } = (await import(
        `@flowtools/plugins/${pluginId}`
      )) as { default: (typeof builtInPlugins)[number] }

      const loadedId: string = plugin.meta.id
      expect(loadedId).toBe(pluginId)
      expect(plugin[PLUGIN_MARKER]).toBe(true)
      expect(typeof plugin.run).toBe('function')
    }
  })

  test('discovers, loads, parses, and runs a built plugin', async () => {
    const pluginId = base64Plugin.meta.id
    const distPath = join(pluginsDir, 'dist', `${pluginId}.js`)

    expect(existsSync(distPath)).toBe(true)

    const discovered = scanPlugins()
    expect(discovered.map(plugin => plugin.id).sort()).toEqual(pluginEntryIds)
    expect(discovered.find(plugin => plugin.id === pluginId)).toMatchObject({
      id: pluginId,
      hasRun: true,
      hasSchema: true,
    })

    const loaded = await loadPlugin(pluginId)
    const { default: builtPlugin } =
      await import('@flowtools/plugins/plugin-base64-encoder')
    expect(Object.is(loaded, builtPlugin)).toBe(true)
    expect(loaded?.meta.id).toBe(pluginId)

    const parsed = loaded?.inputSchema?.safeParse?.({
      text: 'hello',
      mode: 'encode',
    })
    expect(parsed).toMatchObject({ success: true })
    if (!isSuccessfulParse(parsed)) {
      throw new Error('Expected the loaded plugin schema to parse CLI input')
    }

    expect(
      await runPlugin(pluginId, parsed.data, { timeout: 1_000 })
    ).toMatchObject({
      success: true,
      pluginId,
      pluginVersion: loaded?.meta.version,
      data: {
        type: 'json',
        value: { result: 'aGVsbG8=', mode: 'encode', input: 'hello' },
      },
    })
  })
})

describe('deterministic plugin run contracts', () => {
  test('base64 encoder handles success and empty input', async () => {
    expect(
      await base64Plugin.run(fakeContext, { text: 'hello', mode: 'encode' })
    ).toEqual({
      type: 'json',
      value: { result: 'aGVsbG8=', mode: 'encode', input: 'hello' },
    })
    expect(
      await base64Plugin.run(fakeContext, { text: '', mode: 'encode' })
    ).toEqual({ type: 'text', text: 'Error: text is required' })
  })

  test('color converter handles success and invalid colors', async () => {
    expect(await colorPlugin.run(fakeContext, { color: '#ff0000' })).toEqual({
      type: 'json',
      value: {
        result: { hex: '#ff0000', rgb: '255,0,0', hsl: '0,100%,50%' },
      },
    })
    expect(
      await colorPlugin.run(fakeContext, { color: 'not-a-color' })
    ).toEqual({
      type: 'text',
      text: 'Error: invalid color format',
    })
  })

  test('hash generator handles success and empty input', async () => {
    const success = await hashPlugin.run(fakeContext, {
      text: 'abc',
      algorithm: 'SHA-256',
    })

    expect(success).toEqual({
      type: 'json',
      value: {
        result:
          'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
        algorithm: 'SHA-256',
        bytes: 32,
      },
    })
    expect(
      await hashPlugin.run(fakeContext, { text: '', algorithm: 'SHA-256' })
    ).toEqual({ type: 'text', text: 'Error: text is required' })
  })

  test('image Base64 handles success and missing input', async () => {
    const success = await imageBase64Plugin.run(fakeContext, {
      base64: 'aGVsbG8=',
    })

    expect(success).toMatchObject({
      type: 'json',
      value: {
        result: {
          estimatedBytes: 6,
          hasPrefix: true,
        },
      },
    })
    expect(await imageBase64Plugin.run(fakeContext, {})).toEqual({
      type: 'text',
      text: 'Error: base64 is required',
    })
  })

  test('JSON formatter handles minify and invalid validation input', async () => {
    expect(
      await jsonPlugin.run(fakeContext, {
        text: '{"a": 1}',
        mode: 'minify',
      })
    ).toEqual({ type: 'text', text: '{"a":1}' })

    const invalid = await jsonPlugin.run(fakeContext, {
      text: '{',
      mode: 'validate',
    })
    expect(invalid).toMatchObject({
      type: 'json',
      value: { result: { valid: false } },
    })
  })

  test('random picker returns a bounded result from its input', async () => {
    expect(
      await randomPickerPlugin.run(fakeContext, {
        names: 'Ada',
        count: 1,
      })
    ).toEqual({
      type: 'json',
      value: { result: ['Ada'], total: 1 },
    })
  })

  test('regex tester handles matches and invalid patterns', async () => {
    const success = await regexPlugin.run(fakeContext, {
      pattern: 'a+',
      text: 'caaad',
      flags: 'g',
    })

    expect(success).toMatchObject({
      type: 'json',
      value: {
        result: [{ match: 'aaa', index: 1 }],
        matchCount: 1,
      },
    })
    const invalid = await regexPlugin.run(fakeContext, {
      pattern: '[',
      text: 'abc',
      flags: 'g',
    })
    expect(invalid.type).toBe('text')
    expect(invalid.type === 'text' ? invalid.text : '').toStartWith('Error:')
  })

  test('text operations handle intersections and empty sets', async () => {
    expect(
      await textOpsPlugin.run(fakeContext, {
        setA: 'apple\nbanana',
        setB: 'banana\npear',
        operation: 'intersection',
      })
    ).toEqual({
      type: 'json',
      value: { result: ['banana'], operation: 'intersection', count: 1 },
    })
    expect(
      await textOpsPlugin.run(fakeContext, {
        setA: '',
        setB: '',
        operation: 'intersection',
      })
    ).toEqual({
      type: 'json',
      value: { result: [], operation: 'intersection', count: 0 },
    })
  })

  test('timestamp converter handles explicit and invalid timestamps', async () => {
    expect(
      await timestampPlugin.run(fakeContext, {
        timestamp: '0',
        unit: 'seconds',
      })
    ).toEqual({
      type: 'json',
      value: {
        result: '1970-01-01T00:00:00.000Z',
        timestamp: '0',
        unit: 'seconds',
      },
    })
    expect(
      await timestampPlugin.run(fakeContext, {
        timestamp: 'invalid',
        unit: 'seconds',
      })
    ).toEqual({
      type: 'text',
      text: 'Error: invalid timestamp',
    })
  })

  test('UUID generator returns RFC 4122 version 4 shapes', async () => {
    const generated = await uuidPlugin.run(fakeContext, { count: 3 })

    expect(generated.type).toBe('json')
    if (generated.type !== 'json') {
      throw new Error('Expected UUID generator to return JSON')
    }

    const value = generated.value as { result?: unknown; count?: unknown }
    expect(value.count).toBe(3)
    expect(Array.isArray(value.result)).toBe(true)
    if (!Array.isArray(value.result)) {
      throw new Error('Expected UUID generator result to be an array')
    }

    expect(value.result).toHaveLength(3)
    for (const uuid of value.result) {
      expect(uuid).toMatch(
        /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/
      )
    }
  })
})
