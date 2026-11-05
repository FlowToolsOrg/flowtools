import { expect, test } from 'bun:test'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

import Ajv2020 from 'ajv/dist/2020'

import fixtures from '../src/wire-fixtures.json'
import schemas from '../src/wire-schema.json'
import {
  validateRequest,
  validateResponse,
  validateStorageAction,
  validateStorageReport,
} from '../src/wire-validators.cjs'

test('standalone validators preserve the original wire and numeric format decisions', () => {
  const ajv = new Ajv2020({ strict: false, allErrors: false })
  ajv.addFormat('double', { type: 'number', validate: Number.isFinite })
  ajv.addFormat('uint16', {
    type: 'number',
    validate: n => Number.isInteger(n) && n >= 0 && n <= 65535,
  })
  ajv.addFormat('uint32', {
    type: 'number',
    validate: n => Number.isInteger(n) && n >= 0 && n <= 4294967295,
  })
  ajv.addFormat('uint64', {
    type: 'number',
    validate: n => Number.isSafeInteger(n) && n >= 0,
  })
  const cases = [
    [
      schemas.request,
      validateRequest,
      [
        fixtures.request,
        {
          ...fixtures.request,
          call: {
            method: 'tools.inventory',
            payload: { offset: 0, snapshot: null },
          },
        },
        {
          ...fixtures.request,
          call: {
            method: 'schedules.create',
            payload: {
              requestKey: 'one',
              target: 'cli',
              pluginId: 'plugin-base64-encoder',
              commandId: 'run',
              input: { text: 'private' },
              timeoutMs: 1000,
              definition: {
                firstDueMs: 100000,
                intervalMs: 1000,
                occurrences: 3,
                graceMs: 500,
                missed: 'skip',
              },
            },
          },
        },
      ],
    ],
    [schemas.response, validateResponse, fixtures.responses],
    [
      schemas.storageAction,
      validateStorageAction,
      [
        { operation: 'list' },
        { operation: 'restore', parameters: { backupId: 'one' } },
      ],
    ],
    [
      schemas.storageReport,
      validateStorageReport,
      [
        {
          formatVersion: 1,
          backups: [
            {
              id: 'backup',
              bytes: 100,
              usable: true,
              schemaVersion: 1,
              errorCode: null,
            },
          ],
          recovery: null,
        },
      ],
    ],
  ] as const
  let comparisons = 0
  for (const [schema, compiled, positives] of cases) {
    const reference = ajv.compile(schema)
    for (const positive of positives) {
      expect(reference(positive)).toBe(true)
      expect(compiled(positive)).toBe(true)
      const walk = (value: unknown, path: string[]) => {
        for (const replacement of [
          null,
          true,
          'invalid',
          {},
          [],
          -1,
          0,
          0.5,
          65535,
          65536,
          4294967295,
          4294967296,
          Number.MAX_SAFE_INTEGER,
          Number.MAX_SAFE_INTEGER + 1,
          Infinity,
          NaN,
        ]) {
          let mutated: unknown = structuredClone(positive)
          if (!path.length) mutated = replacement
          else {
            let parent = mutated as Record<string, unknown>
            for (const key of path.slice(0, -1))
              parent = parent[key] as Record<string, unknown>
            parent[path[path.length - 1]!] = replacement
          }
          expect(compiled(mutated)).toBe(reference(mutated))
          comparisons++
        }
        if (value && typeof value === 'object')
          for (const [key, child] of Object.entries(value))
            walk(child, [...path, key])
      }
      walk(positive, [])
      expect(compiled({ ...positive, extraAuthority: true })).toBe(false)
    }
  }
  expect(comparisons).toBeGreaterThan(1000)
})

test('actual client artifacts validate requests, replies and recovery without dynamic code generation', () => {
  const entry = new URL('../dist/index.js', import.meta.url).href
  const result = spawnSync(
    'node',
    ['--disallow-code-generation-from-strings', '--input-type=module'],
    {
      input: `
      import assert from 'node:assert/strict';
      const { RuntimeClient, encodeRequest, encodeStorageAction, decodeStorageReport } = await import(${JSON.stringify(entry)});
      const request = {version:1, requestId:'csp', session:null, call:{method:'runtime.status'}};
      assert.equal(JSON.parse(encodeRequest(request)).requestId, 'csp');
      assert.throws(() => encodeRequest({...request, version:65536}), /INVALID_REQUEST/);
      assert.equal(encodeStorageAction({operation:'list'}), '{"operation":"list"}');
      assert.throws(() => encodeStorageAction({operation:'run'}), /INVALID_REQUEST/);
      assert.equal(decodeStorageReport({formatVersion:1, backups:[], recovery:null}).backups.length, 0);
      assert.throws(() => decodeStorageReport({formatVersion:1, backups:[], recovery:null, token:'private'}), /INVALID_RESPONSE/);
      const client = new RuntimeClient({close(){}, async exchange(request){
        return {version:1, requestId:request.requestId, outcome:{type:'error',data:{code:'INVALID_REQUEST'}}};
      }});
      await assert.rejects(client.call({method:'runtime.status'}), error => error.code === 'INVALID_REQUEST');
      console.log('CSP_VALIDATORS_OK');
    `,
      encoding: 'utf8',
      timeout: 5000,
    }
  )
  expect(result.error).toBeUndefined()
  expect(result.status, result.stderr).toBe(0)
  expect(result.stdout.trim()).toBe('CSP_VALIDATORS_OK')
})

test('generated validators exactly match the current Rust schema in read-only mode', () => {
  const script = fileURLToPath(
    new URL('../generate-validators.ts', import.meta.url)
  )
  const result = spawnSync(process.execPath, ['run', script, '--check'], {
    encoding: 'utf8',
    timeout: 5000,
  })
  expect(result.error).toBeUndefined()
  expect(result.status, result.stderr).toBe(0)
})
