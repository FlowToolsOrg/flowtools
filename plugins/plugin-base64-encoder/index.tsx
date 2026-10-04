import { useCallback, useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import { result } from '@flowtools/sdk/result'
import { Button, Card, Chip, Label, TextArea } from '@flowtools/ui/plugin'
import { z } from 'zod'

type Mode = 'encode' | 'decode'

function encodeBase64(text: string): string {
  try {
    return btoa(
      encodeURIComponent(text).replace(/%([0-9A-F]{2})/g, (_, p1) =>
        String.fromCharCode(parseInt(p1, 16))
      )
    )
  } catch (e) {
    throw new Error(
      '编码失败: ' + (e instanceof Error ? e.message : '未知错误')
    )
  }
}

function decodeBase64(base64: string): string {
  try {
    return decodeURIComponent(
      Array.from(
        atob(base64),
        c => '%' + ('00' + c.charCodeAt(0).toString(16)).slice(-2)
      ).join('')
    )
  } catch {
    throw new Error('解码失败: 无效的 Base64 字符串')
  }
}

const inputSchema = z.object({
  text: z.string().describe('Text to encode or decode'),
  mode: z
    .enum(['encode', 'decode'])
    .default('encode')
    .describe('Encode or decode mode'),
})

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-base64-encoder',
    name: 'Base64 编解码',
    version: '0.1.0',
    maturity: 'prototype',
    description: '文本与 Base64 编码互转',
    permissions: ['clipboard'],
    tags: ['base64', 'encode', 'decode', 'converter'],
    category: '编码工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const { text, mode } = input
    if (!text) return result.text('Error: text is required')
    try {
      const output = mode === 'encode' ? encodeBase64(text) : decodeBase64(text)
      return result.json({ result: output, mode, input: text })
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Conversion failed'
      return result.text(`Error: ${message}`)
    }
  },
  setup() {
    return function Base64EncoderPanel() {
      const { clipboard } = useCapability()
      const [input, setInput] = useState('')
      const [output, setOutput] = useState('')
      const [mode, setMode] = useState<Mode>('encode')
      const [error, setError] = useState<string | null>(null)
      const [copied, setCopied] = useState(false)

      const convert = useCallback(() => {
        setError(null)
        setOutput('')

        if (!input.trim()) {
          setError('请输入内容')
          return
        }

        try {
          const result =
            mode === 'encode' ? encodeBase64(input) : decodeBase64(input)
          setOutput(result)
        } catch (e) {
          setError(e instanceof Error ? e.message : '转换失败')
        }
      }, [input, mode])

      const copyOutput = useCallback(async () => {
        if (!output) return
        await clipboard.writeText(output)
        setCopied(true)
        setTimeout(() => setCopied(false), 1500)
      }, [clipboard, output])

      const swap = useCallback(() => {
        setInput(output)
        setOutput('')
        setMode(prev => (prev === 'encode' ? 'decode' : 'encode'))
        setError(null)
      }, [output])

      const clear = useCallback(() => {
        setInput('')
        setOutput('')
        setError(null)
      }, [])

      return (
        <div className="w-full max-w-4xl mx-auto">
          <Card className="p-6">
            <h2 className="text-lg font-semibold mb-4">Base64 编解码</h2>

            <div className="flex gap-2 mb-4">
              <Button
                variant={mode === 'encode' ? 'primary' : 'ghost'}
                onPress={() => {
                  setMode('encode')
                  setOutput('')
                  setError(null)
                }}
              >
                编码
              </Button>
              <Button
                variant={mode === 'decode' ? 'primary' : 'ghost'}
                onPress={() => {
                  setMode('decode')
                  setOutput('')
                  setError(null)
                }}
              >
                解码
              </Button>
            </div>

            <div className="grid grid-cols-2 gap-4 mb-4">
              <div>
                <Label>
                  {mode === 'encode' ? '原始文本' : 'Base64 字符串'}
                </Label>
                <TextArea
                  placeholder={
                    mode === 'encode'
                      ? '输入要编码的文本'
                      : '输入 Base64 字符串'
                  }
                  value={input}
                  onChange={e => setInput(e.target.value)}
                  rows={10}
                  className="font-mono text-sm"
                />
              </div>
              <div>
                <div className="flex items-center justify-between mb-1">
                  <Label>
                    {mode === 'encode' ? 'Base64 结果' : '解码结果'}
                  </Label>
                  {output && (
                    <Button size="sm" variant="ghost" onPress={copyOutput}>
                      {copied ? '已复制!' : '复制'}
                    </Button>
                  )}
                </div>
                <TextArea
                  value={output}
                  readOnly
                  rows={10}
                  className="font-mono text-sm"
                  placeholder="结果将显示在这里"
                />
              </div>
            </div>

            <div className="flex items-center gap-3">
              <Button variant="primary" onPress={convert}>
                {mode === 'encode' ? '编码' : '解码'}
              </Button>
              <Button variant="ghost" onPress={swap}>
                交换输入输出
              </Button>
              <Button variant="ghost" onPress={clear}>
                清空
              </Button>
            </div>

            {error && (
              <div className="mt-3 p-3 rounded-lg bg-danger-50 text-danger text-sm">
                {error}
              </div>
            )}

            {output && (
              <div className="mt-3 flex items-center gap-2">
                <Chip size="sm" variant="soft">
                  输入: {input.length} 字符
                </Chip>
                <Chip size="sm" variant="soft">
                  输出: {output.length} 字符
                </Chip>
              </div>
            )}
          </Card>
        </div>
      )
    }
  },
})
