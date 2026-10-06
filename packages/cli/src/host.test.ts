import { expect, test } from 'bun:test'

import { RuntimeClient, RuntimeClientError } from '@flowtools/runtime-client'

import { readControlStatus } from './host'

test('readiness disconnection reauthenticates the same instance and retries only status', async () => {
  const methods: string[] = []
  let fail = true
  let reconnects = 0
  const client = () =>
    new RuntimeClient({
      async exchange(request) {
        methods.push(request.call.method)
        if (request.call.method === 'session.open')
          return {
            version: 1,
            requestId: request.requestId,
            outcome: {
              type: 'session',
              data: { instanceId: 'original', sessionId: crypto.randomUUID() },
            },
          }
        if (fail) {
          fail = false
          throw new RuntimeClientError('RUNTIME_DISCONNECTED')
        }
        return {
          version: 1,
          requestId: request.requestId,
          outcome: {
            type: 'status',
            data: {
              instanceId: 'original',
              mode: 'managed',
              jobs: 0,
              activeJobs: 0,
            },
          },
        }
      },
      close() {},
    })
  const initial = client()
  await initial.connect('fixture')
  const restored = await readControlStatus(initial, async instance => {
    expect(instance).toBe('original')
    if (reconnects++ === 0) throw new RuntimeClientError('RUNTIME_DISCONNECTED')
    const replacement = client()
    await replacement.connect('fixture', instance)
    return replacement
  })
  expect(restored.type).toBe('status')
  expect(reconnects).toBe(2)
  expect(methods).toEqual([
    'session.open',
    'runtime.status',
    'session.open',
    'runtime.status',
  ])
})

test('readiness does not retry authorization or instance rejection', async () => {
  for (const code of [
    'INSTANCE_MISMATCH',
    'SESSION_INVALID',
    'RUNTIME_BUSY',
  ] as const) {
    let reconnects = 0
    const client = new RuntimeClient({
      async exchange() {
        throw new RuntimeClientError(code)
      },
      close() {},
    })
    const failure: unknown = await readControlStatus(client, async () => {
      reconnects++
      return client
    }).catch((error: unknown) => error)
    expect(failure).toMatchObject({ code })
    expect(reconnects).toBe(0)
  }
})
