import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export type HashAlgorithm = 'SHA-1' | 'SHA-256' | 'SHA-384' | 'SHA-512'

export const ALGORITHM_LABELS: Record<HashAlgorithm, string> = {
  'SHA-1': 'SHA-1',
  'SHA-256': 'SHA-256',
  'SHA-384': 'SHA-384',
  'SHA-512': 'SHA-512',
}

export async function computeHash(
  text: string,
  algorithm: HashAlgorithm
): Promise<string> {
  const encoder = new TextEncoder()
  const data = encoder.encode(text)
  const hashBuffer = await crypto.subtle.digest(algorithm, data)
  const hashArray = Array.from(new Uint8Array(hashBuffer))
  return hashArray.map(b => b.toString(16).padStart(2, '0')).join('')
}

export const inputSchema = z.object({
  text: z.string().describe('Text to hash'),
  algorithm: z
    .enum(['SHA-1', 'SHA-256', 'SHA-384', 'SHA-512'])
    .default('SHA-256')
    .describe('Hash algorithm to use'),
})

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-hash-generator',
    name: '哈希生成器',
    version: '0.1.0',
    maturity: 'prototype',
    description: '生成 SHA-1/SHA-256/SHA-384/SHA-512 哈希值',
    permissions: ['clipboard'],
    tags: ['hash', 'sha', 'sha256', 'md5', 'generator'],
    category: '开发工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const { text, algorithm } = input
    if (!text) return result.text('Error: text is required')
    const hash = await computeHash(text, algorithm)
    return result.json({ result: hash, algorithm, bytes: hash.length / 2 })
  },
})
