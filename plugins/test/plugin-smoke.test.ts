import type { StorageCapability, ToolContext } from '@flowtools/sdk/types'

import { expect, test } from 'bun:test'

import { createPluginRunner, loadPlugin, scanPlugins } from '@flowtools/cli'
import { z } from 'zod'

import { pluginSmokeFixtures } from './smoke-fixtures'

function controlledContext(pluginId: string) {
  const values = new Map<string, unknown>()
  const requests: string[] = []
  const writes: string[] = []
  const storage: StorageCapability = {
    get: <T>(key: string) => values.get(key) as T | undefined,
    set: (key, value) => {
      writes.push(key)
      values.set(key, value)
    },
    remove: key => {
      values.delete(key)
    },
    zustand: () => ({
      getItem: () => null,
      setItem: () => {},
      removeItem: () => {},
    }),
  }
  const ctx: ToolContext = {
    env: { pluginId, pluginType: 'app', platform: 'unknown', mode: 'test' },
    signal: new AbortController().signal,
    ui: { toast: () => {}, openPanel: () => {}, closePanel: () => {} },
    log: () => {},
    utils: { now: Date.now },
    storage,
    request: async input => {
      const url =
        typeof input === 'string'
          ? input
          : input instanceof URL
            ? input.href
            : input.url
      if (url !== 'https://latency.fixture.invalid/')
        throw new Error('Fixture request scope denied')
      requests.push(url)
      return new Response('fixture response', { status: 200 })
    },
  }
  return { ctx, values, requests, writes }
}

test('smoke fixtures exactly cover the runnable CLI inventory', () => {
  const discovered = scanPlugins()
  expect(discovered).toHaveLength(12)
  expect(discovered.every(plugin => plugin.hasRun && plugin.hasSchema)).toBe(
    true
  )
  expect(discovered.map(plugin => plugin.id).sort()).toEqual(
    pluginSmokeFixtures.map(fixture => fixture.id).sort()
  )
})

for (const fixture of pluginSmokeFixtures) {
  test(
    'real compiled smoke: ' + fixture.id,
    async () => {
      const controlled = controlledContext(fixture.id)
      const runner = createPluginRunner({
        loadPlugin,
        createContext: () => controlled.ctx,
      })
      const plugin = await loadPlugin(fixture.id)
      expect(plugin).not.toBeNull()
      const success = await runner(fixture.id, fixture.input)
      expect(success).toMatchObject({
        success: true,
        pluginId: fixture.id,
        pluginVersion: plugin?.meta.version,
      })
      if (!success.success)
        throw new Error('Actual smoke run failed: ' + success.error.code)
      expect(success.durationMs).toBe(success.finishedAt - success.startedAt)
      expect(success.data).toMatchObject(fixture.expected)
      if (fixture.id === 'plugin-uuid-generator') {
        const value = z
          .object({
            type: z.literal('json'),
            value: z.object({
              result: z
                .array(
                  z
                    .string()
                    .regex(
                      /^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/
                    )
                )
                .length(2),
            }),
          })
          .parse(success.data)
        expect(new Set(value.value.result).size).toBe(2)
      }
      if (fixture.id === 'plugin-website-latency')
        expect(controlled.requests).toEqual([
          'https://latency.fixture.invalid/',
        ])
      if (fixture.id === 'plugin-todo-list')
        expect(controlled.values.get('todos')).toEqual([
          { todo: 'fixture task', deadline: '' },
        ])
      const writesBeforeReject = controlled.writes.length
      const requestsBeforeReject = controlled.requests.length
      expect(await runner(fixture.id, null)).toMatchObject({
        success: false,
        error: { code: 'INPUT_INVALID' },
      })
      const controller = new AbortController()
      controller.abort()
      expect(
        await runner(fixture.id, fixture.input, { signal: controller.signal })
      ).toMatchObject({ success: false, error: { code: 'ABORTED' } })
      expect(controlled.writes).toHaveLength(writesBeforeReject)
      expect(controlled.requests).toHaveLength(requestsBeforeReject)
    },
    30_000
  )
}

test('real compiled Todo exception becomes an SDK failure without overwriting data', async () => {
  const controlled = controlledContext('plugin-todo-list')
  controlled.values.set('todos', [null])
  const runner = createPluginRunner({
    loadPlugin,
    createContext: () => controlled.ctx,
  })
  expect(
    await runner('plugin-todo-list', { todo: 'not stored' })
  ).toMatchObject({ success: false, error: { code: 'EXECUTION_FAILED' } })
  expect(controlled.values.get('todos')).toEqual([null])
  expect(controlled.writes).toEqual([])
})

test('CLI exposes the same actual capability exception without a raw fetch fallback', async () => {
  const controlled = controlledContext('plugin-website-latency')
  delete controlled.ctx.request
  const runner = createPluginRunner({
    loadPlugin,
    createContext: () => controlled.ctx,
  })
  expect(await runner('plugin-website-latency', { urls: [] })).toMatchObject({
    success: false,
    error: {
      code: 'EXECUTION_FAILED',
      message: 'Network capability is unavailable',
    },
  })
  expect(controlled.requests).toEqual([])
})
