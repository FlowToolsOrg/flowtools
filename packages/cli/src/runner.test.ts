import { describe, expect, test } from 'bun:test'

import { createPluginRunner } from './runner'

const createContext = () => ({ source: 'test' })

describe('createPluginRunner', () => {
  test('returns successful plugin data', async () => {
    let cancelCalls = 0
    const runPlugin = createPluginRunner({
      startTimeout: (callback, delayMs) => {
        const timeout = setTimeout(callback, delayMs)
        return () => {
          cancelCalls += 1
          clearTimeout(timeout)
        }
      },
      createContext,
      loadPlugin: async () => ({
        run: (context, input) => ({ context, input }),
      }),
    })

    const result = await runPlugin('success-plugin', { value: 2 })

    expect(result.success).toBe(true)
    const data = result.data as {
      context: { source: string; signal: AbortSignal }
      input: Record<string, unknown>
    }
    expect(data.context.source).toBe('test')
    expect(data.context.signal).toBeInstanceOf(AbortSignal)
    expect(data.input).toEqual({ value: 2 })
    expect(cancelCalls).toBe(1)
  })

  test('normalizes thrown plugin errors', async () => {
    let cancelCalls = 0
    const runPlugin = createPluginRunner({
      startTimeout: (callback, delayMs) => {
        const timeout = setTimeout(callback, delayMs)
        return () => {
          cancelCalls += 1
          clearTimeout(timeout)
        }
      },
      createContext,
      loadPlugin: async () => ({
        run: () => {
          throw new Error('plugin exploded')
        },
      }),
    })

    const result = await runPlugin('error-plugin', {})

    expect(result).toEqual({
      success: false,
      error: 'plugin exploded',
    })
    expect(cancelCalls).toBe(1)
  })

  test('normalizes non-Error plugin failures', async () => {
    const runPlugin = createPluginRunner({
      createContext,
      loadPlugin: async () => ({
        run: () => {
          throw 'plugin failed'
        },
      }),
    })

    expect(await runPlugin('error-plugin', {})).toEqual({
      success: false,
      error: 'plugin failed',
    })
  })

  test('normalizes loader failures', async () => {
    const runPlugin = createPluginRunner({
      createContext,
      loadPlugin: async () => {
        throw new Error('load failed')
      },
    })

    expect(await runPlugin('broken-plugin', {})).toEqual({
      success: false,
      error: 'load failed',
    })
  })

  test('times out a non-settling plugin', async () => {
    let cancelCalls = 0
    const runPlugin = createPluginRunner({
      startTimeout: (callback, delayMs) => {
        const timeout = setTimeout(callback, delayMs)
        return () => {
          cancelCalls += 1
          clearTimeout(timeout)
        }
      },
      createContext,
      loadPlugin: async () => ({
        run: () => new Promise<never>(() => {}),
      }),
    })

    const result = await runPlugin('slow-plugin', {}, { timeout: 5 })

    expect(result).toEqual({
      success: false,
      error: 'Plugin execution timed out after 5ms',
    })
    expect(cancelCalls).toBe(1)
  })

  test('reports a loaded plugin without run()', async () => {
    const runPlugin = createPluginRunner({
      createContext,
      loadPlugin: async () => ({}),
    })

    const result = await runPlugin('panel-only', {})

    expect(result).toEqual({
      success: false,
      error: 'Plugin panel-only has no run() function',
    })
  })
})
