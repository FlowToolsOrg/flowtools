import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export type FormatMode = 'format' | 'minify' | 'validate'

export const inputSchema = z.object({
  text: z.string().describe('JSON string to process'),
  mode: z
    .enum(['format', 'minify', 'validate'])
    .default('format')
    .describe('Processing mode'),
})

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-json-formatter',
    name: 'JSON 格式化',
    version: '0.1.0',
    maturity: 'prototype',
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
})
