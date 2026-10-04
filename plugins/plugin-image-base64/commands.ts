import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export const inputSchema = z.object({
  base64: z.string().optional().describe('Base64 string to get info about'),
})

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-image-base64',
    name: '图片 Base64 互转',
    version: '0.1.0',
    maturity: 'prototype',
    description: '在线图片与 Base64 编码互相转换',
    permissions: ['clipboard'],
    tags: ['image', 'base64', 'converter'],
    category: '编码工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const b64 = input.base64
    if (!b64) return result.text('Error: base64 is required')
    const src = b64.startsWith('data:') ? b64 : `data:image/png;base64,${b64}`
    const raw = src.includes(',') ? src.split(',')[1] : src
    const bytes = Math.round((raw.length * 3) / 4)
    return result.json({
      result: {
        dataUrl: src.slice(0, 80) + '...',
        estimatedBytes: bytes,
        hasPrefix: src.startsWith('data:'),
      },
    })
  },
})
