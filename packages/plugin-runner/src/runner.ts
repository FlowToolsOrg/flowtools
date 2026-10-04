import type { ToolContext } from '@flowtools/sdk/types'

import { createHash } from 'node:crypto'

import { createExecutionFailure } from '@flowtools/sdk/execution'
import { executeManifestCommand } from '@flowtools/sdk/manifest'

import { loadPlugin, cliManifestTarget } from '../../cli/src/discovery'

export function manifestDigest(manifest: unknown): string {
  const canonical = (value: unknown): unknown => {
    if (Array.isArray(value)) return value.map(canonical)
    if (value && typeof value === 'object')
      return Object.fromEntries(
        Object.entries(value)
          .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
          .map(([key, item]) => [key, canonical(item)])
      )
    return value
  }
  return createHash('sha256')
    .update(JSON.stringify(canonical(manifest)))
    .digest('hex')
}

/** Fixed T1 evaluator. No shell/argv/paths/capabilities or user storage input. */
export async function runValidationCommand(value: unknown) {
  if (!value || typeof value !== 'object')
    throw new Error('Invalid runner request')
  const request = value as Record<string, unknown>
  if (
    Object.keys(request).sort().join(',') !==
      'commandId,input,packageDigest,pluginId' ||
    typeof request.pluginId !== 'string' ||
    request.commandId !== 'run' ||
    typeof request.packageDigest !== 'string'
  ) {
    throw new Error('Invalid runner request')
  }
  const plugin = await loadPlugin(request.pluginId)
  if (!plugin)
    return createExecutionFailure(request.pluginId, null, request.input, {
      code: 'LOAD_FAILED',
      message: 'Fixed T1 artifact is unavailable',
    })
  const command = plugin.manifest.commands[0]!
  if (manifestDigest(plugin.manifest) !== request.packageDigest)
    throw new Error('Package changed')
  if (command.effects.length || command.permissions.length) {
    return createExecutionFailure(
      request.pluginId,
      plugin.meta.version,
      request.input,
      {
        code: 'NOT_RUNNABLE',
        message: 'Validation does not grant capabilities',
      }
    )
  }
  const ctx: ToolContext = {
    env: {
      pluginId: plugin.meta.id,
      pluginType: plugin.type,
      platform: 'unknown',
      mode: 'test',
    },
    utils: { now: Date.now },
    ui: { toast: () => {}, openPanel: () => {}, closePanel: () => {} },
    signal: new AbortController().signal,
    log: () => {},
  }
  return executeManifestCommand(
    plugin.manifest,
    'run',
    plugin,
    request.input,
    ctx,
    cliManifestTarget
  )
}

if (import.meta.main) {
  try {
    const bytes = await Bun.stdin.text()
    if (Buffer.byteLength(bytes) > 1_048_576)
      throw new Error('Runner input too large')
    process.stdout.write(
      JSON.stringify(await runValidationCommand(JSON.parse(bytes)))
    )
  } catch {
    // No exception message, path, payload or inherited environment in diagnostics.
    process.stderr.write('RUNNER_FAILED\n')
    process.exitCode = 1
  }
}
