import { expect, test } from 'bun:test'
import { createHash } from 'node:crypto'

import { parseManifestCatalog } from '@flowtools/sdk/manifest'

import { builtInCLIManifests } from '../../cli/src/builtin-manifests'
import { RuntimeClient, RuntimeClientError } from '../src/client'
import { decodeResponse, encodeRequest } from '../src/codec'
import { describeRuntimeError } from '../src/diagnostics'
import fixtures from '../src/wire-fixtures.json'
import schemas from '../src/wire-schema.json'

test('Rust-derived golden protocol and full metadata match TS validators and digests', () => {
  expect(() => encodeRequest(fixtures.request)).not.toThrow()
  const codes = fixtures.responses.map(response => {
    const parsed = decodeResponse(response)
    if (parsed.outcome.type !== 'error') throw new Error('Expected rejection')
    const report = describeRuntimeError(
      new RuntimeClientError(parsed.outcome.data.code)
    )
    expect(report.code).toBe(parsed.outcome.data.code)
    expect(report.action.length).toBeGreaterThan(8)
    return report.code
  })
  expect([...new Set(codes)].map(String).sort()).toEqual(
    [...schemas.response.$defs.ErrorCode.enum].sort()
  )
  expect(
    parseManifestCatalog(
      {
        formatVersion: 1,
        plugins: fixtures.manifests.map(item => item.manifest),
      },
      {
        hostVersion: '0.1.0',
        sdkVersion: '0.0.0',
        platform: 'windows',
        arch: 'x64',
      },
      builtInCLIManifests.map(plugin => plugin.id)
    )
  ).toHaveLength(12)
  const canonical = (value: unknown): unknown =>
    Array.isArray(value)
      ? value.map(canonical)
      : value && typeof value === 'object'
        ? Object.fromEntries(
            Object.entries(value)
              .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
              .map(([key, item]) => [key, canonical(item)])
          )
        : value
  for (const item of fixtures.manifests) {
    expect(
      createHash('sha256')
        .update(JSON.stringify(canonical(item.manifest)))
        .digest('hex')
    ).toBe(item.digest)
    expect(item.manifest.files.map(file => file.path).sort()).toEqual(
      [...new Set(Object.values(item.manifest.entries))].sort()
    )
    for (const file of item.manifest.files) {
      const text =
        `// Controlled G2 contract fixture: ${file.path}\r\n`.replace(
          /\r\n/g,
          '\n'
        )
      expect(file.size).toBe(Buffer.byteLength(text))
      expect(file.sha256).toBe(createHash('sha256').update(text).digest('hex'))
    }
  }
})

test('diagnostic exports never include exception, token, path, input or output canaries', () => {
  const report = describeRuntimeError(
    new Error(
      'token-canary input-canary output-canary C:\\private\\path-canary'
    )
  )
  const value = JSON.stringify(report)
  expect(Object.keys(report).sort()).toEqual([
    'acceptanceUnknown',
    'action',
    'code',
    'formatVersion',
    'summary',
  ])
  for (const privateValue of [
    'token-canary',
    'input-canary',
    'output-canary',
    'path-canary',
  ])
    expect(value.includes(privateValue)).toBe(false)
})

test('oversize requests refuse before IO; disconnection has unknown acceptance and never retries', async () => {
  let calls = 0
  const client = new RuntimeClient({
    async exchange() {
      calls++
      throw new Error('private-token')
    },
    close() {},
  })
  const original = {
    ...fixtures.request.call.payload,
    deadline: Date.now() + 10000,
  }
  expect(
    await refusal(
      client.submit({ ...original, input: { text: 'x'.repeat(1_048_576) } })
    )
  ).toMatchObject({ code: 'FRAME_TOO_LARGE', acceptanceUnknown: false })
  expect(calls).toBe(0)
  expect(await refusal(client.submit(original))).toMatchObject({
    code: 'RUNTIME_DISCONNECTED',
    acceptanceUnknown: true,
  })
  expect(calls).toBe(1)
})

test('request-id or method mismatch rejects, and transport rejection codes survive', async () => {
  const client = new RuntimeClient({
    async exchange(request) {
      return {
        version: 1,
        requestId: request.requestId + 'wrong',
        outcome: { type: 'error', data: { code: 'SESSION_INVALID' } },
      }
    },
    close() {},
  })
  expect(
    await refusal(client.call({ method: 'runtime.status' }))
  ).toMatchObject({ code: 'INVALID_RESPONSE' })
  for (const [version, type, expected] of [
    [2, 'status', 'PROTOCOL_MISMATCH'],
    [1, 'plugins', 'INVALID_RESPONSE'],
  ] as const) {
    const mismatch = new RuntimeClient({
      async exchange(request) {
        return {
          version,
          requestId: request.requestId,
          outcome:
            type === 'plugins'
              ? { type, data: [] }
              : {
                  type,
                  data: { instanceId: 'fixture', mode: 'validation', jobs: 0 },
                },
        }
      },
      close() {},
    })
    expect(
      await refusal(mismatch.call({ method: 'runtime.status' }))
    ).toMatchObject({ code: expected })
  }
  const denied = new RuntimeClient({
    async exchange() {
      throw new RuntimeClientError('SESSION_INVALID')
    },
    close() {},
  })
  expect(
    await refusal(denied.call({ method: 'runtime.status' }))
  ).toMatchObject({ code: 'SESSION_INVALID' })
  expect(() =>
    decodeResponse({ ...fixtures.responses[0], privatePayload: 'canary' })
  ).toThrow('INVALID_RESPONSE')
})

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
