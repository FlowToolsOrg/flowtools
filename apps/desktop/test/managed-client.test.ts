import { expect, test } from 'bun:test'

import { RuntimeClient, type Outcome } from '@flowtools/runtime-client'

import { managedPluginData } from '../src/runtime/managed-client'

test('shared Todo loses a write ACK without retry and explicit reread reconnects', async () => {
  let connections = 0
  let writes = 0
  let closed = 0
  let value: unknown = null
  const connect = async () => {
    connections++
    const client = new RuntimeClient({
      async exchange(request) {
        let outcome: Outcome
        if (request.call.method === 'session.open')
          outcome = {
            type: 'session',
            data: {
              sessionId: 'native-' + connections,
              instanceId: 'same-host',
            },
          }
        else if (request.call.method === 'data.write') {
          writes++
          value = request.call.payload.mutation.value
          throw new Error('ACK lost after CAS commit')
        } else
          outcome = {
            type: 'data',
            data: { key: 'todos', revision: writes, value },
          }
        return { version: 1, requestId: request.requestId, outcome }
      },
      close() {
        closed++
      },
    })
    await client.connect('controlled transport')
    return client
  }
  const facade = managedPluginData('plugin-todo-list', connect)
  expect((await facade.data.read('todos')).revision).toBe(0)
  const failure = await facade.data
    .write({
      key: 'todos',
      expectedRevision: 0,
      value: [{ todo: 'one effect', deadline: '' }],
    })
    .catch((error: unknown) => error)
  expect(failure).toMatchObject({ code: 'RUNTIME_DISCONNECTED' })
  expect(writes).toBe(1)
  const actual = await facade.data.read('todos')
  expect(actual.revision).toBe(1)
  expect(actual.value).toEqual([{ todo: 'one effect', deadline: '' }])
  expect(connections).toBe(2)
  facade.close()
  await facade.data.read('todos')
  expect(connections).toBe(3)
  expect(writes).toBe(1)
  expect(closed).toBe(2)
  facade.close()
})
