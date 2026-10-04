import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export type Operation = 'intersection' | 'union' | 'difference'

export function parseLines(text: string): Set<string> {
  return new Set(
    text
      .split('\n')
      .map(s => s.trim())
      .filter(Boolean)
  )
}

export function computeSetOp(
  setA: Set<string>,
  setB: Set<string>,
  op: Operation
): string[] {
  let resultSet: Set<string>
  switch (op) {
    case 'intersection':
      resultSet = new Set([...setA].filter(x => setB.has(x)))
      break
    case 'union':
      resultSet = new Set([...setA, ...setB])
      break
    case 'difference':
      resultSet = new Set([...setA].filter(x => !setB.has(x)))
      break
  }
  return [...resultSet]
}

export const OP_LABELS: Record<Operation, string> = {
  intersection: '交集 (A ∩ B)',
  union: '并集 (A ∪ B)',
  difference: '差集 (A - B)',
}

export const inputSchema = z.object({
  setA: z.string().describe('Set A items (newline-separated)'),
  setB: z.string().describe('Set B items (newline-separated)'),
  operation: z
    .enum(['intersection', 'union', 'difference'])
    .default('intersection')
    .describe('Set operation to perform'),
})

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-text-ops',
    name: '文本集合运算',
    version: '0.1.0',
    maturity: 'prototype',
    description: '计算文本的交集、差集、并集',
    permissions: ['clipboard'],
    tags: ['text', 'set', 'intersection', 'union', 'difference'],
    category: '文本工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const setA = parseLines(input.setA)
    const setB = parseLines(input.setB)
    const items = computeSetOp(setA, setB, input.operation)
    return result.json({
      result: items,
      operation: input.operation,
      count: items.length,
    })
  },
})
