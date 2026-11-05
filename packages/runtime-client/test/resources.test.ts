import { expect, test } from 'bun:test'

import { createResourceCapability } from '../src/resources'
async function rejection(promise: Promise<unknown>) {
  try {
    await promise
    return undefined
  } catch (error) {
    return error instanceof Error ? error.message : String(error)
  }
}
test('resource adapter enforces generated operation/response dialect and bounded chunk identity', async () => {
  const calls: unknown[] = []
  const resources = createResourceCapability(async operation => {
    calls.push(operation)
    return {
      kind: 'chunk',
      value: { base64: 'YWJj', nextOffset: 3, eof: true },
    }
  })
  expect(
    await resources.read('opaque', { revision: 'pin', offset: 0, maxBytes: 3 })
  ).toEqual({ bytes: new Uint8Array([97, 98, 99]), nextOffset: 3, eof: true })
  expect(calls).toEqual([
    {
      operation: 'read',
      parameters: { ref: 'opaque', revision: 'pin', offset: 0, maxBytes: 3 },
    },
  ])
  expect(
    await rejection(
      resources.read('opaque', { revision: 'pin', offset: -1, maxBytes: 3 })
    )
  ).toBe('INPUT_INVALID')
  for (const value of [
    { kind: 'chunk', value: { base64: 'YWJj', nextOffset: 4, eof: true } },
    {
      kind: 'chunk',
      value: { base64: 'YWJj', nextOffset: 3, eof: true, path: 'C:/private' },
    },
    { kind: 'chunk', value: { base64: 'YWJj', nextOffset: 3, eof: true } },
    { kind: 'chunk', value: { base64: '!!!', nextOffset: 0, eof: true } },
  ]) {
    const broken = createResourceCapability(async () => value)
    expect(
      await rejection(
        broken.read('opaque', { revision: 'pin', offset: 0, maxBytes: 2 })
      )
    ).toBeDefined()
  }
})
test('resource abort waits for actual Host completion and suppresses a late successful response', async () => {
  const controller = new AbortController()
  let complete: (value: unknown) => void = () => {
    throw new Error('NOT_STARTED')
  }
  let received: AbortSignal | undefined
  const resources = createResourceCapability((_, signal) => {
    received = signal
    return new Promise(resolve => {
      complete = resolve
    })
  })
  let settled = false
  const reading = resources
    .read('opaque', {
      revision: 'pin',
      offset: 0,
      maxBytes: 3,
      signal: controller.signal,
    })
    .finally(() => {
      settled = true
    })
  controller.abort()
  await Promise.resolve()
  expect(settled).toBe(false)
  expect(received).toBe(controller.signal)
  complete({
    kind: 'chunk',
    value: { base64: 'YWJj', nextOffset: 3, eof: true },
  })
  expect(await rejection(reading)).toBe('ABORTED')
  const unused = createResourceCapability(async () => {
    throw new Error('SHOULD_NOT_RUN')
  })
  expect(
    await rejection(unused.resolve('source', { signal: controller.signal }))
  ).toBe('ABORTED')
})
