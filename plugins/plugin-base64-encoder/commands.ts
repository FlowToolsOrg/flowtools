import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export type Mode = 'encode' | 'decode'

export function encodeBase64(text: string): string {
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

export function decodeBase64(base64: string): string {
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

export const inputSchema = z.object({
  text: z.string().describe('Text to encode or decode'),
  mode: z
    .enum(['encode', 'decode'])
    .default('encode')
    .describe('Encode or decode mode'),
})

export default defineBuiltinCommand({
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
})
