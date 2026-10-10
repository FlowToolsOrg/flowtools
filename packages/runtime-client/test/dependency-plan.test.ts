import type { DependencyPlan, Request } from '../src/bindings'

import { expect, test } from 'bun:test'

import { RuntimeClient, RuntimeClientError } from '../src/client'
import { decodeResponse, encodeRequest } from '../src/codec'
import { describeRuntimeError } from '../src/diagnostics'
import fixtures from '../src/wire-fixtures.json'

const golden = fixtures.dependencyPlan
const roots = golden.lock.roots.map(root => root.id)

test('dependency plan uses Rust golden lock and only sends fixed root IDs', async () => {
  let request: Request | undefined
  const client = new RuntimeClient({
    async exchange(value) {
      request = value
      return {
        version: 1,
        requestId: value.requestId,
        outcome: { type: 'dependency-plan', data: structuredClone(golden) },
      }
    },
    close() {},
  })
  expect(await client.dependenciesPlan(roots)).toEqual(golden)
  expect(request?.call).toEqual({
    method: 'dependencies.plan',
    payload: { pluginIds: roots },
  })
  expect(golden.mode).toBe('plan-only')
  expect(golden.lock.roots).toEqual(
    golden.lock.packages.map(({ publisher, id }) => ({ publisher, id }))
  )
})

test('dependency plan snapshots roots before awaiting transport', async () => {
  const mutable = [...roots]
  const client = new RuntimeClient({
    async exchange(request) {
      mutable[0] = 'different-plugin'
      // A transport cannot change which roots the helper validates either.
      if (request.call.method === 'dependencies.plan')
        request.call.payload.pluginIds[0] = 'transport-changed-plugin'
      return {
        version: 1,
        requestId: request.requestId,
        outcome: { type: 'dependency-plan', data: structuredClone(golden) },
      }
    },
    close() {},
  })
  expect(await client.dependenciesPlan(mutable)).toEqual(golden)
})

test('dependency request schema rejects caller catalogs, targets, paths and grants', () => {
  for (const extra of [
    { catalog: [] },
    { publisher: 'claimed-publisher' },
    { target: { platform: 'windows', arch: 'x64' } },
    { path: 'C:/private/tool.exe' },
    { argv: ['--unsafe'] },
    { grant: { approved: true } },
    { mode: 'install' },
  ])
    expect(() =>
      encodeRequest({
        ...fixtures.request,
        call: {
          method: 'dependencies.plan',
          payload: { pluginIds: roots, ...extra },
        },
      })
    ).toThrow('INVALID_REQUEST')
})

test('invalid, duplicate or oversize root requests fail with typed codes before IO', async () => {
  let calls = 0
  const client = new RuntimeClient({
    async exchange(request) {
      calls++
      return {
        version: 1,
        requestId: request.requestId,
        outcome: { type: 'dependency-plan', data: structuredClone(golden) },
      }
    },
    close() {},
  })
  for (const pluginIds of [
    [],
    [''],
    ['bad/path'],
    ['bad\n--install'],
    [...roots, ...roots],
  ])
    expect(await refusal(client.dependenciesPlan(pluginIds))).toMatchObject({
      code: 'DEPENDENCY_INVALID',
      acceptanceUnknown: false,
    })
  expect(
    await refusal(
      client.dependenciesPlan(
        Array.from({ length: 65 }, (_, index) => `plugin-${index}`)
      )
    )
  ).toMatchObject({
    code: 'DEPENDENCY_BUDGET_EXCEEDED',
    acceptanceUnknown: false,
  })
  expect(calls).toBe(0)
})

test('dependency plan rejects incompatible, injected or mismatched Host responses', async () => {
  const mutations: ((plan: DependencyPlan) => void)[] = [
    plan => {
      plan.formatVersion = 2
    },
    plan => {
      plan.mode = 'activate'
    },
    plan => {
      plan.lock.formatVersion = 2
    },
    plan => {
      plan.lock.digest = 'invalid-digest'
    },
    plan => {
      plan.lock.target.platform = 'windows\nrun tool.exe'
    },
    plan => {
      plan.lock.target.arch = 'unsupported'
    },
    plan => {
      plan.lock.roots = []
    },
    plan => {
      plan.lock.roots[0].id = 'another-plugin'
    },
    plan => {
      plan.lock.roots[0].publisher = 'another-publisher'
    },
    plan => {
      plan.lock.roots.push({ ...plan.lock.roots[0] })
    },
    plan => {
      plan.lock.packages[0].publisher = 'publisher\n--grant'
    },
    plan => {
      plan.lock.packages[0].version = '1.0.0\n--install'
    },
    plan => {
      plan.lock.packages[0].version = '900719925474100.0.0'
    },
    plan => {
      plan.lock.packages[0].version = '1.0.0-900719925474100'
    },
    plan => {
      plan.lock.packages[0].digest = 'A'.repeat(64)
    },
    plan => {
      plan.lock.packages.push({ ...plan.lock.packages[0] })
    },
    plan => {
      plan.lock.topology = []
    },
    plan => {
      plan.lock.topology[0].publisher = 'other-publisher'
    },
    plan => {
      plan.lock.reverseDependencies[0].consumers.push({
        publisher: 'unknown',
        id: 'provider',
      })
    },
    plan => {
      plan.lock.services.push({
        consumer: plan.lock.roots[0],
        provider: plan.lock.packages[0],
        service: 'service\n--execute',
        version: '1.0.0',
      })
    },
    plan => {
      plan.lock.services.push({
        consumer: plan.lock.roots[0],
        provider: { ...plan.lock.packages[0], digest: '0'.repeat(64) },
        service: 'service',
        version: '1.0.0',
      })
    },
    plan => {
      plan.lock.tools.push({
        consumer: plan.lock.roots[0],
        ...plan.lock.packages[0],
        target: plan.lock.target,
        buildFlavor: 'release\n--install',
      })
    },
    plan => {
      plan.lock.tools.push({
        consumer: plan.lock.roots[0],
        ...plan.lock.packages[0],
        target: { ...plan.lock.target, arch: 'wasm32' },
        buildFlavor: 'release',
      })
    },
    plan => {
      plan.lock.edges.push({
        consumer: plan.lock.roots[0],
        provider: { publisher: 'unknown', id: 'provider' },
        service: 'service',
      })
    },
    plan => {
      Object.assign(plan.lock, { actions: [{ type: 'execute' }] })
    },
  ]
  for (const mutate of mutations) {
    const plan: DependencyPlan = structuredClone(golden)
    mutate(plan)
    const client = responseClient(plan)
    expect(await refusal(client.dependenciesPlan(roots))).toMatchObject({
      code: 'INVALID_RESPONSE',
      acceptanceUnknown: false,
    })
  }
})

test('dependency response collection bounds are enforced before display', async () => {
  const mutations: ((plan: DependencyPlan) => void)[] = [
    plan => {
      plan.lock.roots = Array.from({ length: 65 }, () => plan.lock.roots[0])
    },
    plan => {
      plan.lock.packages = Array.from(
        { length: 129 },
        () => plan.lock.packages[0]
      )
    },
    plan => {
      plan.lock.topology = Array.from(
        { length: 129 },
        () => plan.lock.topology[0]
      )
    },
    plan => {
      plan.lock.reverseDependencies = Array.from(
        { length: 129 },
        () => plan.lock.reverseDependencies[0]
      )
    },
  ]
  for (const mutate of mutations) {
    const plan: DependencyPlan = structuredClone(golden)
    mutate(plan)
    expect(
      await refusal(responseClient(plan).dependenciesPlan(roots))
    ).toMatchObject({
      code: 'INVALID_RESPONSE',
      acceptanceUnknown: false,
    })
  }
})

test('Host dependency failures retain typed codes and never imply job acceptance', async () => {
  const failures = fixtures.responses
    .map(decodeResponse)
    .flatMap(response =>
      response.outcome.type === 'error' &&
      response.outcome.data.code.startsWith('DEPENDENCY_')
        ? [response.outcome.data.code]
        : []
    )
  expect(failures).toHaveLength(8)
  for (const code of failures) {
    let calls = 0
    const client = new RuntimeClient({
      async exchange(request) {
        calls++
        return {
          version: 1,
          requestId: request.requestId,
          outcome: { type: 'error', data: { code } },
        }
      },
      close() {},
    })
    const error = await refusal(client.dependenciesPlan(roots))
    expect(error.code).toBe(code)
    expect(describeRuntimeError(error)).toMatchObject({
      code,
      acceptanceUnknown: false,
    })
    expect(calls).toBe(1)
  }
})

test('dependency disconnection never reports unknown execution acceptance or retries', async () => {
  let calls = 0
  const client = new RuntimeClient({
    async exchange() {
      calls++
      throw new Error('private-token path-canary')
    },
    close() {},
  })
  const error = await refusal(client.dependenciesPlan(roots))
  expect(error).toMatchObject({
    code: 'RUNTIME_DISCONNECTED',
    acceptanceUnknown: false,
  })
  expect(JSON.stringify(describeRuntimeError(error))).not.toContain('canary')
  expect(calls).toBe(1)
})

function responseClient(plan: DependencyPlan) {
  return new RuntimeClient({
    async exchange(request) {
      return {
        version: 1,
        requestId: request.requestId,
        outcome: { type: 'dependency-plan', data: plan },
      }
    },
    close() {},
  })
}

async function refusal(
  operation: Promise<unknown>
): Promise<RuntimeClientError> {
  try {
    await operation
  } catch (error) {
    if (error instanceof RuntimeClientError) return error
    throw error
  }
  throw new Error('Expected controlled rejection')
}
