import { valid } from 'semver'
import { z } from 'zod'

import { isJsonValue, type JsonValue } from '../manifest/json-schema'

import { boundedJsonBytes } from './json-budget'
import {
  ExtensionContributionError,
  type ExtensionContributionDocument,
  type ExtensionContributionOwner,
} from './types'

/** Fixed prototype budgets, including retained owner-generation metadata. */
export const extensionContributionLimits = Object.freeze({
  maxContributionsPerOwner: 128,
  maxValueBytes: 65_536,
  maxDocumentBytes: 262_144,
  maxOwners: 128,
  maxContributions: 1024,
  maxRegistryBytes: 1_048_576,
})

const id = z
  .string()
  .max(128)
  .regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/)

const documentSchema = z.strictObject({
  formatVersion: z.literal(1),
  contributions: z
    .array(
      z.strictObject({
        id,
        kind: z.enum(['theme', 'locale', 'settings']),
        value: z.custom<JsonValue>(isJsonValue, 'Expected required JSON data'),
      })
    )
    .max(extensionContributionLimits.maxContributionsPerOwner),
})

const ownerSchema = z.strictObject({
  pluginId: id,
  version: z
    .string()
    .max(128)
    .refine(value => valid(value) === value),
  generation: z.number().int().min(1).max(Number.MAX_SAFE_INTEGER),
})

export function jsonBytes(value: unknown): number {
  return new TextEncoder().encode(JSON.stringify(value)).length
}

function freezeJson<T>(value: T): T {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value)) freezeJson(child)
    Object.freeze(value)
  }
  return value
}

function canonicalJson(value: JsonValue): JsonValue {
  if (Array.isArray(value)) return value.map(canonicalJson)
  if (value && typeof value === 'object')
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map(key => [key, canonicalJson(value[key]!)])
    )
  return value
}

/** Refuse executable/accessor data before schema reads or serialization. */
export function parseExtensionContributions(
  value: unknown
): ExtensionContributionDocument {
  if (!isJsonValue(value))
    throw new ExtensionContributionError('INVALID_DOCUMENT')
  boundedJsonBytes(value, extensionContributionLimits.maxDocumentBytes)
  const parsed = documentSchema.safeParse(value)
  if (!parsed.success) throw new ExtensionContributionError('INVALID_DOCUMENT')
  const ids = new Set<string>()
  for (const contribution of parsed.data.contributions) {
    if (ids.has(contribution.id))
      throw new ExtensionContributionError('INVALID_DOCUMENT')
    ids.add(contribution.id)
    boundedJsonBytes(
      contribution.value,
      extensionContributionLimits.maxValueBytes
    )
  }
  // The initial JSON guard excludes getters/toJSON and lossy serialization.
  return freezeJson(
    JSON.parse(
      JSON.stringify(canonicalJson(parsed.data))
    ) as ExtensionContributionDocument
  )
}

export function parseContributionOwner(
  value: unknown
): ExtensionContributionOwner {
  if (!isJsonValue(value)) throw new ExtensionContributionError('INVALID_OWNER')
  const parsed = ownerSchema.safeParse(value)
  if (!parsed.success) throw new ExtensionContributionError('INVALID_OWNER')
  return Object.freeze(parsed.data)
}
