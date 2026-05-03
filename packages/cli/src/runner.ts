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

/**
 * Run a plugin by id with the given input.
 * Returns the structured result.
 */
export async function runPlugin(
  pluginId: string,
  input: Record<string, unknown>,
  options?: {
    format?: OutputFormat
    timeout?: number
  }
): Promise<RunResult> {
  const plugin = await loadPlugin(pluginId)
  if (!plugin) {
    return { success: false, error: `Plugin not found: ${pluginId}` }
  }

  if (!plugin.run) {
    return {
      success: false,
      error: `Plugin ${pluginId} has no run() function`,
    }
  }

  const ctx = createCLIToolContext(pluginId)

  // Apply timeout
  const timeoutMs = options?.timeout ?? 30_000
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), timeoutMs)

  // Override signal with custom timeout
  const execCtx = { ...ctx, signal: controller.signal }

  try {
    const result = await Promise.race([
      Promise.resolve(plugin.run(execCtx, input)),
      new Promise<never>((_, reject) => {
        controller.signal.addEventListener('abort', () => {
          reject(new Error(`Plugin execution timed out after ${timeoutMs}ms`))
        })
      }),
    ])

    clearTimeout(timeout)
    return { success: true, data: result }
  } catch (err) {
    clearTimeout(timeout)
    const message = err instanceof Error ? err.message : String(err)
    return { success: false, error: message }
  }
}

/**
 * Run a plugin and print the result to stdout/stderr.
 */
export async function runPluginAndPrint(
  pluginId: string,
  input: Record<string, unknown>,
  format: OutputFormat = 'json'
): Promise<number> {
  const result = await runPlugin(pluginId, input, { format })

  if (!result.success) {
    console.error(`Error: ${result.error}`)
    return 1
  }

  if (result.data && typeof result.data === 'object' && 'type' in result.data) {
    const output = formatResult(
      result.data as Parameters<typeof formatResult>[0],
      format
    )
    console.log(output)
  } else {
    console.log(formatRaw(result.data, format))
  }

  return 0
}
