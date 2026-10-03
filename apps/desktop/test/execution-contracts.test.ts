import { expect, test } from 'bun:test'

import base64 from '../../../plugins/plugin-base64-encoder/index.tsx'
import { runDesktopPlugin } from '../src/runtime/plugin-execution'

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
