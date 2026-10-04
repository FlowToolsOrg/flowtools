import { useCallback, useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import { result } from '@flowtools/sdk/result'
import { Button, Card, Chip, Label, TextArea } from '@flowtools/ui/plugin'
import { z } from 'zod'

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
): string[] {
  let resultSet: Set<string>
  switch (op) {
    case 'intersection':
      resultSet = new Set([...setA].filter(x => setB.has(x)))
      break
    case 'union':
      resultSet = new Set([...setA, ...setB])
      break
    case 'difference':
      resultSet = new Set([...setA].filter(x => !setB.has(x)))
      break
  }
  return [...resultSet]
}

const OP_LABELS: Record<Operation, string> = {
  intersection: '交集 (A ∩ B)',
  union: '并集 (A ∪ B)',
  difference: '差集 (A - B)',
}

const inputSchema = z.object({
  setA: z.string().describe('Set A items (newline-separated)'),
  setB: z.string().describe('Set B items (newline-separated)'),
  operation: z
    .enum(['intersection', 'union', 'difference'])
    .default('intersection')
    .describe('Set operation to perform'),
})

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-text-ops',
    name: '文本集合运算',
    version: '0.1.0',
    maturity: 'prototype',
    description: '计算文本的交集、差集、并集',
    permissions: ['clipboard'],
    tags: ['text', 'set', 'intersection', 'union', 'difference'],
    category: '文本工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const setA = parseLines(input.setA)
    const setB = parseLines(input.setB)
    const items = computeSetOp(setA, setB, input.operation)
    return result.json({
      result: items,
      operation: input.operation,
      count: items.length,
    })
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
