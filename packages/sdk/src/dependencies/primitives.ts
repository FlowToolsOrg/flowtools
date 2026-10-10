import { valid, validRange } from 'semver'
import { z } from 'zod'

// The Host npm parser has a narrower numeric domain than JavaScript semver.
// Bound explicit core and purely numeric prerelease identifiers, leaving
// alphanumeric prerelease/build strings and synthetic upper bounds intact.
const MAX_VERSION_NUMBER = 900_719_925_474_099n
function supportedNumericDomain(value: string): boolean {
  const versions = value.matchAll(
    /(?:^|[\s|<>=~^])v?([0-9xX*]+(?:\.[0-9xX*]+){0,2})(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?/g
  )
  for (const version of versions) {
    const identifiers = [
      ...version[1]!.split('.'),
      ...(version[2]?.split('.') ?? []),
    ]
    if (
      identifiers.some(
        identifier =>
          /^\d+$/.test(identifier) && BigInt(identifier) > MAX_VERSION_NUMBER
      )
    )
      return false
  }
  return true
}

export const dependencyIdSchema = z
  .string()
  .max(128)
  .regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/)
export const dependencyVersionSchema = z
  .string()
  .max(128)
  .refine(
    value => valid(value) === value && supportedNumericDomain(value),
    'Expected canonical semver in the supported numeric domain'
  )
export const dependencyRangeSchema = z
  .string()
  .min(1)
  .max(256)
  .refine(
    value => validRange(value) !== null && supportedNumericDomain(value),
    'Expected semver range in the supported numeric domain'
  )
export const dependencyTargetSchema = z.strictObject({
  platform: z.enum(['windows', 'macos', 'linux', 'web']),
  arch: z.enum(['x64', 'arm64', 'wasm32']),
})
export const dependencyDigestSchema = z.string().regex(/^[a-f0-9]{64}$/)
export const dependencyBuildFlavorSchema = dependencyIdSchema

export const dependencyIdentityShape = {
  publisher: dependencyIdSchema,
  id: dependencyIdSchema,
  version: dependencyRangeSchema,
}
