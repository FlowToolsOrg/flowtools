import { satisfies, valid } from 'semver'
import { z } from 'zod'

import { portablePluginPathSchema } from '../compat/catalog'
import {
  dependencyIdSchema,
  dependencyVersionSchema,
  dependencyRangeSchema,
  dependencyTargetSchema,
} from '../dependencies/primitives'
import {
  dependencyDeclarationsSchema,
  serviceDefinitionsSchema,
} from '../dependencies/schema'
import { pluginMaturitySchema } from '../types/maturity'

import { commandManifestSchema, type CommandManifestV1 } from './command'
import { isJsonValue } from './json-schema'

export { commandManifestSchema, type CommandManifestV1 } from './command'
export const manifestIdSchema = dependencyIdSchema
const version = dependencyVersionSchema
const range = dependencyRangeSchema
const unique = <T>(values: T[]) => new Set(values).size === values.length

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
    targets: z.array(dependencyTargetSchema).min(1).max(16),
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
    services: serviceDefinitionsSchema.optional(),
    dependencies: dependencyDeclarationsSchema,
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
    // Omitted definitions preserve old Manifest bytes and legacy metadata.
    // Explicit definitions must describe exactly the dedicated services entry.
    if (
      manifest.services !== undefined &&
      manifest.services.length > 0 !== Boolean(manifest.entries.services)
    )
      fail('Service definitions must match the services entry')
    if (
      entries.some(entry => !manifest.files.some(file => file.path === entry))
    )
      fail('Entry is missing from package file inventory')
  })

export type PluginManifestV1 = z.infer<typeof pluginManifestSchema>
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
