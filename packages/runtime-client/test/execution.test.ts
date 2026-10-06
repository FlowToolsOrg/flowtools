import type { JobSnapshot, Outcome, SubmitJob } from '../src/bindings'

import { expect, test } from 'bun:test'

import { RuntimeClient } from '../src/client'
import { executeRuntimeJob, RuntimeExecutionError } from '../src/execution'

const operation: SubmitJob = {
  pluginId: 'plugin-base64-encoder',
  commandId: 'run',
  input: { text: 'private-fixture' },
  idempotencyKey: 'one-original-key',
  background: true,
  deadline: Date.now() + 30000,
}
const terminal: JobSnapshot = {
  formatVersion: 1,
  runId: 'one-run',
  parentRunId: null,
  rootCaller: 'local-cli',
  pluginId: operation.pluginId,
  commandId: 'run',
  packageVersion: '0.0.1',
  packageDigest: 'a'.repeat(64),
  dependencyLock: 't1-no-dependencies-v1',
  deadline: operation.deadline,
  grantEpoch: 1,
  resources: {},
  state: 'succeeded',
  sequence: 3,
  result: {
    formatVersion: 1,
    runId: 'one-run',
    pluginId: operation.pluginId,
    pluginVersion: '0.0.1',
    startedAt: 1,
    finishedAt: 2,
    durationMs: 1,
    inputSummary: { kind: 'object', size: 1 },
    success: true,
    data: { type: 'json', value: { result: 'fixture' } },
  },
}

test('lost acknowledgement queries the original key and never submits a second effect', async () => {
  const methods: string[] = []
  let connections = 0
  const connect = async () => {
    const number = ++connections
    const client = new RuntimeClient({
      async exchange(request) {
        methods.push(request.call.method)
        let outcome: Outcome
        if (request.call.method === 'session.open')
          outcome = {
            type: 'session',
            data: { sessionId: 'session-' + number, instanceId: 'instance' },
          }
        else if (request.call.method === 'jobs.submit')
          throw new Error('lost after durable acceptance')
        else if (request.call.method === 'jobs.lookup') {
          expect(request.call.payload.idempotencyKey).toBe(
            operation.idempotencyKey
          )
          outcome = {
            type: 'receipt',
            data: {
              formatVersion: 1,
              receiptType: 'job',
              runId: 'one-run',
              instanceId: 'instance',
              acceptedAt: 1,
            },
          }
        } else outcome = { type: 'job', data: terminal }
        return { version: 1, requestId: request.requestId, outcome }
      },
      close() {},
    })
    await client.connect('token')
    return client
  }
  const result = await executeRuntimeJob(connect, operation)
  expect(result.success).toBe(true)
  expect(methods.filter(method => method === 'jobs.submit')).toHaveLength(1)
  expect(connections).toBe(2)
})

test('failed lost-ACK lookup retains the original key and requires review', async () => {
  let submits = 0
  let connections = 0
  const connect = async () => {
    const number = ++connections
    const client = new RuntimeClient({
      async exchange(request) {
        if (request.call.method === 'session.open')
          return {
            version: 1,
            requestId: request.requestId,
            outcome: {
              type: 'session',
              data: { sessionId: 'session', instanceId: 'instance' },
            },
          }
        if (request.call.method === 'jobs.submit') {
          submits++
          throw new Error('lost')
        }
        if (number > 1)
          return {
            version: 1,
            requestId: request.requestId,
            outcome: { type: 'error', data: { code: 'JOB_NOT_FOUND' } },
          }
        throw new Error('unexpected')
      },
      close() {},
    })
    await client.connect('token')
    return client
  }
  const error = await executeRuntimeJob(connect, operation).catch(
    (error: unknown) => error
  )
  expect(error).toBeInstanceOf(RuntimeExecutionError)
  expect(error).toMatchObject({
    code: 'ACCEPTANCE_UNKNOWN',
    idempotencyKey: operation.idempotencyKey,
    acceptanceUnknown: true,
  })
  expect(submits).toBe(1)
})
