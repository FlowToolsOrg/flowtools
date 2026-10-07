import { expect, test } from 'bun:test'

import {
  inspectLegacyTodos,
  hydratePluginData,
  type DataCapability,
} from '../src/data'

test('legacy inspection validates without changing raw sources or issuing an import', async () => {
  const raw = '[{"todo":"legacy","unrelated":true}]'
  const inspected = await inspectLegacyTodos(raw, 'cli-v0')
  expect(raw).toBe('[{"todo":"legacy","unrelated":true}]')
  expect(inspected.value).toEqual([
    { todo: 'legacy', unrelated: true, deadline: '' },
  ])
  expect(inspected.sourceDigest).toMatch(/^[a-f0-9]{64}$/)
  expect(
    (
      await inspectLegacyTodos(
        '{"todos":[{"todo":"legacy","deadline":"","unrelated":true}]}',
        'web-localstorage-v1'
      )
    ).sourceDigest
  ).toBe(inspected.sourceDigest)
  for (const invalid of [
    '{',
    '{}',
    '[{"todo":3}]',
    '[{"todo":"x","deadline":4}]',
  ])
    expect(inspectLegacyTodos(invalid, 'cli-v0')).rejects.toThrow()
})

test('hydration is asynchronous, skips stale revisions, and honors cancellation', async () => {
  const abort = new AbortController()
  const seen: number[] = []
  const data: DataCapability = {
    async read(key) {
      return { key, revision: 1, value: ['first'] }
    },
    async write() {
      throw new Error('Unexpected write')
    },
    async transaction() {
      throw new Error('Unexpected transaction')
    },
    async *watch(key) {
      yield { key, revision: 1, value: ['stale'] }
      yield { key, revision: 2, value: ['new'] }
      abort.abort()
      yield { key, revision: 3, value: ['cancelled'] }
    },
  }
  await hydratePluginData(
    data,
    'todos',
    snapshot => seen.push(snapshot.revision),
    abort.signal
  )
  expect(seen).toEqual([1, 2])
})
