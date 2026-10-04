import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export const inputSchema = z.object({
  names: z.string().describe('List of names (newline or comma separated)'),
  count: z.number().default(1).describe('Number of names to pick'),
})

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-random-picker',
    name: '随机点名',
    version: '0.1.0',
    maturity: 'prototype',
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
})
