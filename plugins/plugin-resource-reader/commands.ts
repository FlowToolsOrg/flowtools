import type { ToolContext } from '@flowtools/sdk/types'

import {
  commandManifestV2Schema,
  exportOperationSchema,
} from '@flowtools/sdk/manifest'
import { result } from '@flowtools/sdk/result'
import { z } from 'zod'
export const inputSchema = z.object({})
export const outputSchema = z.strictObject({
  type: z.literal('json'),
  value: z.strictObject({
    result: z.strictObject({
      kind: z.enum(['file', 'directory']),
      bytes: z.number().int().min(0),
      entries: z.number().int().min(0),
    }),
  }),
  stdio: z.string().optional(),
})
const command = commandManifestV2Schema.parse({
  id: 'run',
  name: '读取所选资源',
  description: '分块读取文件或分页统计目录；结果仅含计数',
  capabilityProfile: 'flowtools-native-v2',
  inputSchema: exportOperationSchema(inputSchema),
  outputSchema: exportOperationSchema(outputSchema),
  runtimeValidation: { input: 'required', output: 'required' },
  headless: true,
  supportsColdStart: false,
  interaction: 'optional',
  effects: ['file-read'],
  permissions: [
    {
      capability: 'fs',
      operations: ['read', 'stat', 'list'],
      scopes: [{ kind: 'resource', slot: 'source' }],
    },
  ],
  resources: { timeoutMs: 30000, maxInputBytes: 1024, maxOutputBytes: 4096 },
})
export default {
  type: 'app' as const,
  meta: {
    id: 'plugin-resource-reader',
    name: '资源读取',
    description: '授权后读取所选文件或统计目录',
    version: '0.0.1',
    maturity: 'prototype' as const,
    permissions: ['fs'] as const,
  },
  inputSchema,
  outputSchema,
  command,
  async run(ctx: ToolContext) {
    if (!ctx.resources) throw new Error('CAPABILITY_UNAVAILABLE')
    const source = await ctx.resources.resolve('source', { signal: ctx.signal })
    let bytes = 0,
      entries = 0
    try {
      if (source.kind === 'file') {
        let offset = 0
        for (;;) {
          const chunk = await ctx.resources.read(source.ref, {
            revision: source.revision,
            offset,
            maxBytes: 65536,
            signal: ctx.signal,
          })
          bytes += chunk.bytes.length
          offset = chunk.nextOffset
          if (chunk.eof) break
        }
      } else {
        let cursor: string | undefined
        do {
          const page = await ctx.resources.list(source.ref, {
            revision: source.revision,
            cursor,
            limit: 8,
            signal: ctx.signal,
          })
          entries += page.entries.length
          for (const child of page.entries)
            await ctx.resources.release(child.ref, { signal: ctx.signal })
          cursor = page.nextCursor ?? undefined
        } while (cursor)
      }
      return result.json({ result: { kind: source.kind, bytes, entries } })
    } finally {
      if (!ctx.signal.aborted)
        await ctx.resources.release(source.ref, { signal: ctx.signal })
    }
  },
}
