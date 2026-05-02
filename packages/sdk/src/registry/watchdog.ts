import type { ToolContext } from '../types/ctx'
import type { ToolPlugin } from '../types/plugin'

const DEFAULT_TIMEOUT_MS = 30_000

interface WatchdogOptions {
  timeoutMs?: number
}

/**
 * Wraps a tool plugin's run() with timeout detection.
 * If execution exceeds timeoutMs, the abort signal is triggered.
 */
export function withWatchdog<TInput, TOutput>(
  plugin: ToolPlugin<TInput, TOutput>,
  options?: WatchdogOptions
): (ctx: ToolContext, input: TInput) => Promise<TOutput> {
  const timeoutMs = options?.timeoutMs ?? DEFAULT_TIMEOUT_MS

  return async (ctx: ToolContext, input: TInput): Promise<TOutput> => {
    const controller = new AbortController()
    const timeoutId = setTimeout(() => {
      controller.abort()
    }, timeoutMs)

    const wrappedCtx: ToolContext = {
      ...ctx,
      signal: controller.signal,
    }

    try {
      const result = await plugin.run(wrappedCtx, input)

      return result
    } finally {
      clearTimeout(timeoutId)
    }
  }
}
