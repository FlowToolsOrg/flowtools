import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export function generateUUID(): string {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return crypto.randomUUID()
  }
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, c => {
    const r = (Math.random() * 16) | 0
    const v = c === 'x' ? r : (r & 0x3) | 0x8
    return v.toString(16)
  })
}

export const inputSchema = z.object({
  count: z.number().default(1).describe('Number of UUIDs to generate (1-100)'),
})

export default defineBuiltinCommand({
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
})
