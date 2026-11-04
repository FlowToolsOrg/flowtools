import { z } from 'zod'
export const mediaInputSchema = z.object({
  planId: z
    .string()
    .regex(/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/)
    .describe('已批准媒体计划的 ID'),
  planDigest: z
    .string()
    .regex(/^[a-f0-9]{64}$/)
    .describe('媒体计划摘要'),
})
export const mediaResultSchema = z.strictObject({
  formats: z.array(z.string()).max(8),
  durationMs: z.number().nullable(),
  sourceBytes: z.number().nullable(),
  streams: z
    .array(
      z.strictObject({
        kind: z.string(),
        codec: z.string().nullable(),
        width: z.number().nullable(),
        height: z.number().nullable(),
        sampleRate: z.number().nullable(),
        channels: z.number().nullable(),
      })
    )
    .max(32),
  published: z.boolean(),
  outputBytes: z.number().nullable(),
  outputSha256: z.string().nullable(),
})
