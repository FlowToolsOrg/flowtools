import type { ToolContext } from '../src/types/ctx'

import { describe, expect, test } from 'bun:test'

import { z } from 'zod'

import { executePlugin } from '../src/execution/executor'

function context(signal = new AbortController().signal): ToolContext {
  return {
    env: {
      pluginId: 'executor-fixture',
      pluginType: 'app',
      platform: 'web',
      mode: 'test',
    },
    ui: { toast: () => {}, openPanel: () => {}, closePanel: () => {} },
    signal,
    log: () => {},
    utils: { now: () => Date.now() },
  }
}

const meta = { id: 'executor-fixture', version: '1.2.3' }

describe('executePlugin', () => {
  test('calls real run with parsed defaults and records actual metadata', async () => {
    const inputSchema = z.object({
      text: z.string(),
      count: z.number().default(2),
    })
    const result = await executePlugin(
      {
        meta,
        inputSchema,
        run: (_ctx, input: z.infer<typeof inputSchema>) => ({
          type: 'json',
          value: { result: input.text.repeat(input.count) },
        }),
      },
      { text: 'hello' },
      context()
    )
    expect(result).toMatchObject({
      success: true,
      pluginId: meta.id,
      pluginVersion: meta.version,
      inputSummary: { kind: 'object', size: 1 },
      data: { type: 'json', value: { result: 'hellohello' } },
    })
    expect(result.startedAt).toBeGreaterThan(0)
    expect(result.finishedAt).toBeGreaterThanOrEqual(result.startedAt)
    expect(result.durationMs).toBeGreaterThanOrEqual(0)
  })

  test('rejects invalid schema input before invoking the plugin', async () => {
    let calls = 0
    const result = await executePlugin(
      {
        meta,
        inputSchema: z.object({ count: z.number().int().positive() }),
        run: () => {
          calls += 1
        },
      },
      { count: 'not-a-number' },
      context()
    )
    expect(result).toMatchObject({
      success: false,
      error: { code: 'INPUT_INVALID' },
    })
    expect(calls).toBe(0)
    if (!result.success)
      expect(result.error.issues?.[0]?.path).toEqual(['count'])
  })

  test('rejects a missing run entry without fabricating success', async () => {
    expect(await executePlugin({ meta }, {}, context())).toMatchObject({
      success: false,
      error: { code: 'NOT_RUNNABLE' },
    })
  })

  test('rejects mismatched host context identity before run', async () => {
    let calls = 0
    const ctx = context()
    ctx.env.pluginId = 'another-plugin'
    expect(
      await executePlugin(
        {
          meta,
          run: () => {
            calls += 1
          },
        },
        {},
        ctx
      )
    ).toMatchObject({ success: false, error: { code: 'PLUGIN_ID_MISMATCH' } })
    expect(calls).toBe(0)
  })

  test.each(['error', 'string'])(
    'normalizes a thrown %s into a stable failed envelope',
    async kind => {
      const result = await executePlugin(
        {
          meta,
          run: () => {
            if (kind === 'error') throw new Error('actual plugin failure')
            throw 'actual plugin failure'
          },
        },
        {},
        context()
      )
      expect(result).toMatchObject({
        success: false,
        error: { code: 'EXECUTION_FAILED', message: 'actual plugin failure' },
      })
    }
  )

  test('does not call run for an already-cancelled request', async () => {
    const controller = new AbortController()
    controller.abort()
    let calls = 0
    const result = await executePlugin(
      {
        meta,
        run: () => {
          calls += 1
        },
      },
      {},
      context(),
      {
        signal: controller.signal,
      }
    )
    expect(result).toMatchObject({ success: false, error: { code: 'ABORTED' } })
    expect(calls).toBe(0)
  })

  test('also honors cancellation from the host context', async () => {
    const controller = new AbortController()
    controller.abort()
    expect(
      await executePlugin(
        { meta, run: () => 'must not run' },
        {},
        context(controller.signal)
      )
    ).toMatchObject({ success: false, error: { code: 'ABORTED' } })
  })

  test('forwards cancellation and ignores a late plugin result', async () => {
    const controller = new AbortController()
    let receivedSignal: AbortSignal | undefined
    let finish: ((value: string) => void) | undefined
    const running = executePlugin(
      {
        meta,
        run: ctx => {
          receivedSignal = ctx.signal
          return new Promise<string>(resolve => {
            finish = resolve
          })
        },
      },
      {},
      context(),
      { signal: controller.signal }
    )
    await Promise.resolve()
    controller.abort()
    const result = await running
    expect(receivedSignal?.aborted).toBe(true)
    expect(result).toMatchObject({ success: false, error: { code: 'ABORTED' } })
    finish?.('late-success')
    await Promise.resolve()
    expect(result.success).toBe(false)
  })

  test('bounds a non-cooperative async plugin and aborts its signal', async () => {
    let receivedSignal: AbortSignal | undefined
    const result = await executePlugin(
      {
        meta,
        run: ctx => {
          receivedSignal = ctx.signal
          return new Promise<never>(() => {})
        },
      },
      {},
      context(),
      { timeoutMs: 5 }
    )
    expect(result).toMatchObject({ success: false, error: { code: 'TIMEOUT' } })
    expect(receivedSignal?.aborted).toBe(true)
  })

  test('honors cancellation triggered during schema validation', async () => {
    const controller = new AbortController()
    let calls = 0
    const result = await executePlugin(
      {
        meta,
        inputSchema: {
          safeParse: input => {
            controller.abort()
            return { success: true, data: input }
          },
        },
        run: () => {
          calls += 1
        },
      },
      {},
      context(controller.signal)
    )
    expect(result).toMatchObject({ success: false, error: { code: 'ABORTED' } })
    expect(calls).toBe(0)
  })

  test('handles a synchronous timeout scheduler without starting run', async () => {
    let calls = 0
    let cleanups = 0
    const result = await executePlugin(
      {
        meta,
        run: () => {
          calls += 1
        },
      },
      {},
      context(),
      {
        startTimeout: callback => {
          callback()
          return () => {
            cleanups += 1
          }
        },
      }
    )
    expect(result).toMatchObject({ success: false, error: { code: 'TIMEOUT' } })
    expect(calls).toBe(0)
    expect(cleanups).toBe(1)
  })

  test('cleans resources after plugin failure', async () => {
    const controller = new AbortController()
    let signal: AbortSignal | undefined
    let cleanups = 0
    const result = await executePlugin(
      {
        meta,
        run: ctx => {
          signal = ctx.signal
          throw new Error('failed')
        },
      },
      {},
      context(controller.signal),
      {
        signal: controller.signal,
        startTimeout: () => () => {
          cleanups += 1
        },
      }
    )
    controller.abort()
    expect(result).toMatchObject({
      success: false,
      error: { code: 'EXECUTION_FAILED' },
    })
    expect(signal?.aborted).toBe(false)
    expect(cleanups).toBe(1)
  })

  test('cleans timers and upstream abort listeners after success', async () => {
    const controller = new AbortController()
    let receivedSignal: AbortSignal | undefined
    let cleanups = 0
    const result = await executePlugin(
      {
        meta,
        run: ctx => {
          receivedSignal = ctx.signal
          return 'actual result'
        },
      },
      {},
      context(),
      {
        signal: controller.signal,
        startTimeout: (callback, ms) => {
          const timer = setTimeout(callback, ms)
          return () => {
            cleanups += 1
            clearTimeout(timer)
          }
        },
      }
    )
    controller.abort()
    expect(result.success).toBe(true)
    expect(receivedSignal?.aborted).toBe(false)
    expect(cleanups).toBe(1)
  })

  test.each([0, -1, Infinity, NaN, 0.5, 2_147_483_648])(
    'rejects invalid timeout %s',
    async timeoutMs => {
      expect(
        await executePlugin(
          { meta, run: () => 'not executed' },
          {},
          context(),
          { timeoutMs }
        )
      ).toMatchObject({ success: false, error: { code: 'TIMEOUT_INVALID' } })
    }
  )

  test('records only shape information, never raw secret input', async () => {
    const result = await executePlugin(
      { meta, run: () => null },
      { password: 'secret-token' },
      context()
    )
    expect(JSON.stringify(result)).not.toContain('secret-token')
    expect(result.inputSummary).toEqual({ kind: 'object', size: 1 })
  })
})
