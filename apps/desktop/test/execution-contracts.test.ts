import { expect, test } from 'bun:test'

import base64 from '../../../plugins/plugin-base64-encoder/index.tsx'
import latency from '../../../plugins/plugin-website-latency/index.tsx'
import { runDesktopPlugin } from '../src/runtime/plugin-execution'

test('serialized command schema rejects undeclared fields and unavailable identities', async () => {
  expect(
    await runDesktopPlugin(base64, { text: 'hello', unknown: true })
  ).toMatchObject({ success: false, error: { code: 'INPUT_INVALID' } })
  expect(
    await runDesktopPlugin(
      { ...base64, meta: { ...base64.meta, id: 'plugin-unlisted-fixture' } },
      { text: 'hello' }
    )
  ).toMatchObject({ success: false, error: { code: 'NOT_RUNNABLE' } })
})

test('Desktop runs actual app entry through SDK schema and execution envelope', async () => {
  expect(await runDesktopPlugin(base64, { text: 'hello' })).toMatchObject({
    success: true,
    pluginId: base64.meta.id,
    pluginVersion: base64.meta.version,
    data: { type: 'json', value: { result: 'aGVsbG8=' } },
  })
  expect(await runDesktopPlugin(base64, { text: 3 })).toMatchObject({
    success: false,
    error: { code: 'INPUT_INVALID' },
  })
  const controller = new AbortController()
  controller.abort()
  expect(
    await runDesktopPlugin(
      base64,
      { text: 'hello' },
      { signal: controller.signal }
    )
  ).toMatchObject({ success: false, error: { code: 'ABORTED' } })
})

test('Desktop exposes an actual built-in capability exception as a failed envelope', async () => {
  const denied = { ...latency, meta: { ...latency.meta, permissions: [] } }
  expect(await runDesktopPlugin(denied, { urls: [] })).toMatchObject({
    success: false,
    error: {
      code: 'EXECUTION_FAILED',
      message: 'Network capability is unavailable',
    },
  })
})
