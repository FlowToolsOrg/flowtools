import type { PluginManifestV1, JsonValue } from '@flowtools/sdk/manifest'
import type { ToolContext } from '@flowtools/sdk/types'

import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { executeManifestCommand, isJsonValue } from '@flowtools/sdk/manifest'
import {
  executeManifestService,
  serviceTargetSchema,
} from '@flowtools/sdk/services'

import { manifestDigest } from '../../src/manifest-digest'
import { serveRunner } from '../../src/protocol'

await serveRunner(async (value, frames) => {
  const request = value as Record<string, unknown>
  if (
    !request ||
    Object.keys(request).sort().join(',') !==
      (request.serviceTarget
        ? 'commandId,data,deadline,input,packageDigest,pluginId,serviceTarget'
        : 'commandId,data,deadline,input,packageDigest,pluginId') ||
    request.data !== null ||
    typeof request.deadline !== 'number' ||
    !Number.isFinite(request.deadline) ||
    !isJsonValue(request.input)
  )
    throw new Error('INVALID_REQUEST')
  const manifests = JSON.parse(
    readFileSync(resolve(import.meta.dirname, 'manifests.json'), 'utf8')
  ) as PluginManifestV1[]
  const manifest = manifests.find(
    item =>
      item.id === request.pluginId &&
      manifestDigest(item) === request.packageDigest
  )
  if (!manifest) throw new Error('PACKAGE_CHANGED')
  const ctx: ToolContext = {
    env: {
      pluginId: manifest.id,
      pluginType: 'app',
      platform: 'unknown',
      mode: 'test',
    },
    utils: { now: Date.now },
    ui: { toast: () => {}, openPanel: () => {}, closePanel: () => {} },
    signal: new AbortController().signal,
    log: () => {},
    services: frames.services,
    artifacts: frames.artifacts,
  }
  const implementation = {
    meta: { id: manifest.id, version: manifest.version },
    async run(ctx: ToolContext, input: JsonValue) {
      const data = input as { text: string; mode: string }
      if (manifest.id === 'fixture-c') {
        if (data.mode === 'daemon') {
          const descendant = Bun.spawn(
            [process.execPath, '--eval', 'await Bun.sleep(60000)'],
            { stdin: 'ignore', stdout: 'ignore', stderr: 'ignore' }
          )
          descendant.unref()
          // Exercise timeout and whole-group cleanup even when spawn returns promptly.
          await new Promise<void>(() => {})
          return `daemon:${descendant.pid}`
        }
        if (data.mode === 'wait')
          await new Promise<void>(resolve => setTimeout(resolve, 9000))
        if (data.mode === 'fail')
          throw new Error('PRIVATE_PROVIDER_SECRET_DONT_EXPORT')
        if (data.mode === 'allowed' || data.mode === 'extra')
          return ctx.artifacts!.read(data.mode)
        return `C:${data.text}`
      }
      const result = await ctx.services!.call(
        {
          publisher: 'flowtools',
          id: manifest.id === 'fixture-a' ? 'fixture-b' : 'fixture-c',
          service: 'transform',
          operation: 'run',
        },
        data
      )
      if (typeof result !== 'string') throw new Error('OUTPUT_INVALID')
      return `${manifest.id === 'fixture-a' ? 'A' : 'B'}:${result}`
    },
  }
  const target = {
    hostVersion: '0.1.0',
    sdkVersion: '0.0.0',
    platform: 'windows' as const,
    arch: 'x64' as const,
  }
  const options = { timeoutMs: Math.max(1, request.deadline - Date.now()) }
  const execution = request.serviceTarget
    ? await (async () => {
        const selector = serviceTargetSchema.parse(request.serviceTarget)
        return executeManifestService(
          manifest,
          selector.service,
          selector.operation,
          implementation,
          request.input,
          ctx,
          target,
          options
        )
      })()
    : await executeManifestCommand(
        manifest,
        'run',
        implementation,
        request.input,
        ctx,
        target,
        options
      )
  return { execution, mutations: [] }
})
