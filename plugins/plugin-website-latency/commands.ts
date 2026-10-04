import type { RequestCapability } from '@flowtools/sdk/types'

import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export interface LatencyResult {
  name: string
  url: string
  latency: number | null
  status: 'pending' | 'testing' | 'success' | 'error'
  error?: string
}

export const DEFAULT_SITES = [
  { name: '百度', url: 'https://www.baidu.com' },
  { name: '淘宝', url: 'https://www.taobao.com' },
  { name: '抖音', url: 'https://www.douyin.com' },
  { name: '京东', url: 'https://www.jd.com' },
  { name: '哔哩哔哩', url: 'https://www.bilibili.com' },
  { name: '知乎', url: 'https://www.zhihu.com' },
  { name: '微博', url: 'https://weibo.com' },
  { name: 'GitHub', url: 'https://github.com' },
  { name: 'Google', url: 'https://www.google.com' },
  { name: 'YouTube', url: 'https://www.youtube.com' },
]

export async function measureLatency(
  url: string,
  request: RequestCapability,
  signal?: AbortSignal
): Promise<number> {
  const start = performance.now()
  await request(url, {
    mode: 'no-cors',
    cache: 'no-store',
    signal,
  })
  return Math.round(performance.now() - start)
}

export function getStatusColor(
  status: LatencyResult['status']
): 'success' | 'danger' | 'warning' | 'default' {
  switch (status) {
    case 'success':
      return 'success'
    case 'error':
      return 'danger'
    case 'testing':
      return 'warning'
    default:
      return 'default'
  }
}

export function getLatencyColor(latency: number | null): string {
  if (latency === null) return 'text-gray-400'
  if (latency < 100) return 'text-green-600'
  if (latency < 300) return 'text-yellow-600'
  if (latency < 1000) return 'text-orange-600'
  return 'text-red-600'
}

export const inputSchema = z.object({
  urls: z
    .array(z.url())
    .optional()
    .describe('Comma-separated URLs to test (defaults to popular sites)'),
})

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-website-latency',
    name: '网站延迟测试',
    version: '0.1.0',
    maturity: 'prototype',
    description: '测试常用网站的网络延迟（Ping）',
    permissions: ['network'],
    tags: ['network', 'latency', 'ping'],
    category: '网络工具',
  },
  inputSchema,
  async run(ctx, input: z.infer<typeof inputSchema>) {
    const request = ctx.request
    if (!request) throw new Error('Network capability is unavailable')
    const sites = input.urls
      ? input.urls.filter(Boolean)
      : DEFAULT_SITES.map(s => s.url)
    const results = await Promise.allSettled(
      sites.map(async url => {
        try {
          const latency = await measureLatency(url, request, ctx.signal)
          return { url, latency, status: 'success' as const }
        } catch (err) {
          return {
            url,
            latency: null,
            status: 'error' as const,
            error: err instanceof Error ? err.message : 'request failed',
          }
        }
      })
    )
    const items = results.map(r =>
      r.status === 'fulfilled'
        ? r.value
        : { url: '?', latency: null, status: 'error' as const }
    )
    const successes = items.filter(i => i.latency !== null)
    const avg = successes.length
      ? Math.round(
          successes.reduce((s, i) => s + (i.latency ?? 0), 0) / successes.length
        )
      : null
    return result.json({
      result: items.map(
        i =>
          `${i.url}: ${i.latency !== null ? `${i.latency}ms` : `error (${'error' in i ? i.error : 'request failed'})`}`
      ),
      average: avg,
      tested: items.length,
    })
  },
})
