import type {
  ToolContext,
  StorageCapability,
  PluginStoreCapability,
} from '@flowtools/sdk/types'

import { expect, spyOn, test } from 'bun:test'

import { executePlugin } from '@flowtools/sdk/execution'

import todo from '../plugin-todo-list/index.tsx'
import latency from '../plugin-website-latency/index.tsx'

function context(id: string): ToolContext {
  return {
    env: { pluginId: id, pluginType: 'app', platform: 'unknown', mode: 'test' },
    signal: new AbortController().signal,
    ui: { toast: () => {}, openPanel: () => {}, closePanel: () => {} },
    utils: { now: Date.now },
    log: () => {},
  }
}

test('website latency requires the SDK capability and never bypasses it with raw fetch', async () => {
  const rawFetch = spyOn(globalThis, 'fetch').mockImplementation(
    Object.assign(
      () => Promise.reject(new Error('Raw fetch is forbidden in this fixture')),
      { preconnect: () => {} }
    )
  )
  try {
    expect(
      await executePlugin(latency, { urls: [] }, context(latency.meta.id))
    ).toMatchObject({ success: false, error: { code: 'EXECUTION_FAILED' } })
    const calls: string[] = []
    const ctx = {
      ...context(latency.meta.id),
      request: async (input: RequestInfo | URL) => {
        calls.push(
          typeof input === 'string'
            ? input
            : input instanceof URL
              ? input.href
              : input.url
        )
        return new Response('fixture response')
      },
    }
    const result = await executePlugin(
      latency,
      { urls: ['https://latency.fixture.invalid/'] },
      ctx
    )
    expect(result).toMatchObject({
      success: true,
      data: { type: 'json', value: { tested: 1 } },
    })
    expect(calls).toEqual(['https://latency.fixture.invalid/'])
    expect(rawFetch).not.toHaveBeenCalled()
  } finally {
    rawFetch.mockRestore()
  }
})

test('Todo requires real storage or an app store instead of claiming an unsaved addition', async () => {
  expect(
    await executePlugin(
      todo,
      { todo: 'must-not-pretend-saved' },
      context(todo.meta.id)
    )
  ).toMatchObject({ success: false, error: { code: 'EXECUTION_FAILED' } })
})

test('Todo JSON runs update the same app store used by its panel', async () => {
  let state = { todos: [{ todo: 'existing UI item', deadline: '' }] }
  const store: PluginStoreCapability<typeof state> = {
    getState: () => state,
    setState: updater => {
      state = {
        ...state,
        ...(typeof updater === 'function' ? updater(state) : updater),
      }
    },
    subscribe: () => () => {},
    reset: () => {
      state = { todos: [] }
    },
    actions: {},
  }
  const ctx = { ...context(todo.meta.id), store }
  expect(
    await executePlugin(todo, { todo: 'new JSON item' }, ctx)
  ).toMatchObject({ success: true, data: { value: { result: { total: 2 } } } })
  expect(state.todos.map(item => item.todo)).toEqual([
    'existing UI item',
    'new JSON item',
  ])
  expect(await executePlugin(todo, {}, ctx)).toMatchObject({
    success: true,
    data: { value: { count: 2 } },
  })
})

test('Todo validates legacy storage before writing and preserves corrupt data for recovery', async () => {
  let raw: unknown = [{ todo: 'legacy item', deadline: '' }]
  const storage: StorageCapability = {
    get: <T>() => raw as T,
    set: (_key, value) => {
      raw = value
    },
    remove: () => {},
    zustand: () => ({
      getItem: () => null,
      setItem: () => {},
      removeItem: () => {},
    }),
  }
  const ctx = { ...context(todo.meta.id), storage }
  expect(await executePlugin(todo, { todo: 'second item' }, ctx)).toMatchObject(
    { success: true, data: { value: { result: { total: 2 } } } }
  )
  raw = [null]
  expect(await executePlugin(todo, { todo: 'third item' }, ctx)).toMatchObject({
    success: false,
    error: { code: 'EXECUTION_FAILED' },
  })
  expect(raw).toEqual([null])
})
