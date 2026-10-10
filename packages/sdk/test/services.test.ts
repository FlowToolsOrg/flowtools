import type { ToolContext } from '../src/types/ctx'

import { expect, test } from 'bun:test'

import {
  executeManifestService,
  serviceTargetSchema,
} from '../src/services/service-execution'

import { serviceManifestFixture } from './fixtures/dependencies-v1'
import { manifestTarget } from './fixtures/manifest-v1'

function context(signal = new AbortController().signal): ToolContext {
  return {
    env: {
      pluginId: 'fixture-plugin',
      pluginType: 'tool',
      mode: 'test',
      platform: 'unknown',
    },
    utils: { now: Date.now },
    ui: { toast: () => {}, openPanel: () => {}, closePanel: () => {} },
    signal,
    log: () => {},
  }
}
test('service selectors reject caller, grant, executable and identity spoofing fields', () => {
  const target = {
    publisher: 'flowtools',
    id: 'fixture-plugin',
    service: 'text-transform',
    operation: 'convert',
  }
  expect(serviceTargetSchema.parse(target)).toEqual(target)
  for (const field of [
    'caller',
    'parentRunId',
    'version',
    'digest',
    'grant',
    'path',
    'argv',
  ])
    expect(
      serviceTargetSchema.safeParse({ ...target, [field]: 'injected' }).success
    ).toBe(false)
  expect(
    serviceTargetSchema.safeParse({ ...target, id: '../private' }).success
  ).toBe(false)
})
test('actual service implementation receives validated defaults and enforces output contract', async () => {
  const manifest = serviceManifestFixture()
  let calls = 0
  const implementation = {
    meta: { id: manifest.id, version: manifest.version },
    run: (_ctx: ToolContext, input: { text: string; count: number }) => {
      calls++
      return input.text.repeat(input.count)
    },
  }
  const run = (input: unknown) =>
    executeManifestService(
      manifest,
      'text-transform',
      'convert',
      implementation,
      input,
      context(),
      manifestTarget
    )
  const output = await run({ text: 'hello' })
  expect(output.success && output.data).toBe('hello')
  expect((await run({ text: 3 })).success).toBe(false)
  expect(calls).toBe(1)
  const invalid = await executeManifestService(
    manifest,
    'text-transform',
    'convert',
    { ...implementation, run: () => ({ credentials: 'private' }) },
    { text: 'ok' },
    context(),
    manifestTarget
  )
  expect(!invalid.success && invalid.error.code).toBe('OUTPUT_INVALID')
})
test('missing services, required validators and wrong provider versions fail before handler entry', async () => {
  const manifest = serviceManifestFixture()
  let called = false
  const implementation = {
    meta: { id: manifest.id, version: manifest.version },
    run: () => {
      called = true
      return 'ok'
    },
  }
  expect(
    (
      await executeManifestService(
        manifest,
        'absent',
        'convert',
        implementation,
        { text: 'ok' },
        context(),
        manifestTarget
      )
    ).success
  ).toBe(false)
  const wrong = await executeManifestService(
    manifest,
    'text-transform',
    'convert',
    { ...implementation, meta: { ...implementation.meta, version: '9.0.0' } },
    { text: 'ok' },
    context(),
    manifestTarget
  )
  expect(!wrong.success && wrong.error.code).toBe('PLUGIN_ID_MISMATCH')
  manifest.services![0]!.operations[0]!.runtimeValidation.input = 'required'
  expect(
    (
      await executeManifestService(
        manifest,
        'text-transform',
        'convert',
        implementation,
        { text: 'ok' },
        context(),
        manifestTarget
      )
    ).success
  ).toBe(false)
  expect(called).toBe(false)
})
test('service execution inherits caller cancellation and a smaller timeout', async () => {
  const manifest = serviceManifestFixture()
  const controller = new AbortController()
  controller.abort()
  let calls = 0
  const implementation = {
    meta: { id: manifest.id, version: manifest.version },
    run: () => {
      calls++
      return 'ok'
    },
  }
  const result = await executeManifestService(
    manifest,
    'text-transform',
    'convert',
    implementation,
    { text: 'ok' },
    context(controller.signal),
    manifestTarget
  )
  expect(!result.success && result.error.code).toBe('ABORTED')
  expect(calls).toBe(0)
  let budget = 0
  const timeout = await executeManifestService(
    manifest,
    'text-transform',
    'convert',
    { ...implementation, run: () => new Promise(() => {}) },
    { text: 'ok' },
    context(),
    manifestTarget,
    {
      timeoutMs: 17,
      startTimeout: (callback, ms) => {
        budget = ms
        queueMicrotask(callback)
        return () => {}
      },
    }
  )
  expect(!timeout.success && timeout.error.code).toBe('TIMEOUT')
  expect(budget).toBe(17)
})
