import type { DataMutation, DataSnapshot } from '@flowtools/sdk/data'
import type { ToolContext } from '@flowtools/sdk/types'

import { createExecutionFailure } from '@flowtools/sdk/execution'
import { isJsonValue } from '@flowtools/sdk/manifest'
import { executeManifestCommand } from '@flowtools/sdk/manifest'
import {
  executeManifestService,
  serviceTargetSchema,
} from '@flowtools/sdk/services'

import {
  loadPlugin,
  loadBuiltinServices,
  cliManifestTarget,
} from '../../cli/src/discovery'

import { manifestDigest } from './manifest-digest'
import { serveRunner, type RunnerFrames } from './protocol'
export { manifestDigest } from './manifest-digest'

/** Fixed T1 evaluator. No shell/argv/paths/capabilities or user storage input. */
export async function runValidationCommand(value: unknown) {
  return (await execute(value, false)).execution
}

/** Host-only fixed T1 process protocol; this is not an untrusted plugin sandbox. */
export async function runManagedCommand(value: unknown) {
  return execute(value, true)
}

async function execute(
  value: unknown,
  managed: boolean,
  frames?: RunnerFrames
) {
  if (!value || typeof value !== 'object')
    throw new Error('Invalid runner request')
  const request = value as Record<string, unknown>
  const requestKeys = Object.keys(request)
    .filter(key => !frames || !['deadline', 'serviceTarget'].includes(key))
    .sort()
    .join(',')
  if (
    requestKeys !==
      (managed
        ? 'commandId,data,input,packageDigest,pluginId'
        : 'commandId,input,packageDigest,pluginId') ||
    typeof request.pluginId !== 'string' ||
    (request.serviceTarget === undefined && request.commandId !== 'run') ||
    typeof request.packageDigest !== 'string'
  ) {
    throw new Error('Invalid runner request')
  }
  if (
    frames &&
    (typeof request.deadline !== 'number' ||
      !Number.isFinite(request.deadline) ||
      request.deadline <= Date.now())
  )
    throw new Error('TIMEOUT')
  if (frames && request.serviceTarget !== undefined) {
    const target = serviceTargetSchema.parse(request.serviceTarget)
    if (
      target.id !== request.pluginId ||
      request.commandId !== `svc:${target.service}:${target.operation}` ||
      request.data !== null
    )
      throw new Error('INVALID_REQUEST')
    const loaded = await loadBuiltinServices(target.id, request.packageDigest)
    const implementation =
      loaded?.implementations[target.service]?.[target.operation]
    if (
      !loaded ||
      !implementation ||
      manifestDigest(loaded.manifest) !== request.packageDigest
    )
      throw new Error('LOAD_FAILED')
    const execution = await executeManifestService(
      loaded.manifest,
      target.service,
      target.operation,
      implementation,
      request.input,
      {
        env: {
          pluginId: target.id,
          pluginType: loaded.manifest.type,
          platform: 'unknown',
          mode: 'test',
        },
        utils: { now: Date.now },
        ui: { toast: () => {}, openPanel: () => {}, closePanel: () => {} },
        signal: new AbortController().signal,
        log: () => {},
        services: frames.services,
        artifacts: frames.artifacts,
      },
      cliManifestTarget,
      { timeoutMs: Math.max(1, (request.deadline as number) - Date.now()) }
    )
    return { execution, mutations: [] }
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
    !(frames && plugin.manifest.dependencies.services.length) &&
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
    services: frames?.services,
    artifacts: frames?.artifacts,
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
    await serveRunner((value, frames) => execute(value, true, frames))
  } catch {
    // No exception message, path, payload or inherited environment in diagnostics.
    process.stderr.write('RUNNER_FAILED\n')
    process.exitCode = 1
  }
}
