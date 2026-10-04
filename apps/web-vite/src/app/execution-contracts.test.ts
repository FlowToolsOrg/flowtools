import { expect, test } from 'bun:test'

import base64 from '@flowtools/plugins/plugin-base64-encoder'
import latency from '@flowtools/plugins/plugin-website-latency'

import { runWebPlugin } from '../runtime/plugin-runtime'

test('serialized command schema rejects undeclared fields and unavailable identities', async () => {
  expect(
    await runWebPlugin(base64, { text: 'hello', unknown: true })
  ).toMatchObject({ success: false, error: { code: 'INPUT_INVALID' } })
  expect(
    await runWebPlugin(
      { ...base64, meta: { ...base64.meta, id: 'plugin-unlisted-fixture' } },
      { text: 'hello' }
    )
  ).toMatchObject({ success: false, error: { code: 'NOT_RUNNABLE' } })
})

test('Web runs actual app entry through SDK schema and execution envelope', async () => {
  expect(await runWebPlugin(base64, { text: 'hello' })).toMatchObject({
    success: true,
    pluginId: base64.meta.id,
    pluginVersion: base64.meta.version,
    data: { type: 'json', value: { result: 'aGVsbG8=' } },
  })
  expect(await runWebPlugin(base64, { text: 3 })).toMatchObject({
    success: false,
    error: { code: 'INPUT_INVALID' },
  })
  const controller = new AbortController()
  controller.abort()
  expect(
    await runWebPlugin(base64, { text: 'hello' }, { signal: controller.signal })
  ).toMatchObject({ success: false, error: { code: 'ABORTED' } })
})

test('Web exposes an actual built-in capability exception as a failed envelope', async () => {
  const denied = { ...latency, meta: { ...latency.meta, permissions: [] } }
  expect(await runWebPlugin(denied, { urls: [] })).toMatchObject({
    success: false,
    error: {
      code: 'EXECUTION_FAILED',
      message: 'Network capability is unavailable',
    },
  })
})
