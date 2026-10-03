/**
 * Plugin runner — loads and executes a plugin's run() function.
 */

import type { OutputFormat } from './types'

import { createCLIToolContext } from './context'
import { loadPlugin } from './discovery'
import { formatRaw, formatResult } from './formatter'

export interface RunResult {
  success: boolean
  data?: unknown
  error?: string
}

interface RunnablePlugin {
  run?: (context: unknown, input: Record<string, unknown>) => unknown
}

export interface PluginRunnerDependencies {
  loadPlugin: (pluginId: string) => Promise<RunnablePlugin | null>
  createContext: (pluginId: string) => object
  startTimeout?: (callback: () => void, delayMs: number) => () => void
}

type PluginRunner = (
  pluginId: string,
  input: Record<string, unknown>,
  options?: { timeout?: number }
) => Promise<RunResult>

/**
 * Create an isolated runner with injectable discovery and context boundaries.
 */
export function createPluginRunner(
  dependencies: PluginRunnerDependencies
): PluginRunner {
  return async (pluginId, input, options) => {
    let cancelTimeout: (() => void) | undefined

    try {
      const plugin = await dependencies.loadPlugin(pluginId)
      if (!plugin) {
        return { success: false, error: `Plugin not found: ${pluginId}` }
      }

      if (!plugin.run) {
        return {
          success: false,
          error: `Plugin ${pluginId} has no run() function`,
        }
      }

      const ctx = dependencies.createContext(pluginId)
      const timeoutMs = options?.timeout ?? 30_000
      const controller = new AbortController()
      const startTimeout =
        dependencies.startTimeout ??
        ((callback: () => void, delayMs: number) => {
          const timeout = setTimeout(callback, delayMs)
          return () => clearTimeout(timeout)
        })
      cancelTimeout = startTimeout(() => controller.abort(), timeoutMs)
      const execCtx = { ...ctx, signal: controller.signal }

      const result = await Promise.race([
        Promise.resolve(plugin.run(execCtx, input)),
        new Promise<never>((_, reject) => {
          controller.signal.addEventListener(
            'abort',
            () => {
              reject(
                new Error(`Plugin execution timed out after ${timeoutMs}ms`)
              )
            },
            { once: true }
          )
        }),
      ])

      return { success: true, data: result }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      return { success: false, error: message }
    } finally {
      cancelTimeout?.()
    }
  }
}

const defaultPluginRunner = createPluginRunner({
  loadPlugin,
  createContext: createCLIToolContext,
})

/**
 * Run a plugin by id with the given input.
 * Returns the structured result.
 */
export async function runPlugin(
  pluginId: string,
  input: Record<string, unknown>,
  options?: {
    timeout?: number
  }
): Promise<RunResult> {
  return defaultPluginRunner(pluginId, input, options)
}

/**
 * Run a plugin and print the result to stdout/stderr.
 */
export async function runPluginAndPrint(
  pluginId: string,
  input: Record<string, unknown>,
  format: OutputFormat = 'json',
  options?: {
    timeout?: number
  }
): Promise<number> {
  const result = await runPlugin(pluginId, input, options)

  if (!result.success) {
    process.stderr.write(`${result.error ?? 'Plugin execution failed'}\n`)
    return 1
  }

  if (result.data && typeof result.data === 'object' && 'type' in result.data) {
    const output = formatResult(
      result.data as Parameters<typeof formatResult>[0],
      format
    )
    if (output) process.stdout.write(`${output}\n`)
  } else {
    const output = formatRaw(result.data, format)
    if (output) process.stdout.write(`${output}\n`)
  }

  return 0
}
