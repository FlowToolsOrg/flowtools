import type { CommandManifestV1 } from '@flowtools/sdk/manifest'
import type { CommandResult } from '@flowtools/sdk/result'
import type { ToolContext, PluginMeta } from '@flowtools/sdk/types'

import {
  commandManifestSchema,
  exportOperationSchema,
  operationSchema,
} from '@flowtools/sdk/manifest'
import { z } from 'zod'

const text = z.strictObject({
  type: z.literal('text'),
  text: z.string(),
  stdio: z.string().optional(),
})
const strings = z.array(z.string())
const count = z.number().int().min(0)
const json = <T extends z.ZodType>(value: T) =>
  z.strictObject({
    type: z.literal('json'),
    value,
    stdio: z.string().optional(),
  })
const outputById: Record<string, z.ZodType> = {
  'plugin-base64-encoder': json(
    z.strictObject({
      result: z.string(),
      mode: z.enum(['encode', 'decode']),
      input: z.string(),
    })
  ),
  'plugin-color-converter': json(
    z.strictObject({
      result: z.strictObject({
        hex: z.string(),
        rgb: z.string(),
        hsl: z.string(),
      }),
    })
  ),
  'plugin-hash-generator': json(
    z.strictObject({
      result: z.string(),
      algorithm: z.enum(['SHA-1', 'SHA-256', 'SHA-384', 'SHA-512']),
      bytes: count,
    })
  ),
  'plugin-image-base64': json(
    z.strictObject({
      result: z.strictObject({
        dataUrl: z.string(),
        estimatedBytes: count,
        hasPrefix: z.boolean(),
      }),
    })
  ),
  'plugin-json-formatter': json(
    z.strictObject({
      result: z.strictObject({
        valid: z.boolean(),
        error: z.string().optional(),
      }),
    })
  ),
  'plugin-random-picker': json(
    z.strictObject({ result: strings, total: count })
  ),
  'plugin-regex-tester': json(
    z.strictObject({
      result: z.array(
        z.strictObject({
          match: z.string(),
          index: count,
          groups: z.record(z.string(), z.string()).optional(),
        })
      ),
      pattern: z.string(),
      flags: z.string(),
      matchCount: count,
    })
  ),
  'plugin-text-ops': json(
    z.strictObject({
      result: strings,
      operation: z.enum(['intersection', 'union', 'difference']),
      count,
    })
  ),
  'plugin-timestamp-converter': json(
    z.strictObject({
      result: z.union([z.string(), z.number()]),
      timestamp: z.string().optional(),
      date: z.string().optional(),
      iso: z.string().optional(),
      unit: z.enum(['seconds', 'milliseconds']),
    })
  ),
  'plugin-todo-list': json(
    z.union([
      z.strictObject({
        result: z.strictObject({ added: z.string(), total: count }),
      }),
      z.strictObject({ result: strings, count }),
    ])
  ),
  'plugin-uuid-generator': json(z.strictObject({ result: strings, count })),
  'plugin-website-latency': json(
    z.strictObject({
      result: strings,
      average: z.number().nullable(),
      tested: count,
    })
  ),
}

export function defineBuiltinCommand<T>(plugin: {
  type: 'app'
  meta: PluginMeta
  inputSchema: z.ZodType<T>
  run: (ctx: ToolContext, input: T) => CommandResult | Promise<CommandResult>
}) {
  const output = outputById[plugin.meta.id]
  if (!output) throw new Error('Unknown built-in command output contract')
  const outputSchema = z.union([text, output])
  let input: z.ZodType = plugin.inputSchema
  // URL syntax has an explicit runtime validator; v1 advertises the serializable
  // string/array shape, rather than claiming to export URL format semantics.
  if (plugin.meta.id === 'plugin-website-latency')
    input = z.object({
      urls: z
        .array(z.string())
        .optional()
        .describe('URLs to test; runtime validates URL syntax'),
    })
  let outputContract
  // Named regex captures have open string keys; retained by the output validator.
  if (plugin.meta.id === 'plugin-regex-tester') {
    const exported = z.toJSONSchema(outputSchema, { target: 'draft-7' })
    const removeRecord = (value: unknown): void => {
      if (!value || typeof value !== 'object') return
      const node = value as Record<string, unknown>
      if (
        node.type === 'object' &&
        node.additionalProperties &&
        typeof node.additionalProperties === 'object'
      ) {
        delete node.propertyNames
        node.additionalProperties = true
      }
      Object.values(node).forEach(removeRecord)
    }
    removeRecord(exported)
    delete exported.$schema
    outputContract = operationSchema.parse(JSON.parse(JSON.stringify(exported)))
  } else outputContract = exportOperationSchema(outputSchema)
  const network = plugin.meta.id === 'plugin-website-latency'
  const storage = plugin.meta.id === 'plugin-todo-list'
  const command: CommandManifestV1 = commandManifestSchema.parse({
    id: 'run',
    name: `执行${plugin.meta.name}`,
    description: plugin.meta.description ?? `执行${plugin.meta.name}的默认操作`,
    inputSchema: exportOperationSchema(input),
    outputSchema: outputContract,
    runtimeValidation: { input: 'required', output: 'required' },
    headless: true,
    supportsColdStart: !network,
    interaction: 'none',
    effects: network
      ? ['network-read']
      : storage
        ? ['data-read', 'data-write']
        : [],
    permissions: network
      ? [
          {
            capability: 'network',
            operations: ['request'],
            scopes: ['input:urls', 'built-in-default-sites'],
          },
        ]
      : storage
        ? [
            {
              capability: 'storage',
              operations: ['get', 'set'],
              scopes: ['plugin-data'],
            },
          ]
        : [],
    resources: {
      timeoutMs: 30_000,
      maxInputBytes: 1_048_576,
      maxOutputBytes: 1_048_576,
    },
  })
  return {
    ...plugin,
    inputSchema: plugin.inputSchema as z.ZodObject<z.ZodRawShape>,
    outputSchema,
    command,
  }
}
