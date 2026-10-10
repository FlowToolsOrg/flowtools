import { expect, test } from 'bun:test'

import { isJsonValue } from '../src/manifest/json-schema'

test('JSON arrays must be dense standard arrays with only indexed data', () => {
  expect(isJsonValue([null, false, 1, 'text', { nested: [] }])).toBe(true)
  const sparse: unknown[] = []
  sparse.length = 2
  Object.assign(sparse, { x: 'lost-x', y: 'lost-y' })
  const extra = ['normal']
  Object.assign(extra, { extra: 'lost' })
  let calls = 0
  class ExecutableArray extends Array<unknown> {
    toJSON() {
      calls++
      return []
    }
  }
  const custom = Object.setPrototypeOf([], { inherited: true }) as unknown
  for (const invalid of [sparse, extra, new ExecutableArray(), custom])
    expect(isJsonValue(invalid)).toBe(false)
  expect(calls).toBe(0)
})
