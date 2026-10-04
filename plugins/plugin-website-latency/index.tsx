import { useCallback, useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import { Button, Card, Chip } from '@flowtools/ui/plugin'

import commandPlugin, {
  type LatencyResult,
  DEFAULT_SITES,
  measureLatency,
  getStatusColor,
  getLatencyColor,
} from './commands'

export default definePlugin({
  ...commandPlugin,
  setup() {
    return function WebsiteLatencyPanel() {
      const { request } = useCapability()
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
            const latency = await measureLatency(site.url, request, ac.signal)
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
      }, [request])
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
