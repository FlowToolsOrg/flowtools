import type {
  Outcome,
  ProviderChangePlan,
  ServiceCallDiagnostic,
} from '../src/bindings'

import { expect, test } from 'bun:test'

import { RuntimeClient } from '../src/client'
import { encodeRequest } from '../src/codec'
import fixtures from '../src/wire-fixtures.json'

const root = '00000000-0000-4000-8000-000000000001'
const pin = fixtures.dependencyPlan.lock.packages[0]!
const plan: ProviderChangePlan = fixtures.providerUnloadPlan
const call: ServiceCallDiagnostic = {
  runId: '00000000-0000-4000-8000-000000000002',
  parentRunId: root,
  rootRunId: root,
  rootCaller: 'local-cli',
  consumer: { publisher: 'flowtools', id: 'consumer' },
  provider: pin,
  service: 'transform',
  operation: 'run',
  dependencyLock: fixtures.dependencyPlan.lock.digest,
  deadline: 10000,
  state: 'succeeded',
  failureCode: null,
}
function client(outcome: Outcome) {
  return new RuntimeClient({
    async exchange(request) {
      return { version: 1, requestId: request.requestId, outcome }
    },
    close() {},
  })
}
test('readonly provider plan and call diagnostics use distinct Host wire contracts', async () => {
  expect(
    await client({
      type: 'provider-change-plan',
      data: plan,
    }).providerUnloadPlan(pin.id)
  ).toEqual(plan)
  expect(
    await client({ type: 'service-calls', data: [call] }).serviceCalls(root)
  ).toEqual([call])
  for (const method of ['dependencies.unload-plan', 'services.calls']) {
    expect(() =>
      encodeRequest({
        ...fixtures.request,
        call: {
          method,
          payload: { pluginId: pin.id, runId: root, grant: true },
        },
      })
    ).toThrow('INVALID_REQUEST')
  }
})
test('unload inspection rejects activation, substituted identity, paths and duplicate metadata', async () => {
  for (const mutate of [
    (p: ProviderChangePlan) => {
      p.mode = 'apply'
    },
    (p: ProviderChangePlan) => {
      p.provider.id = 'substituted'
    },
    (p: ProviderChangePlan) => {
      p.provider.version = '900719925474100.0.0'
    },
    (p: ProviderChangePlan) => {
      p.provider.digest = 'path/private'
    },
    (p: ProviderChangePlan) => {
      p.replacement = { ...pin }
    },
  ]) {
    const copy = structuredClone(plan)
    mutate(copy)
    expect(
      client({ type: 'provider-change-plan', data: copy }).providerUnloadPlan(
        pin.id
      )
    ).rejects.toMatchObject({ code: 'INVALID_RESPONSE' })
  }
})
test('service diagnostics reject forged parent roots, invalid states and duplicate calls', async () => {
  for (const mutate of [
    (c: ServiceCallDiagnostic) => {
      c.rootRunId = '00000000-0000-4000-8000-000000000009'
    },
    (c: ServiceCallDiagnostic) => {
      c.parentRunId = '../private'
    },
    (c: ServiceCallDiagnostic) => {
      c.provider.publisher = 'other\npublisher'
    },
    (c: ServiceCallDiagnostic) => {
      c.state = 'grant-approved'
    },
    (c: ServiceCallDiagnostic) => {
      c.deadline = null
    },
  ]) {
    const copy = structuredClone(call)
    mutate(copy)
    expect(
      client({ type: 'service-calls', data: [copy] }).serviceCalls(root)
    ).rejects.toMatchObject({ code: 'INVALID_RESPONSE' })
  }
  expect(
    client({ type: 'service-calls', data: [call, call] }).serviceCalls(root)
  ).rejects.toMatchObject({ code: 'INVALID_RESPONSE' })
  expect(
    client({ type: 'service-calls', data: [] }).serviceCalls('../private')
  ).rejects.toMatchObject({ code: 'INVALID_REQUEST' })
})
