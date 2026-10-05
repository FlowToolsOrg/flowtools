import { expect, test } from 'bun:test'

import { loadPlugin } from '../../cli/src/discovery'
import { runValidationCommand, manifestDigest } from '../src/runner'

const request = async (pluginId: string, input: unknown) => ({
  pluginId,
  commandId: 'run',
  input,
  packageDigest: manifestDigest((await loadPlugin(pluginId))?.manifest ?? null),
})

test('fixed T1 runner invokes actual Base64 and denies side effects', async () => {
  expect(
    await runValidationCommand(
      await request('plugin-base64-encoder', { text: 'hello' })
    )
  ).toMatchObject({ success: true, data: { value: { result: 'aGVsbG8=' } } })
  expect(
    await runValidationCommand(
      await request('plugin-base64-encoder', { text: 3 })
    )
  ).toMatchObject({ success: false, error: { code: 'INPUT_INVALID' } })
  expect(
    await runValidationCommand(
      await request('plugin-todo-list', { todo: 'private' })
    )
  ).toMatchObject({ success: false, error: { code: 'NOT_RUNNABLE' } })
  expect(
    await runValidationCommand(await request('../escape', {}))
  ).toMatchObject({ success: false, error: { code: 'LOAD_FAILED' } })
  expect(
    await runValidationCommand({
      ...(await request('plugin-base64-encoder', { text: 'hello' })),
      packageDigest: 'spoof',
    }).then(
      () => false,
      error => error instanceof Error && error.message === 'Package changed'
    )
  ).toBe(true)
})
