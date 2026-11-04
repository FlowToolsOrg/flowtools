import { cpus, totalmem } from 'node:os'

import { CommandSearchIndex } from '@flowtools/sdk/search'

globalThis.gc?.()
const before = process.memoryUsage()
const started = performance.now()
const index = new CommandSearchIndex()
const titles = ['文件整理', '音频提取', '日程提醒', 'JSON Formatter']
for (let i = 0; i < 10_000; i++)
  index.upsert({
    id: `command-${String(i).padStart(5, '0')}`,
    pluginId: `plugin-${i % 100}`,
    title: `${titles[i % 4]} ${i}`,
    description: 'Office workflow metadata fixture',
    keywords: ['office', i % 4 === 2 ? 'schedule' : 'workflow'],
    aliases: [`alias-${i}`],
  })
const buildMs = performance.now() - started
globalThis.gc?.()
const after = process.memoryUsage()
const measure = (mode: 'text' | 'regex', queries: string[]) => {
  const durations: number[] = []
  for (let i = 0; i < 125; i++) {
    const started = performance.now()
    index.search(queries[i % queries.length]!, { mode })
    if (i >= 25) durations.push(performance.now() - started)
  }
  durations.sort((a, b) => a - b)
  return {
    samples: durations.length,
    p50Ms: durations[50],
    p95Ms: durations[95],
    maxMs: durations.at(-1),
  }
}
const text = measure('text', [
  'wenjian',
  'wjzl',
  'JSON',
  'alias-9123',
  '日程 87',
  'office schedule',
  'not-found',
])
const regex = measure('regex', ['^json', '^文件', '日程.*87$', 'alias-9123'])
const report = {
  formatVersion: 1,
  commands: index.size,
  runtime: process.version,
  platform: process.platform,
  arch: process.arch,
  cpu: cpus()[0]?.model,
  totalMemoryBytes: totalmem(),
  buildMs,
  before: { heapUsedBytes: before.heapUsed, rssBytes: before.rss },
  indexed: { heapUsedBytes: after.heapUsed, rssBytes: after.rss },
  afterQueries: process.memoryUsage(),
  heapDeltaBytes: after.heapUsed - before.heapUsed,
  rssDeltaBytes: after.rss - before.rss,
  text,
  regex,
}
process.stdout.write(JSON.stringify(report, null, 2) + '\n')
if ((text.p95Ms ?? Infinity) >= 50 || (regex.p95Ms ?? Infinity) >= 50)
  process.exitCode = 1
