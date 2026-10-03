import type { ToolContext } from '../src/types/ctx'
import type { ToolPlugin } from '../src/types/plugin'

import { describe, expect, test } from 'bun:test'

import { withWatchdog } from '../src/registry/watchdog'

async function getError(promise: Promise<unknown>): Promise<Error> {
  try {
    await promise
  } catch (error) {
    return error instanceof Error ? error : new Error(String(error))
  }

  throw new Error('Expected promise to reject')
}

function createContext(): ToolContext {
  return {
    env: {
      pluginId: 'watchdog-plugin',
      pluginType: 'tool',
      platform: 'desktop',
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
}

function createPlugin<TInput, TOutput>(
  run: ToolPlugin<TInput, TOutput>['run']
): ToolPlugin<TInput, TOutput> {
  return {
    type: 'tool',
    meta: {
      id: 'watchdog-plugin',
      name: 'Watchdog Plugin',
      version: '1.0.0',
    },
    run,
  }
}

describe('withWatchdog', () => {
  test('returns successful plugin output with a watchdog signal', async () => {
    const context = createContext()
    let receivedSignal: AbortSignal | undefined
    const run = withWatchdog(
      createPlugin<{ value: number }, number>((ctx, input) => {
        receivedSignal = ctx.signal
        return input.value * 2
      })
    )

    expect(await run(context, { value: 4 })).toBe(8)
    expect(receivedSignal).toBeDefined()
    expect(receivedSignal).not.toBe(context.signal)
  })

  test('propagates the original plugin error', async () => {
    const error = new Error('plugin failed')
    const run = withWatchdog(
      createPlugin<void, never>(() => {
        throw error
      })
    )

    expect(await getError(run(createContext(), undefined))).toBe(error)
  })

  test('clears the timeout after a successful run', async () => {
    let receivedSignal: AbortSignal | undefined
    const run = withWatchdog(
      createPlugin<void, string>(ctx => {
        receivedSignal = ctx.signal
        return 'done'
      }),
      { timeoutMs: 5 }
    )

    expect(await run(createContext(), undefined)).toBe('done')
    await Bun.sleep(25)

    expect(receivedSignal?.aborted).toBe(false)
  })

  test('aborts the signal so a cooperative plugin can exit', async () => {
    let receivedSignal: AbortSignal | undefined
    const run = withWatchdog(
      createPlugin<void, string>(ctx => {
        receivedSignal = ctx.signal
        return new Promise(resolve => {
          ctx.signal.addEventListener('abort', () => resolve('aborted'), {
            once: true,
          })
        })
      }),
      { timeoutMs: 5 }
    )

    expect(await run(createContext(), undefined)).toBe('aborted')
    expect(receivedSignal?.aborted).toBe(true)
  })
})
