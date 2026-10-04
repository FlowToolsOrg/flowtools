import { useCallback, useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import { Button, Card, Chip, Label, TextArea } from '@flowtools/ui/plugin'

import commandPlugin, {
  type HashAlgorithm,
  ALGORITHM_LABELS,
  computeHash,
} from './commands'

export default definePlugin({
  ...commandPlugin,
  setup() {
    return function HashGeneratorPanel() {
      const { clipboard } = useCapability()
      const [input, setInput] = useState('')
      const [results, setResults] = useState<Record<string, string>>({})
      const [selectedAlgo, setSelectedAlgo] = useState<HashAlgorithm>('SHA-256')
      const [isComputing, setIsComputing] = useState(false)
      const [copiedAlgo, setCopiedAlgo] = useState<string | null>(null)
      const computeAll = useCallback(async () => {
        if (!input.trim()) return
        setIsComputing(true)
        const newResults: Record<string, string> = {}
        for (const algo of Object.keys(ALGORITHM_LABELS) as HashAlgorithm[]) {
          newResults[algo] = await computeHash(input, algo)
        }
        setResults(newResults)
        setIsComputing(false)
      }, [input])
      const computeSingle = useCallback(
        async (algo: HashAlgorithm) => {
          if (!input.trim()) return
          setIsComputing(true)
          const hash = await computeHash(input, algo)
          setResults(prev => ({ ...prev, [algo]: hash }))
          setSelectedAlgo(algo)
          setIsComputing(false)
        },
        [input]
      )
      const copyHash = useCallback(
        async (algo: string) => {
          const hash = results[algo]
          if (!hash) return
          await clipboard.writeText(hash)
          setCopiedAlgo(algo)
          setTimeout(() => setCopiedAlgo(null), 1500)
        },
        [clipboard, results]
      )
      const clear = useCallback(() => {
        setInput('')
        setResults({})
      }, [])
      return (
        <div className="w-full max-w-2xl mx-auto">
          <Card className="p-6">
            <h2 className="text-lg font-semibold mb-4">哈希生成器</h2>

            <div className="mb-4">
              <Label>输入文本</Label>
              <TextArea
                placeholder="输入要计算哈希的文本"
                value={input}
                onChange={e => setInput(e.target.value)}
                rows={6}
                className="font-mono text-sm"
              />
            </div>

            <div className="flex flex-wrap gap-2 mb-4">
              {(Object.keys(ALGORITHM_LABELS) as HashAlgorithm[]).map(algo => (
                <Button
                  key={algo}
                  variant={selectedAlgo === algo ? 'primary' : 'ghost'}
                  onPress={() => computeSingle(algo)}
                  isDisabled={isComputing || !input.trim()}
                >
                  {ALGORITHM_LABELS[algo]}
                </Button>
              ))}
              <Button
                variant="ghost"
                onPress={computeAll}
                isDisabled={isComputing || !input.trim()}
              >
                全部计算
              </Button>
            </div>

            {Object.keys(results).length > 0 && (
              <div className="space-y-3">
                {(Object.entries(results) as [string, string][]).map(
                  ([algo, hash]) => (
                    <div
                      key={algo}
                      className="p-3 rounded-lg border bg-gray-50"
                    >
                      <div className="flex items-center justify-between mb-1">
                        <div className="flex items-center gap-2">
                          <Chip
                            size="sm"
                            variant="soft"
                            color={
                              algo === selectedAlgo ? 'success' : 'default'
                            }
                          >
                            {algo}
                          </Chip>
                          <span className="text-xs text-gray-500">
                            {hash.length / 2} 字节
                          </span>
                        </div>
                        <Button
                          size="sm"
                          variant="ghost"
                          onPress={() => copyHash(algo)}
                        >
                          {copiedAlgo === algo ? '已复制!' : '复制'}
                        </Button>
                      </div>
                      <div className="font-mono text-xs break-all">{hash}</div>
                    </div>
                  )
                )}
              </div>
            )}

            <div className="mt-4 flex gap-2">
              <Button variant="ghost" onPress={clear}>
                清空
              </Button>
            </div>
          </Card>
        </div>
      )
    }
  },
})
