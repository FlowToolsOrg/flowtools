import { expect, test } from 'bun:test'

import { CommandRegistry } from '../src/registry/command-registry'
import { CommandSearchIndex, SearchError } from '../src/search'

const index = () => {
  const search = new CommandSearchIndex()
  search.upsert({
    id: 'reminder',
    pluginId: 'reminder',
    title: '本地提醒',
    aliases: ['闹钟', 'alarm'],
    keywords: ['schedule'],
    description: '定时处理办公待办',
  })
  search.upsert({ id: 'json', pluginId: 'formatter', title: 'JSON Formatter' })
  return search
}
test('pinyin, initials, aliases, keyword and fuzzy matches explain their source', () => {
  const search = index()
  for (const [query, reason] of [
    ['bendi', 'pinyin'],
    ['bdtx', 'initials'],
    ['alarm', 'alias'],
    ['schedule', 'keyword'],
    ['JSN', 'fuzzy'],
    ['本地提醒', 'exact'],
  ] as const) {
    expect(search.search(query)[0]?.reasons).toContain(reason)
  }
  expect(search.search('office missing')).toEqual([])
  expect(search.search('ＪＳＯＮ')[0]?.id).toBe('json')
})
test('regex is explicit, bounded and refuses unsupported or huge compilation', () => {
  const search = index()
  expect(search.search('^json', { mode: 'regex' })[0]?.id).toBe('json')
  expect(search.search('^json')).toEqual([])
  for (const pattern of ['[', '(a)\\1', 'a{999999999}', 'a'.repeat(129)])
    expect(() => search.search(pattern, { mode: 'regex' })).toThrow(SearchError)
  expect(search.search('(a+)+$', { mode: 'regex' })).toEqual([])
})

test('fuzzy matching keeps supplementary characters whole and in order', () => {
  const search = new CommandSearchIndex()
  search.upsert({ id: 'astral', pluginId: 'p', title: '😀 notes 🚀 launch' })
  expect(search.search('😀🚀')).toEqual([
    { id: 'astral', score: 200, reasons: ['fuzzy'] },
  ])
  expect(search.search('🚀😀')).toEqual([])
  expect(search.search('😁🚀')).toEqual([])
  expect(search.search('\ud83d🚀')).toEqual([])
})
test('metadata updates and disable/removal remove stale hits immediately', () => {
  const registry = new CommandRegistry()
  registry.register({
    id: 'p:run',
    pluginId: 'p',
    title: '本地提醒',
    keywords: [],
    mode: 'headless',
    handler() {},
  })
  expect(registry.search('bdtx').map(item => item.id)).toEqual(['p:run'])
  registry.register({
    id: 'p:run',
    pluginId: 'p',
    title: '文件整理',
    keywords: [],
    mode: 'headless',
    handler() {},
  })
  expect(registry.search('bdtx')).toEqual([])
  expect(registry.search('wjzl')).toHaveLength(1)
  registry.unregisterByPlugin('p')
  expect(registry.search('wjzl')).toEqual([])
})
test('exact ranking dominates frequency, with stable ties and bounded output', () => {
  const search = new CommandSearchIndex()
  for (const id of ['b', 'a'])
    search.upsert({ id, pluginId: 'p', title: 'foo other' })
  search.upsert({ id: 'exact', pluginId: 'p', title: 'foo' })
  expect(
    search.search('foo', {
      pinned: new Set(['a']),
      frequency: new Map([['a', 999999]]),
    })[0]?.id
  ).toBe('exact')
  expect(search.search('foo').map(item => item.id)).toEqual(['exact', 'a', 'b'])
  expect(search.search('', { limit: 1 })).toHaveLength(1)
})
