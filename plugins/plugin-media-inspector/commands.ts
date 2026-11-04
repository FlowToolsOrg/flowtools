import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'
import { mediaResultSchema } from '../media-contract'
export const inputSchema = z.object({
  planId: z
    .string()
    .regex(/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/),
  planDigest: z.string().regex(/^[a-f0-9]{64}$/),
})
export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-media-inspector',
    name: '媒体信息',
    description: '查看所选媒体的格式、时长与音视频轨道',
    version: '0.0.1',
    maturity: 'prototype',
    permissions: ['fs', 'native'],
  },
  inputSchema,
  async run(ctx, input) {
    if (!ctx.media) throw new Error('CAPABILITY_UNAVAILABLE')
    return result.json({
      result: mediaResultSchema.parse(await ctx.media.run(input.planId)),
    })
  },
})
