import { satisfies, valid, validRange } from 'semver'
import { z } from 'zod'

import { portablePluginPathSchema } from '../compat/catalog'
import { pluginMaturitySchema } from '../types/maturity'

import { isJsonValue, operationSchema } from './json-schema'

export const manifestIdSchema = z
  .string()
  .max(128)
  .regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/)
const version = z
  .string()
  .max(128)
  .refine(value => valid(value) === value, 'Expected canonical semver')
const range = z
  .string()
  .min(1)
  .max(256)
  .refine(value => validRange(value) !== null, 'Expected semver range')
const unique = <T>(values: T[]) => new Set(values).size === values.length
const dependency = z.strictObject({
  publisher: manifestIdSchema,
  id: manifestIdSchema,
  version: range,
})

export const commandManifestSchema = z
  .strictObject({
    id: manifestIdSchema,
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
          operations: z.array(manifestIdSchema).min(1).max(32).refine(unique),
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

export const pluginManifestSchema = z
  .strictObject({
    formatVersion: z.literal(1),
    publisher: manifestIdSchema,
    id: manifestIdSchema,
    name: z.string().min(1).max(256),
    description: z.string().min(1).max(4096),
    version,
    type: z.enum(['app', 'tool']),
    maturity: pluginMaturitySchema,
    engines: z.strictObject({ host: range, sdk: range }),
    targets: z
      .array(
        z.strictObject({
          platform: z.enum(['windows', 'macos', 'linux', 'web']),
          arch: z.enum(['x64', 'arm64', 'wasm32']),
        })
      )
      .min(1)
      .max(16),
    entries: z.strictObject({
      ui: portablePluginPathSchema.optional(),
      executor: portablePluginPathSchema.optional(),
      services: portablePluginPathSchema.optional(),
    }),
    files: z
      .array(
        z.strictObject({
          path: portablePluginPathSchema,
          sha256: z.string().regex(/^[a-f0-9]{64}$/),
          size: z.number().int().min(0).max(67_108_864),
        })
      )
      .min(1)
      .max(1024),
    commands: z.array(commandManifestSchema).max(256),
    dependencies: z.strictObject({
      services: z.array(dependency).max(128),
      tools: z.array(dependency).max(128),
    }),
    // Signing protocol/trust roots are deliberately deferred to P2.5a.
    signature: z.strictObject({ status: z.literal('unsigned') }),
  })
  .superRefine((manifest, ctx) => {
    const fail = (message: string) => ctx.addIssue({ code: 'custom', message })
    if (!unique(manifest.commands.map(command => command.id)))
      fail('Duplicate command ID')
    const paths = manifest.files.map(file => file.path.toLowerCase())
    if (manifest.files.reduce((size, file) => size + file.size, 0) > 67_108_864)
      fail('Package exceeds the v1 file budget')
    if (!unique(paths)) fail('Duplicate package path')
    if (
      !unique(
        manifest.targets.map(target => `${target.platform}/${target.arch}`)
      )
    )
      fail('Duplicate target')
    const entries = Object.values(manifest.entries)
    if (!entries.length || !unique(entries))
      fail('Entries must be present and distinct')
    if (manifest.commands.length && !manifest.entries.executor)
      fail('Commands require an executor entry')
    if (
      entries.some(entry => !manifest.files.some(file => file.path === entry))
    )
      fail('Entry is missing from package file inventory')
    for (const dependencies of Object.values(manifest.dependencies))
      if (!unique(dependencies.map(item => `${item.publisher}/${item.id}`)))
        fail('Duplicate dependency identity')
  })

export type PluginManifestV1 = z.infer<typeof pluginManifestSchema>
export type CommandManifestV1 = z.infer<typeof commandManifestSchema>
export interface ManifestTarget {
  hostVersion: string
  sdkVersion: string
  platform: PluginManifestV1['targets'][number]['platform']
  arch: PluginManifestV1['targets'][number]['arch']
}

export function parsePluginManifest(
  value: unknown,
  target?: ManifestTarget
): PluginManifestV1 {
  // Inspect bounded JSON before touching recursively parsed schema fields.
  if (!isManifestJson(value))
    throw new Error('Manifest must contain bounded JSON data')
  const manifest = pluginManifestSchema.parse(value)
  if (
    target &&
    (!valid(target.hostVersion) ||
      !valid(target.sdkVersion) ||
      !satisfies(target.hostVersion, manifest.engines.host) ||
      !satisfies(target.sdkVersion, manifest.engines.sdk) ||
      !manifest.targets.some(
        item => item.platform === target.platform && item.arch === target.arch
      ))
  )
    throw new Error('Manifest is incompatible with this Host/SDK target')
  return manifest
}

function isManifestJson(value: unknown): boolean {
  return isJsonValue(value) && JSON.stringify(value).length <= 1_048_576
}

export function commandIdentity(
  manifest: Pick<PluginManifestV1, 'publisher' | 'id'>,
  command: Pick<CommandManifestV1, 'id'>
): string {
  return [
    manifestIdSchema.parse(manifest.publisher),
    manifestIdSchema.parse(manifest.id),
    manifestIdSchema.parse(command.id),
  ].join('/')
}
