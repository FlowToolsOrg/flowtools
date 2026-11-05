import { satisfies, valid } from 'semver'
import { z } from 'zod'

import {
  dependencyIdSchema,
  dependencyVersionSchema,
} from '../dependencies/primitives'

import { commandManifestSchema } from './command'
import { isJsonValue } from './json-schema'
import {
  parsePluginManifest,
  pluginManifestSchema,
  type ManifestTarget,
  type PluginManifestV1,
} from './schema'

export const NATIVE_CAPABILITY_PROFILE = 'flowtools-native-v2' as const
const unique = <T>(values: readonly T[]) =>
  new Set(values).size === values.length
const segment = z.string().regex(/^[A-Za-z0-9_-]{1,128}$/)
const key = z.string().regex(/^[A-Za-z0-9_-]{1,128}$/)
// Same exact-origin domain as v1: this is not DNS/connection enforcement.
const origin = z
  .string()
  .regex(/^https:\/\/[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?:[1-9][0-9]{0,4}$/)
  .refine(value => {
    const authority = value.slice(8)
    const [host, port] = authority.split(':')
    return (
      Number(port) <= 65535 &&
      host!.includes('.') &&
      !/^[0-9.]+$/.test(host!) &&
      host!
        .split('.')
        .every(
          label =>
            label.length > 0 &&
            label.length <= 63 &&
            !label.startsWith('-') &&
            !label.endsWith('-')
        )
    )
  })
export const nativeHttpMethodSchema = z.enum([
  'GET',
  'HEAD',
  'POST',
  'PUT',
  'PATCH',
  'DELETE',
])
const pathPrefix = z.array(segment).max(16)
const methods = z.array(nativeHttpMethodSchema).min(1).max(6).refine(unique)

export const nativeScopeV2Schema = z.discriminatedUnion('kind', [
  z.strictObject({ kind: z.literal('resource'), slot: dependencyIdSchema }),
  z.strictObject({
    kind: z.literal('endpoint'),
    slot: dependencyIdSchema,
    origin,
    methods,
    pathPrefix,
  }),
  z.strictObject({ kind: z.literal('url'), origin, pathPrefix }),
  z.strictObject({
    kind: z.literal('credential'),
    slot: dependencyIdSchema,
    endpoint: dependencyIdSchema,
    method: nativeHttpMethodSchema,
    header: z.enum(['authorization', 'x-api-key']),
  }),
  z.strictObject({ kind: z.literal('data'), keyPrefix: key }),
  z.strictObject({ kind: z.literal('clipboard') }),
  z.strictObject({ kind: z.literal('notification') }),
  z.strictObject({ kind: z.literal('tool'), lock: key, action: key }),
])
export type NativeScopeV2 = z.infer<typeof nativeScopeV2Schema>

export const nativePermissionV2Schema = z
  .strictObject({
    capability: z.enum([
      'fs',
      'network',
      'opener',
      'secrets',
      'storage',
      'clipboard',
      'notification',
      'tool',
    ]),
    operations: z.array(dependencyIdSchema).min(1).max(32),
    scopes: z.array(nativeScopeV2Schema).min(1).max(64),
  })
  .superRefine((permission, ctx) => {
    if (!validNativePermission(permission))
      ctx.addIssue({
        code: 'custom',
        message: 'Invalid or duplicate native permission scope/operation',
      })
  })
export type NativePermissionV2 = z.infer<typeof nativePermissionV2Schema>

const operationEffects: Readonly<
  Record<string, Readonly<Record<string, string>>>
> = {
  fs: {
    read: 'file-read',
    stat: 'file-read',
    list: 'file-read',
    create: 'file-create',
  },
  network: { request: 'network-read', send: 'network-send' },
  opener: {
    'open-url': 'opener-open-url',
    'open-resource': 'opener-open-resource',
    'reveal-resource': 'opener-reveal-resource',
  },
  secrets: { use: 'secret-use', describe: 'secret-use' },
  storage: { get: 'data-read', set: 'data-write' },
  clipboard: { read: 'clipboard-read', write: 'clipboard-write' },
  notification: { notify: 'notification' },
  tool: { execute: 'tool-execute' },
}
export function validNativePermission(permission: NativePermissionV2): boolean {
  const effects = operationEffects[permission.capability]!
  const kinds: Record<string, readonly string[]> = {
    fs: ['resource'],
    network: ['endpoint'],
    opener: ['url', 'resource'],
    secrets: ['credential'],
    storage: ['data'],
    clipboard: ['clipboard'],
    notification: ['notification'],
    tool: ['tool'],
  }
  return (
    unique(permission.operations) &&
    unique(permission.scopes.map(scope => JSON.stringify(scope))) &&
    permission.operations.every(operation =>
      Object.prototype.hasOwnProperty.call(effects, operation)
    ) &&
    permission.scopes.every(scope =>
      kinds[permission.capability]!.includes(scope.kind)
    ) &&
    permission.operations.every(operation =>
      permission.scopes.some(
        scope =>
          permission.capability !== 'opener' ||
          (operation === 'open-url'
            ? scope.kind === 'url'
            : scope.kind === 'resource')
      )
    ) &&
    permission.operations.every(
      operation =>
        permission.capability !== 'network' ||
        permission.scopes.some(
          scope =>
            scope.kind === 'endpoint' &&
            scope.methods.some(
              method =>
                (method === 'GET' || method === 'HEAD') ===
                (operation === 'request')
            )
        )
    ) &&
    permission.scopes.every(
      scope =>
        scope.kind !== 'endpoint' ||
        scope.methods.every(method =>
          permission.operations.includes(
            method === 'GET' || method === 'HEAD' ? 'request' : 'send'
          )
        )
    ) &&
    permission.scopes.every(
      scope =>
        permission.capability !== 'opener' ||
        permission.operations.some(operation =>
          scope.kind === 'url'
            ? operation === 'open-url'
            : operation !== 'open-url'
        )
    )
  )
}

export const commandManifestV2Schema = z
  .strictObject({
    ...commandManifestSchema.shape,
    capabilityProfile: z.literal(NATIVE_CAPABILITY_PROFILE),
    effects: z
      .array(
        z.enum([
          ...commandManifestSchema.shape.effects.element.options,
          'opener-open-url',
          'opener-open-resource',
          'opener-reveal-resource',
          'secret-use',
        ])
      )
      .max(32),
    permissions: z.array(nativePermissionV2Schema).max(32),
  })
  .superRefine((command, ctx) => {
    const fail = (message: string) => ctx.addIssue({ code: 'custom', message })
    if (command.name.length > 256 || command.description.length > 4096)
      fail('Command text exceeds UTF-16 bounds')
    if (
      !unique(command.effects) ||
      !unique(command.permissions.map(permission => permission.capability))
    )
      fail('Duplicate effect or capability')
    if (
      command.supportsColdStart &&
      (!command.headless || command.interaction === 'required')
    )
      fail('Cold start requires a headless command')
    if (command.inputSchema.type !== 'object')
      fail('Command input must be an object')
    const declared = new Set(
      command.permissions.flatMap(permission =>
        permission.operations.map(
          operation => operationEffects[permission.capability]?.[operation]
        )
      )
    )
    if (
      command.effects.some(effect => !declared.has(effect)) ||
      [...declared].some(
        effect =>
          !effect ||
          !command.effects.includes(effect as (typeof command.effects)[number])
      )
    )
      fail('Effects and native permission operations must match')
  })
export type CommandManifestV2 = z.infer<typeof commandManifestV2Schema>
export type CommandManifest =
  | import('./command').CommandManifestV1
  | CommandManifestV2

export const serviceDefinitionV2Schema = z
  .strictObject({
    id: dependencyIdSchema,
    version: dependencyVersionSchema,
    operations: z.array(commandManifestV2Schema).min(1).max(64),
  })
  .superRefine((service, ctx) => {
    if (
      !unique(service.operations.map(operation => operation.id)) ||
      service.operations.some(
        operation => !operation.headless || operation.interaction !== 'none'
      )
    )
      ctx.addIssue({ code: 'custom', message: 'Invalid service operations' })
  })
export const serviceDefinitionsV2Schema = z
  .array(serviceDefinitionV2Schema)
  .max(64)
  .refine(definitions => unique(definitions.map(definition => definition.id)))

export const pluginManifestV2Schema = z
  .strictObject({
    ...pluginManifestSchema.shape,
    formatVersion: z.literal(2),
    capabilityProfile: z.literal(NATIVE_CAPABILITY_PROFILE),
    commands: z.array(commandManifestV2Schema).max(256),
    services: serviceDefinitionsV2Schema.optional(),
  })
  .superRefine((manifest, ctx) => {
    const fail = (message: string) => ctx.addIssue({ code: 'custom', message })
    if (manifest.name.length > 256 || manifest.description.length > 4096)
      fail('Manifest text exceeds UTF-16 bounds')
    if (!unique(manifest.commands.map(command => command.id)))
      fail('Duplicate command ID')
    if (
      manifest.files.reduce((total, file) => total + file.size, 0) > 67_108_864
    )
      fail('Package exceeds file budget')
    if (!unique(manifest.files.map(file => file.path.toLowerCase())))
      fail('Duplicate package path')
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
      fail('Commands require an executor')
    if (
      manifest.services !== undefined &&
      manifest.services.length > 0 !== Boolean(manifest.entries.services)
    )
      fail('Service definitions must match the services entry')
    if (
      entries.some(entry => !manifest.files.some(file => file.path === entry))
    )
      fail('Entry is missing from inventory')
  })
export type PluginManifestV2 = z.infer<typeof pluginManifestV2Schema>
export type PluginManifest = PluginManifestV1 | PluginManifestV2

/** Explicit version dispatch. The original v1 parser and contract stay closed. */
export function parseVersionedPluginManifest(
  value: unknown,
  target?: ManifestTarget
): PluginManifest {
  if (!isJsonValue(value) || JSON.stringify(value).length > 1_048_576)
    throw new Error('Manifest must contain bounded JSON data')
  if (
    value &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    value.formatVersion === 1
  )
    return parsePluginManifest(value, target)
  const manifest = pluginManifestV2Schema.parse(value)
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

// Build-time export registers every custom check explicitly; it does not grant IO.
export const nativeV2Refinements: readonly (readonly [object, string])[] = [
  [origin, 'native-origin-v2'],
  [methods, 'unique'],
  [nativePermissionV2Schema, 'native-permission-v2'],
  [commandManifestV2Schema, 'native-command-v2'],
  [serviceDefinitionV2Schema, 'service-v1'],
  [serviceDefinitionsV2Schema, 'services-v1'],
  [pluginManifestV2Schema, 'manifest-v1'],
]
