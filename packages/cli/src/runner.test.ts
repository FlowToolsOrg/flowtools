import type { ToolContext } from '@flowtools/sdk/types'

import { describe, expect, test } from 'bun:test'

import { z } from 'zod'

import { createPluginRunner } from './runner'

const meta = { id: 'runner-fixture', version: '1.2.3' }
const createContext = (pluginId: string): ToolContext => ({
  env: { pluginId, pluginType: 'app', platform: 'desktop', mode: 'test' },
  signal: new AbortController().signal,
  ui: { toast: () => {}, openPanel: () => {}, closePanel: () => {} },
  log: () => {},
  utils: { now: Date.now },
})

describe('createPluginRunner SDK adapter', () => {
  test('validates defaults and returns the shared versioned envelope', async () => {
    let cleanups = 0
    const run = createPluginRunner({
      createContext,
      startTimeout: () => () => {
        cleanups += 1
      },
      loadPlugin: async () => ({
        meta,
        inputSchema: z.object({ count: z.number().default(2) }),
        run: (_ctx, input: { count: number }) => input.count,
      }),
    })
    expect(await run(meta.id, {})).toMatchObject({
      success: true,
      data: 2,
      pluginId: meta.id,
      pluginVersion: '1.2.3',
      inputSummary: { kind: 'object', size: 0 },
    })
    expect(cleanups).toBe(1)
  })

  test('rejects schema input without running', async () => {
    let calls = 0
    const run = createPluginRunner({
      createContext,
      loadPlugin: async () => ({
        meta,
        inputSchema: z.object({ count: z.number() }),
        run: () => {
          calls += 1
        },
      }),
    })
    expect(await run(meta.id, { count: 'bad' })).toMatchObject({
      success: false,
      error: { code: 'INPUT_INVALID' },
    })
    expect(calls).toBe(0)
  })

  test.each([new Error('plugin exploded'), 'plugin exploded'])(
    'normalizes actual thrown failures',
    async failure => {
      const run = createPluginRunner({
        createContext,
        loadPlugin: async () => ({
          meta,
          run: () => {
            throw failure
          },
        }),
      })
      expect(await run(meta.id, {})).toMatchObject({
        success: false,
        error: { code: 'EXECUTION_FAILED', message: 'plugin exploded' },
      })
    }
  )

  test('normalizes loader failures separately', async () => {
    const run = createPluginRunner({
      createContext,
      loadPlugin: async () => {
        throw new Error('load failed')
      },
    })
    expect(await run(meta.id, {})).toMatchObject({
      success: false,
      pluginVersion: null,
      error: { code: 'LOAD_FAILED', message: 'load failed' },
    })
  })

  test('bounds a non-settling run', async () => {
    const run = createPluginRunner({
      createContext,
      loadPlugin: async () => ({
        meta,
        run: () => new Promise<never>(() => {}),
      }),
    })
    expect(await run(meta.id, {}, { timeout: 5 })).toMatchObject({
      success: false,
      error: { code: 'TIMEOUT' },
    })
  })

  test('reports missing plugin and missing run truthfully', async () => {
    const missing = createPluginRunner({
      createContext,
      loadPlugin: async () => null,
    })
    const panel = createPluginRunner({
      createContext,
      loadPlugin: async () => ({ meta }),
    })
    expect(await missing(meta.id, {})).toMatchObject({
      success: false,
      error: { code: 'PLUGIN_NOT_FOUND' },
    })
    expect(await panel(meta.id, {})).toMatchObject({
      success: false,
      error: { code: 'NOT_RUNNABLE' },
    })
  })

  test('honors cancellation without invoking run', async () => {
    const controller = new AbortController()
    controller.abort()
    const run = createPluginRunner({
      createContext,
      loadPlugin: async () => ({
        meta,
        run: () => {
          throw new Error('must not run')
        },
      }),
    })
    expect(await run(meta.id, {}, { signal: controller.signal })).toMatchObject(
      {
        success: false,
        error: { code: 'ABORTED' },
      }
    )
  })
})
