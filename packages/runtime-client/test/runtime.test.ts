import { expect, test } from 'bun:test'

import { encodeRequest } from '../src/codec'

test('actual Node clients and native Host share real T1 tasks and cancellation', async () => {
  // Bun's Windows net.Socket missed a response after switching pipes. Exercise the
  // supported Node transport in a real Node process, with every assertion intact.
  const fixture = Bun.spawn(['node', import.meta.dir + '/native-fixture.mjs'], {
    stdout: 'pipe',
    stderr: 'pipe',
  })
  const [stdout, stderr, code] = await Promise.all([
    new Response(fixture.stdout).text(),
    new Response(fixture.stderr).text(),
    fixture.exited,
  ])
  expect({ code, stderr }).toEqual({ code: 0, stderr: '' })
  expect(stdout.trim()).toBe('Native fixture passed')
}, 30000)

test('wire schema rejects identity injection and non-finite JSON before transport', () => {
  const request = {
    version: 1,
    requestId: 'test',
    session: null,
    call: {
      method: 'jobs.submit' as const,
      payload: {
        pluginId: 'plugin-base64-encoder',
        commandId: 'run',
        input: { text: 'hello' },
        idempotencyKey: 'test',
        background: false,
        deadline: Date.now() + 1000,
        agentId: 'admin',
      },
    },
  }
  expect(() => encodeRequest(request)).toThrow('INVALID_REQUEST')
  delete (request.call.payload as { agentId?: string }).agentId
  request.call.payload.deadline = Number.POSITIVE_INFINITY
  expect(() => encodeRequest(request)).toThrow('INVALID_REQUEST')
})

test('actual GUI and CLI Host clients share durable revisions and reject unauthorized data', async () => {
  const fixture = Bun.spawn(['node', import.meta.dir + '/data-fixture.mjs'], {
    stdout: 'pipe',
    stderr: 'pipe',
  })
  const [stdout, stderr, code] = await Promise.all([
    new Response(fixture.stdout).text(),
    new Response(fixture.stderr).text(),
    fixture.exited,
  ])
  expect({ code, stderr }).toEqual({ code: 0, stderr: '' })
  expect(stdout.trim()).toBe('Shared data fixture passed')
}, 30000)
