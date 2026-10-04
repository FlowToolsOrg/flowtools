import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export interface MatchResult {
  match: string
  index: number
  groups?: Record<string, string>
}

export const inputSchema = z.object({
  pattern: z.string().describe('Regular expression pattern'),
  text: z.string().describe('Text to test against'),
  flags: z.string().default('g').describe('Regex flags (e.g., "gi", "gm")'),
})

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-regex-tester',
    name: '正则表达式测试',
    version: '0.1.0',
    maturity: 'prototype',
    description: '测试和调试正则表达式',
    permissions: ['clipboard'],
    tags: ['regex', 'regexp', 'test', 'pattern'],
    category: '开发工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const { pattern, text, flags } = input
    if (!pattern) return result.text('Error: pattern is required')
    if (!text) return result.text('Error: text is required')
    try {
      const regex = new RegExp(pattern, flags)
      const matches: MatchResult[] = []
      if (flags.includes('g')) {
        let match: RegExpExecArray | null
        while ((match = regex.exec(text)) !== null) {
          matches.push({
            match: match[0],
            index: match.index,
            ...(match.groups
              ? {
                  groups: Object.fromEntries(
                    Object.entries(match.groups).filter(
                      ([, group]) => group !== undefined
                    )
                  ),
                }
              : {}),
          })
          if (!match[0]) break
        }
      } else {
        const match = regex.exec(text)
        if (match) {
          matches.push({
            match: match[0],
            index: match.index,
            ...(match.groups
              ? {
                  groups: Object.fromEntries(
                    Object.entries(match.groups).filter(
                      ([, group]) => group !== undefined
                    )
                  ),
                }
              : {}),
          })
        }
      }
      return result.json({
        result: matches,
        pattern,
        flags,
        matchCount: matches.length,
      })
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Invalid regex'
      return result.text(`Error: ${message}`)
    }
  },
})
