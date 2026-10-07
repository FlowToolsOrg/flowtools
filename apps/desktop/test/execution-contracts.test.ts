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

test('Desktop requires the authenticated native Host and validates before IPC', async () => {
  expect(await runDesktopPlugin(base64, { text: 'hello' })).toMatchObject({
    success: false,
    pluginId: base64.meta.id,
    pluginVersion: base64.meta.version,
    error: { code: 'CONTEXT_FAILED' },
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

test('caller metadata cannot bypass native Host grants', async () => {
  const denied = { ...latency, meta: { ...latency.meta, permissions: [] } }
  expect(await runDesktopPlugin(denied, { urls: [] })).toMatchObject({
    success: false,
    error: {
      code: 'CONTEXT_FAILED',
    },
  })
})
