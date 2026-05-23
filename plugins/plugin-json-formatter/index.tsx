import { useCallback, useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import { result } from '@flowtools/sdk/result'
import { Button, Card, Chip, Label, TextArea } from '@flowtools/ui/plugin'
import { z } from 'zod'

type FormatMode = 'format' | 'minify' | 'validate'

const inputSchema = z.object({
  text: z.string().describe('JSON string to process'),
  mode: z
    .enum(['format', 'minify', 'validate'])
    .default('format')
    .describe('Processing mode'),
})

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-json-formatter',
    name: 'JSON 格式化',
    version: '0.1.0',
    description: 'JSON 格式化、压缩、验证工具',
    permissions: ['clipboard'],
    tags: ['json', 'format', 'minify', 'validate'],
    category: '开发工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const { text, mode } = input
    if (!text) return result.text('Error: text is required')
    try {
      const parsed: unknown = JSON.parse(text)
      if (mode === 'validate') return result.json({ result: { valid: true } })
      const output =
        mode === 'format'
          ? JSON.stringify(parsed, null, 2)
          : JSON.stringify(parsed)
      return result.text(output)
    } catch (e) {
      const message = e instanceof Error ? e.message : 'JSON parse error'
      if (mode === 'validate')
        return result.json({ result: { valid: false, error: message } })
      return result.text(`Error: ${message}`)
    }
  },
  setup() {
    return function JSONFormatterPanel() {
      const { clipboard } = useCapability()
      const [input, setInput] = useState('')
      const [output, setOutput] = useState('')
      const [mode, setMode] = useState<FormatMode>('format')
      const [error, setError] = useState<string | null>(null)
      const [copied, setCopied] = useState(false)
      const [isValid, setIsValid] = useState<boolean | null>(null)

      const process = useCallback(() => {
        setError(null)
        setOutput('')
        setIsValid(null)

        if (!input.trim()) {
          setError('请输入 JSON 内容')
          return
        }

        try {
          const parsed = JSON.parse(input)

          if (mode === 'validate') {
            setIsValid(true)
            setOutput('JSON 格式有效')
            return
          }

          const result =
            mode === 'format'
              ? JSON.stringify(parsed, null, 2)
              : JSON.stringify(parsed)

          setOutput(result)
        } catch (e) {
          if (mode === 'validate') {
            setIsValid(false)
            setOutput('')
          }
          setError(e instanceof Error ? e.message : 'JSON 解析失败')
        }
      }, [input, mode])

      const copyOutput = useCallback(async () => {
        if (!output) return
        await clipboard.writeText(output)
        setCopied(true)
        setTimeout(() => setCopied(false), 1500)
      }, [clipboard, output])

      const clear = useCallback(() => {
        setInput('')
        setOutput('')
        setError(null)
        setIsValid(null)
      }, [])

      return (
        <div className="w-full max-w-4xl mx-auto">
          <Card className="p-6">
            <h2 className="text-lg font-semibold mb-4">JSON 格式化</h2>

            <div className="flex gap-2 mb-4">
              <Button
                variant={mode === 'format' ? 'primary' : 'ghost'}
                onPress={() => setMode('format')}
              >
                格式化
              </Button>
              <Button
                variant={mode === 'minify' ? 'primary' : 'ghost'}
                onPress={() => setMode('minify')}
              >
                压缩
              </Button>
              <Button
                variant={mode === 'validate' ? 'primary' : 'ghost'}
                onPress={() => setMode('validate')}
              >
                验证
              </Button>
            </div>

            <div className="grid grid-cols-2 gap-4 mb-4">
              <div>
                <Label>输入</Label>
                <TextArea
                  placeholder='{"name": "value", "array": [1, 2, 3]}'
                  value={input}
                  onChange={e => setInput(e.target.value)}
                  rows={12}
                  className="font-mono text-sm"
                />
              </div>
              <div>
                <div className="flex items-center justify-between mb-1">
                  <Label>输出</Label>
                  {output && (
                    <Button size="sm" variant="ghost" onPress={copyOutput}>
                      {copied ? '已复制!' : '复制'}
                    </Button>
                  )}
                </div>
                <TextArea
                  value={output}
                  readOnly
                  rows={12}
                  className="font-mono text-sm"
                  placeholder="结果将显示在这里"
                />
              </div>
            </div>

            <div className="flex items-center gap-3">
              <Button variant="primary" onPress={process}>
                {mode === 'validate' ? '验证' : '转换'}
              </Button>
              <Button variant="ghost" onPress={clear}>
                清空
              </Button>

              {isValid !== null && (
                <Chip
                  size="sm"
                  variant="soft"
                  color={isValid ? 'success' : 'danger'}
                >
                  {isValid ? 'JSON 有效' : 'JSON 无效'}
                </Chip>
              )}
            </div>

            {error && (
              <div className="mt-3 p-3 rounded-lg bg-danger-50 text-danger text-sm">
                {error}
              </div>
            )}
          </Card>
        </div>
      )
    }
  },
})
