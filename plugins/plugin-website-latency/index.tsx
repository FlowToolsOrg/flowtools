import { useCallback, useState } from 'react'

import { definePlugin } from '@flow-tool/sdk'
import { Button, Card, Chip } from '@flow-tool/ui/plugin'

interface LatencyResult {
  name: string
  url: string
  latency: number | null
  status: 'pending' | 'testing' | 'success' | 'error'
  error?: string
}

const DEFAULT_SITES = [
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

async function measureLatency(
  url: string,
  signal?: AbortSignal
): Promise<number> {
  const start = performance.now()
  await fetch(url, {
    mode: 'no-cors',
    cache: 'no-store',
    signal,
  })
  return Math.round(performance.now() - start)
}

function getStatusColor(
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

function getLatencyColor(latency: number | null): string {
  if (latency === null) return 'text-gray-400'
  if (latency < 100) return 'text-green-600'
  if (latency < 300) return 'text-yellow-600'
  if (latency < 1000) return 'text-orange-600'
  return 'text-red-600'
}

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-website-latency',
    name: '网站延迟测试',
    version: '0.1.0',
    description: '测试常用网站的网络延迟（Ping）',
    permissions: ['network'],
    tags: ['network', 'latency', 'ping'],
    category: '网络工具',
  },
  setup() {
    return function WebsiteLatencyPanel() {
      const [results, setResults] = useState<LatencyResult[]>(
        DEFAULT_SITES.map(s => ({
          ...s,
          latency: null,
          status: 'pending',
        }))
      )
      const [isRunning, setIsRunning] = useState(false)
      const [controller, setController] = useState<AbortController | null>(null)

      const runTests = useCallback(async () => {
        const ac = new AbortController()
        setController(ac)
        setIsRunning(true)
        setResults(prev =>
          prev.map(r => ({ ...r, latency: null, status: 'testing' as const }))
        )

        const tasks = DEFAULT_SITES.map(async (site, index) => {
          try {
            const latency = await measureLatency(site.url, ac.signal)
            setResults(prev =>
              prev.map((r, i) =>
                i === index ? { ...r, latency, status: 'success' as const } : r
              )
            )
          } catch (err) {
            setResults(prev =>
              prev.map((r, i) =>
                i === index
                  ? {
                      ...r,
                      latency: null,
                      status: 'error' as const,
                      error: err instanceof Error ? err.message : '未知错误',
                    }
                  : r
              )
            )
          }
        })

        await Promise.allSettled(tasks)
        setIsRunning(false)
        setController(null)
      }, [])

      const stopTests = useCallback(() => {
        controller?.abort()
        setIsRunning(false)
        setController(null)
      }, [controller])

      return (
        <div className="w-full max-w-3xl mx-auto">
          <Card className="p-6">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-lg font-semibold">网站延迟测试</h2>
              <div className="flex gap-2">
                {isRunning ? (
                  <Button variant="danger" onPress={stopTests}>
                    停止
                  </Button>
                ) : (
                  <Button variant="primary" onPress={runTests}>
                    开始测试
                  </Button>
                )}
              </div>
            </div>

            <div className="space-y-2">
              {results.map(r => (
                <div
                  key={r.url}
                  className="flex items-center justify-between rounded-lg border px-4 py-3"
                >
                  <div className="flex items-center gap-3">
                    <Chip
                      size="sm"
                      variant="soft"
                      color={getStatusColor(r.status)}
                    >
                      {r.status === 'pending'
                        ? '待测'
                        : r.status === 'testing'
                          ? '测试中'
                          : r.status === 'success'
                            ? '完成'
                            : '失败'}
                    </Chip>
                    <span className="font-medium">{r.name}</span>
                    <span className="text-sm text-gray-400">{r.url}</span>
                  </div>
                  <div className="text-right">
                    {r.status === 'success' && r.latency !== null ? (
                      <span
                        className={`text-lg font-mono font-bold ${getLatencyColor(r.latency)}`}
                      >
                        {r.latency} ms
                      </span>
                    ) : r.status === 'error' ? (
                      <span className="text-sm text-red-500">
                        {r.error ?? '请求失败'}
                      </span>
                    ) : r.status === 'testing' ? (
                      <span className="text-sm text-yellow-500 animate-pulse">
                        测试中...
                      </span>
                    ) : (
                      <span className="text-sm text-gray-400">-</span>
                    )}
                  </div>
                </div>
              ))}
            </div>

            {results.some(r => r.status === 'success') && (
              <div className="mt-4 pt-4 border-t text-sm text-gray-500">
                <span>
                  平均延迟:{' '}
                  {Math.round(
                    results
                      .filter(r => r.latency !== null)
                      .reduce((sum, r) => sum + (r.latency ?? 0), 0) /
                      results.filter(r => r.latency !== null).length
                  )}{' '}
                  ms
                </span>
                <span className="mx-2">|</span>
                <span>
                  最快:{' '}
                  {
                    results
                      .filter(r => r.latency !== null)
                      .sort((a, b) => (a.latency ?? 0) - (b.latency ?? 0))[0]
                      ?.name
                  }
                </span>
              </div>
            )}
          </Card>
        </div>
      )
    }
  },
})
