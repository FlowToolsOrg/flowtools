import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export type TimestampUnit = 'seconds' | 'milliseconds'

export function formatDate(timestamp: number, unit: TimestampUnit): string {
  const ms = unit === 'seconds' ? timestamp * 1000 : timestamp
  const date = new Date(ms)

  if (isNaN(date.getTime())) {
    return '无效时间戳'
  }

  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
    timeZoneName: 'short',
  })
}

export function parseDateInput(dateStr: string): number | null {
  const date = new Date(dateStr)
  if (isNaN(date.getTime())) return null
  return date.getTime()
}

export const inputSchema = z.object({
  timestamp: z
    .string()
    .optional()
    .describe('Unix timestamp to convert to date'),
  date: z
    .string()
    .optional()
    .describe('Date string to convert to timestamp (ISO format)'),
  unit: z
    .enum(['seconds', 'milliseconds'])
    .default('seconds')
    .describe('Timestamp unit'),
})

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-timestamp-converter',
    name: '时间戳转换',
    version: '0.1.0',
    maturity: 'prototype',
    description: 'Unix 时间戳与日期时间互转',
    permissions: ['clipboard'],
    tags: ['timestamp', 'unix', 'date', 'converter'],
    category: '开发工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const unit = input.unit ?? 'seconds'
    if (input.timestamp) {
      const ts = Number(input.timestamp)
      if (isNaN(ts)) return result.text('Error: invalid timestamp')
      const iso = new Date(unit === 'seconds' ? ts * 1000 : ts).toISOString()
      return result.json({ result: iso, timestamp: input.timestamp, unit })
    }
    if (input.date) {
      const ms = parseDateInput(input.date)
      if (ms === null) return result.text('Error: invalid date string')
      const ts = unit === 'seconds' ? Math.floor(ms / 1000) : ms
      return result.json({ result: ts, date: input.date, unit })
    }
    const now = Date.now()
    const ts = unit === 'seconds' ? Math.floor(now / 1000) : now
    return result.json({
      result: ts,
      unit,
      iso: new Date(now).toISOString(),
    })
  },
})
