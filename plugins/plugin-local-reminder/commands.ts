import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export const inputSchema = z.object({
  title: z.string().min(1).max(64).describe('提醒标题'),
  message: z.string().max(1024).default('').describe('提醒内容'),
})

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-local-reminder',
    name: '本地提醒',
    description: '在共享 Runtime 中创建有限周期提醒，并查看发送尝试结果',
    version: '0.0.1',
    maturity: 'prototype',
    permissions: ['notification'],
  },
  inputSchema,
  async run(ctx, input: z.infer<typeof inputSchema>) {
    const status = ctx.notification?.deliver
      ? await ctx.notification.deliver(input)
      : 'unsupported'
    return result.json({ result: { status } })
  },
})
