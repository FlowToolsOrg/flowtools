import { expect, test } from 'bun:test'

import base64 from '@flowtools/plugins/plugin-base64-encoder'

import { runWebPlugin } from '../runtime/plugin-runtime'

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
