import type { DataMutation, DataSnapshot } from '@flowtools/sdk/data'
import type { ToolContext } from '@flowtools/sdk/types'

import { createHash } from 'node:crypto'

import { createExecutionFailure } from '@flowtools/sdk/execution'
import { isJsonValue } from '@flowtools/sdk/manifest'
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
  return (await execute(value, false)).execution
}

/** Host-only fixed T1 process protocol; this is not an untrusted plugin sandbox. */
export async function runManagedCommand(value: unknown) {
  return execute(value, true)
}

async function execute(value: unknown, managed: boolean) {
  if (!value || typeof value !== 'object')
    throw new Error('Invalid runner request')
  const request = value as Record<string, unknown>
  if (
    Object.keys(request).sort().join(',') !==
      (managed
        ? 'commandId,data,input,packageDigest,pluginId'
        : 'commandId,input,packageDigest,pluginId') ||
    typeof request.pluginId !== 'string' ||
    request.commandId !== 'run' ||
    typeof request.packageDigest !== 'string'
  ) {
    throw new Error('Invalid runner request')
  }
  const mutations: DataMutation[] = []
  const failure = (execution: ReturnType<typeof createExecutionFailure>) => ({
    execution,
    mutations,
  })
  let snapshot: DataSnapshot | undefined
  if (managed && request.pluginId === 'plugin-todo-list') {
    const data = request.data as Partial<DataSnapshot> | null
    if (
      !data ||
      Object.keys(data).sort().join(',') !== 'key,revision,value' ||
      data.key !== 'todos' ||
      !Number.isSafeInteger(data.revision) ||
      data.revision! < 0 ||
      !isJsonValue(data.value)
    )
      throw new Error('Invalid runner data')
    snapshot = data as DataSnapshot
  } else if (managed && request.data !== null)
    throw new Error('Invalid runner data')
  const plugin = await loadPlugin(request.pluginId)
  if (!plugin)
    return failure(
      createExecutionFailure(request.pluginId, null, request.input, {
        code: 'LOAD_FAILED',
        message: 'Fixed T1 artifact is unavailable',
      })
    )
  const command = plugin.manifest.commands[0]!
  if (manifestDigest(plugin.manifest) !== request.packageDigest)
    throw new Error('Package changed')
  if (
    !(managed && snapshot) &&
    (command.effects.length || command.permissions.length)
  ) {
    return failure(
      createExecutionFailure(
        request.pluginId,
        plugin.meta.version,
        request.input,
        {
          code: 'NOT_RUNNABLE',
          message: 'Host adapter is unavailable for this command',
        }
      )
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
    data: snapshot
      ? {
          async read(key) {
            if (key !== 'todos') throw new Error('SCOPE_DENIED')
            return structuredClone(snapshot!)
          },
          async write(mutation) {
            if (
              mutation.key !== 'todos' ||
              mutation.expectedRevision !== snapshot!.revision ||
              mutations.length
            )
              throw new Error('REVISION_CONFLICT')
            mutations.push(structuredClone(mutation))
            snapshot = {
              key: 'todos',
              revision: snapshot!.revision + 1,
              value: mutation.value,
            }
            return structuredClone(snapshot)
          },
          async transaction() {
            throw new Error('SCOPE_DENIED')
          },
          watch() {
            return {
              [Symbol.asyncIterator]() {
                throw new Error('SCOPE_DENIED')
              },
            }
          },
        }
      : undefined,
  }
  const execution = await executeManifestCommand(
    plugin.manifest,
    'run',
    plugin,
    request.input,
    ctx,
    cliManifestTarget
  )
  return { execution, mutations: execution.success ? mutations : [] }
}

if (import.meta.main) {
  try {
    const bytes = await Bun.stdin.text()
    if (Buffer.byteLength(bytes) > 1_048_576)
      throw new Error('Runner input too large')
    process.stdout.write(
      JSON.stringify(
        'data' in JSON.parse(bytes)
          ? await runManagedCommand(JSON.parse(bytes))
          : {
              execution: await runValidationCommand(JSON.parse(bytes)),
              mutations: [],
            }
      )
    )
  } catch {
    // No exception message, path, payload or inherited environment in diagnostics.
    process.stderr.write('RUNNER_FAILED\n')
    process.exitCode = 1
  }
}
