import { useCallback, useState } from 'react'

import { definePlugin, useCapability } from '@flow-tool/sdk'
import { Button, Card, Chip, Label, TextArea } from '@flow-tool/ui/plugin'

type Operation = 'intersection' | 'union' | 'difference'

function parseLines(text: string): Set<string> {
  return new Set(
    text
      .split('\n')
      .map(s => s.trim())
      .filter(Boolean)
  )
}

function computeSetOp(
  setA: Set<string>,
  setB: Set<string>,
  op: Operation
): Set<string> {
  switch (op) {
    case 'intersection':
      return new Set([...setA].filter(x => setB.has(x)))
    case 'union':
      return new Set([...setA, ...setB])
    case 'difference':
      return new Set([...setA].filter(x => !setB.has(x)))
  }
}

const OP_LABELS: Record<Operation, string> = {
  intersection: '交集 (A ∩ B)',
  union: '并集 (A ∪ B)',
  difference: '差集 (A - B)',
}

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-text-ops',
    name: '文本集合运算',
    version: '0.1.0',
    description: '计算文本的交集、差集、并集',
    permissions: ['clipboard'],
    tags: ['text', 'set', 'intersection', 'union', 'difference'],
    category: '文本工具',
  },
  setup() {
    return function TextOpsPanel() {
      const { clipboard } = useCapability()
      const [textA, setTextA] = useState('')
      const [textB, setTextB] = useState('')
      const [operation, setOperation] = useState<Operation>('intersection')
      const [result, setResult] = useState<string[] | null>(null)
      const [copied, setCopied] = useState(false)

      const compute = useCallback(() => {
        const setA = parseLines(textA)
        const setB = parseLines(textB)
        const resultSet = computeSetOp(setA, setB, operation)
        setResult([...resultSet])
      }, [textA, textB, operation])

      const copyResult = useCallback(async () => {
        if (!result) return
        await clipboard.writeText(result.join('\n'))
        setCopied(true)
        setTimeout(() => setCopied(false), 1500)
      }, [clipboard, result])

      const setA = parseLines(textA)
      const setB = parseLines(textB)

      return (
        <div className="w-full max-w-4xl mx-auto">
          <Card className="p-6">
            <h2 className="text-lg font-semibold mb-4">文本集合运算</h2>

            <div className="grid grid-cols-2 gap-4 mb-4">
              <div>
                <Label>集合 A（每行一项）</Label>
                <TextArea
                  placeholder={'apple\nbanana\ncherry\ndate'}
                  value={textA}
                  onChange={e => setTextA(e.target.value)}
                  rows={8}
                />
                <div className="mt-1 text-xs text-gray-400">{setA.size} 项</div>
              </div>
              <div>
                <Label>集合 B（每行一项）</Label>
                <TextArea
                  placeholder={'banana\ndate\nfig\ngrape'}
                  value={textB}
                  onChange={e => setTextB(e.target.value)}
                  rows={8}
                />
                <div className="mt-1 text-xs text-gray-400">{setB.size} 项</div>
              </div>
            </div>

            <div className="flex items-center gap-2 mb-4">
              {(Object.keys(OP_LABELS) as Operation[]).map(op => (
                <Button
                  key={op}
                  variant={operation === op ? 'primary' : 'ghost'}
                  onPress={() => {
                    setOperation(op)
                    setResult(null)
                  }}
                >
                  {OP_LABELS[op]}
                </Button>
              ))}
            </div>

            <Button
              variant="primary"
              onPress={compute}
              isDisabled={!textA.trim() || !textB.trim()}
            >
              计算
            </Button>

            {result !== null && (
              <div className="mt-4 pt-4 border-t">
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <h3 className="text-sm font-medium">结果</h3>
                    <Chip size="sm" variant="soft">
                      {result.length} 项
                    </Chip>
                  </div>
                  <Button size="sm" variant="ghost" onPress={copyResult}>
                    {copied ? '已复制!' : '复制结果'}
                  </Button>
                </div>

                {result.length === 0 ? (
                  <p className="text-sm text-gray-400">空集</p>
                ) : (
                  <div className="rounded-lg border bg-gray-50 p-3 max-h-60 overflow-y-auto">
                    <pre className="text-sm whitespace-pre-wrap">
                      {result.join('\n')}
                    </pre>
                  </div>
                )}
              </div>
            )}
          </Card>
        </div>
      )
    }
  },
})
