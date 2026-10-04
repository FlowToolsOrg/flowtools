import { useCallback, useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import { result } from '@flowtools/sdk/result'
import { Button, Card, Input, Label, TextField } from '@flowtools/ui/plugin'
import { z } from 'zod'

function generateUUID(): string {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return crypto.randomUUID()
  }
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, c => {
    const r = (Math.random() * 16) | 0
    const v = c === 'x' ? r : (r & 0x3) | 0x8
    return v.toString(16)
  })
}

const inputSchema = z.object({
  count: z.number().default(1).describe('Number of UUIDs to generate (1-100)'),
})

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-uuid-generator',
    name: 'UUID 生成器',
    version: '0.1.0',
    maturity: 'prototype',
    description: '在线生成随机 UUID v4',
    permissions: ['clipboard'],
    tags: ['uuid', 'random', 'generator'],
    category: '开发工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const count = Math.max(1, Math.min(100, input.count ?? 1))
    const uuids = Array.from({ length: count }, () => generateUUID())
    return result.json({ result: uuids, count: uuids.length })
  },
  setup() {
    return function UUIDGeneratorPanel() {
      const { clipboard } = useCapability()
      const [uuids, setUuids] = useState<string[]>([generateUUID()])
      const [count, setCount] = useState(1)
      const [copiedIndex, setCopiedIndex] = useState<number | null>(null)

      const generate = useCallback(() => {
        const n = Math.max(1, Math.min(100, count))
        const newUuids = Array.from({ length: n }, () => generateUUID())
        setUuids(newUuids)
        setCopiedIndex(null)
      }, [count])

      const copyOne = useCallback(
        async (uuid: string, index: number) => {
          await clipboard.writeText(uuid)
          setCopiedIndex(index)
          setTimeout(() => setCopiedIndex(null), 1500)
        },
        [clipboard]
      )

      const copyAll = useCallback(async () => {
        await clipboard.writeText(uuids.join('\n'))
        setCopiedIndex(-1)
        setTimeout(() => setCopiedIndex(null), 1500)
      }, [clipboard, uuids])

      return (
        <div className="w-full max-w-2xl mx-auto">
          <Card className="p-6">
            <h2 className="text-lg font-semibold mb-4">UUID 生成器</h2>

            <div className="flex items-end gap-3 mb-4">
              <TextField
                className="w-32"
                value={String(count)}
                onChange={v => setCount(Number(v) || 1)}
              >
                <Label>生成数量</Label>
                <Input type="number" min={1} max={100} />
              </TextField>
              <Button variant="primary" onPress={generate}>
                生成
              </Button>
              {uuids.length > 1 && (
                <Button variant="ghost" onPress={copyAll}>
                  {copiedIndex === -1 ? '已复制!' : '复制全部'}
                </Button>
              )}
            </div>

            <div className="space-y-2">
              {uuids.map((uuid, index) => (
                <div
                  key={`${uuid}-${index}`}
                  className="flex items-center justify-between rounded-lg border px-4 py-2.5 font-mono text-sm"
                >
                  <span className="select-all">{uuid}</span>
                  <Button
                    size="sm"
                    variant="ghost"
                    onPress={() => copyOne(uuid, index)}
                  >
                    {copiedIndex === index ? '已复制!' : '复制'}
                  </Button>
                </div>
              ))}
            </div>
          </Card>
        </div>
      )
    }
  },
})
