import { useCallback, useRef, useState } from 'react'

import { definePlugin } from '@flowtools/sdk'
import { result } from '@flowtools/sdk/result'
import { Button, Card, Chip, Label, TextArea } from '@flowtools/ui/plugin'
import { z } from 'zod'

const inputSchema = z.object({
  names: z.string().describe('List of names (newline or comma separated)'),
  count: z.number().default(1).describe('Number of names to pick'),
})

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-random-picker',
    name: '随机点名',
    version: '0.1.0',
    description: '在线名单随机点名工具',
    permissions: [],
    tags: ['random', 'picker', 'name'],
    category: '实用工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const { names: text } = input
    if (!text) return result.text('Error: names is required')
    const nameList = text
      .split(/[\n,，、;；]+/)
      .map(s => s.trim())
      .filter(Boolean)
    if (nameList.length === 0) return result.text('Error: no valid names')
    const count = Math.min(input.count ?? 1, nameList.length)
    const picked: string[] = []
    const remaining = [...nameList]
    for (let i = 0; i < count; i++) {
      const idx = Math.floor(Math.random() * remaining.length)
      picked.push(remaining[idx])
      remaining.splice(idx, 1)
    }
    return result.json({ result: picked, total: nameList.length })
  },
  setup() {
    return function RandomPickerPanel() {
      const [names, setNames] = useState('')
      const [picked, setPicked] = useState<string | null>(null)
      const [history, setHistory] = useState<string[]>([])
      const [isRolling, setIsRolling] = useState(false)
      const [rollingName, setRollingName] = useState('')
      const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

      const parseNames = useCallback((text: string): string[] => {
        return text
          .split(/[\n,，、;；]+/)
          .map(s => s.trim())
          .filter(Boolean)
      }, [])

      const pick = useCallback(() => {
        const list = parseNames(names)
        if (list.length === 0) return

        setIsRolling(true)
        setPicked(null)

        let count = 0
        const totalTicks = 20

        timerRef.current = setInterval(() => {
          const randomIndex = Math.floor(Math.random() * list.length)
          setRollingName(list[randomIndex])
          count++

          if (count >= totalTicks) {
            if (timerRef.current) clearInterval(timerRef.current)
            const finalIndex = Math.floor(Math.random() * list.length)
            const selected = list[finalIndex]
            setPicked(selected)
            setHistory(prev => [selected, ...prev].slice(0, 20))
            setRollingName('')
            setIsRolling(false)
          }
        }, 80)
      }, [names, parseNames])

      const stop = useCallback(() => {
        if (timerRef.current) clearInterval(timerRef.current)
        setIsRolling(false)
        setRollingName('')
      }, [])

      const clearHistory = useCallback(() => {
        setHistory([])
        setPicked(null)
      }, [])

      const nameList = parseNames(names)

      return (
        <div className="w-full max-w-2xl mx-auto">
          <Card className="p-6">
            <h2 className="text-lg font-semibold mb-4">随机点名</h2>

            <Label>名单（每行一个名字，或用逗号分隔）</Label>
            <TextArea
              placeholder={'张三\n李四\n王五\n赵六'}
              value={names}
              onChange={e => setNames(e.target.value)}
              rows={6}
            />

            {nameList.length > 0 && (
              <div className="mt-2 text-sm text-gray-500">
                共 {nameList.length} 人
              </div>
            )}

            <div className="flex gap-3 mt-4">
              {isRolling ? (
                <Button variant="danger" onPress={stop}>
                  停止
                </Button>
              ) : (
                <Button
                  variant="primary"
                  onPress={pick}
                  isDisabled={nameList.length === 0}
                >
                  开始点名
                </Button>
              )}
              {history.length > 0 && (
                <Button variant="ghost" onPress={clearHistory}>
                  清空记录
                </Button>
              )}
            </div>

            {isRolling && (
              <div className="mt-6 text-center">
                <div className="text-4xl font-bold animate-pulse text-primary">
                  {rollingName}
                </div>
              </div>
            )}

            {picked && !isRolling && (
              <div className="mt-6 text-center">
                <div className="text-sm text-gray-500 mb-1">点中</div>
                <div className="text-5xl font-bold text-success">{picked}</div>
              </div>
            )}

            {history.length > 0 && (
              <div className="mt-6 pt-4 border-t">
                <h3 className="text-sm font-medium mb-2">历史记录</h3>
                <div className="flex flex-wrap gap-2">
                  {history.map((name, i) => (
                    <Chip
                      key={`${name}-${i}`}
                      size="sm"
                      variant="soft"
                      color={i === 0 ? 'success' : 'default'}
                    >
                      {name}
                    </Chip>
                  ))}
                </div>
              </div>
            )}
          </Card>
        </div>
      )
    }
  },
})
