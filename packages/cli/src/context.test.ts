import { expect, test } from 'bun:test'

import { createCLIToolContext } from './context'

test('CLI context exposes no undeclared capabilities', () => {
  const ctx = createCLIToolContext('context-fixture')
  expect(ctx.storage).toBeUndefined()
  expect(ctx.request).toBeUndefined()
  expect(ctx.signal.aborted).toBe(false)
})

test('CLI storage preserves SDK remove/zustand contract and rejects path keys', () => {
  const ctx = createCLIToolContext('context-fixture', {
    permissions: ['storage'],
  })
  const storage = ctx.storage!
  storage.set('regression', { value: 2 })
  expect(storage.get<{ value: number }>('regression')).toEqual({ value: 2 })
  storage.remove('regression')
  expect(storage.get('regression')).toBeUndefined()
  const state = storage.zustand('regression')
  state.setItem('fixture', 'state-value')
  expect(state.getItem('fixture')).toBe('state-value')
  state.removeItem('fixture')
  expect(state.getItem('fixture')).toBeNull()
  for (const key of ['../secret', 'C:\\secret', 'CON', 'x/y', 'x\\y']) {
    expect(() => storage.set(key, 'must-not-write')).toThrow()
  }
})

test('CLI rejects plugin identities that could escape its storage namespace', () => {
  expect(() => createCLIToolContext('../escape')).toThrow()
})
