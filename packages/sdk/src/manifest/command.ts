import { z } from 'zod'

import { dependencyIdSchema } from '../dependencies/primitives'

import { operationSchema } from './json-schema'

const unique = <T>(values: T[]) => new Set(values).size === values.length

/** Shared operation contract; service declarations do not add an execution grant. */
export const commandManifestSchema = z
  .strictObject({
    id: dependencyIdSchema,
    name: z.string().min(1).max(256),
    description: z.string().min(1).max(4096),
    inputSchema: operationSchema,
    outputSchema: operationSchema,
    runtimeValidation: z.strictObject({
      input: z.enum(['schema-only', 'required']),
      output: z.enum(['schema-only', 'required']),
    }),
    headless: z.boolean(),
    supportsColdStart: z.boolean(),
    interaction: z.enum(['none', 'optional', 'required']),
    effects: z
      .array(
        z.enum([
          'file-read',
          'file-create',
          'file-replace',
          'file-delete',
          'network-read',
          'network-send',
          'clipboard-read',
          'clipboard-write',
          'data-read',
          'data-write',
          'notification',
          'tool-execute',
        ])
      )
      .max(32)
      .refine(unique),
    permissions: z
      .array(
        z.strictObject({
          capability: z.enum([
            'fs',
            'network',
            'clipboard',
            'dialog',
            'notification',
            'storage',
            'db',
            'native',
            'tool',
          ]),
          operations: z.array(dependencyIdSchema).min(1).max(32).refine(unique),
          scopes: z.array(z.string().min(1).max(1024)).max(64).refine(unique),
        })
      )
      .max(32),
    resources: z.strictObject({
      timeoutMs: z.number().int().min(1).max(2_147_483_647),
      maxInputBytes: z.number().int().min(1).max(1_048_576),
      maxOutputBytes: z.number().int().min(1).max(16_777_216),
    }),
  })
  .superRefine((command, ctx) => {
    if (
      command.supportsColdStart &&
      (!command.headless || command.interaction === 'required')
    )
      ctx.addIssue({
        code: 'custom',
        message:
          'Cold start requires headless execution without required interaction',
      })
    if (command.inputSchema.type !== 'object')
      ctx.addIssue({
        code: 'custom',
        message: 'Command input must be an object schema',
      })
    if (!unique(command.permissions.map(permission => permission.capability)))
      ctx.addIssue({ code: 'custom', message: 'Duplicate capability request' })
  })

export type CommandManifestV1 = z.infer<typeof commandManifestSchema>
