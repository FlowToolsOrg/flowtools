import { z } from 'zod'

import { manifestIdSchema } from './schema'

export const PACKAGE_PAYLOAD_TYPE = 'application/vnd.flowtools.package.v1+json'
export const ROOT_PAYLOAD_TYPE = 'application/vnd.flowtools.trust-root.v1+json'
export const MAX_PACKAGE_PAYLOAD_BYTES = 1_048_576

const MAX_PACKAGE_BYTES = 67_108_864
const MAX_SAFE_INTEGER = 9_007_199_254_740_991
const uint = z.number().int().min(0).max(MAX_SAFE_INTEGER)
const digest = z.string().regex(/^[a-f0-9]{64}$/)
const version = z
  .string()
  .max(128)
  .regex(/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/)

export const signedPackageTargetSchema = z.strictObject({
  platform: z.enum(['windows', 'macos', 'linux']),
  arch: z.enum(['x64', 'arm64']),
})

// The archive profile deliberately uses lowercase ASCII names on every platform.
export const signedPackagePathSchema = z
  .string()
  .min(1)
  .max(240)
  .refine(path => {
    if (!/^[a-z0-9._/-]+$/.test(path)) return false
    const segments = path.split('/')
    return segments.every(
      segment =>
        segment.length > 0 &&
        segment !== '.' &&
        segment !== '..' &&
        !segment.endsWith('.') &&
        !/^(?:con|prn|aux|nul|com[0-9]|lpt[0-9])(?:\.|$)/.test(segment)
    )
  }, 'Expected a portable package file path')

const packageFileSchema = z.strictObject({
  path: signedPackagePathSchema,
  size: uint.max(MAX_PACKAGE_BYTES),
  sha256: digest,
})

/** Authoring contract only. Rust verifies trust; this schema creates no grant. */
export const packageDescriptorSchema = z
  .strictObject({
    formatVersion: z.literal(1),
    kind: z.enum(['plugin', 'tool']),
    publisher: manifestIdSchema,
    id: manifestIdSchema,
    version,
    releaseSequence: uint.min(1),
    target: signedPackageTargetSchema,
    issuedAt: uint,
    expiresAt: uint,
    manifest: z.strictObject({
      sha256: digest,
      size: uint.min(1).max(MAX_PACKAGE_PAYLOAD_BYTES),
    }),
    archive: z.strictObject({
      format: z.literal('zip'),
      sha256: digest,
      size: uint.min(1).max(MAX_PACKAGE_BYTES),
    }),
    files: z.array(packageFileSchema).min(1).max(1024),
    source: z.strictObject({
      registry: manifestIdSchema,
      artifact: manifestIdSchema,
    }),
    license: z
      .string()
      .min(1)
      .max(128)
      .regex(/^[A-Za-z0-9][A-Za-z0-9.+-]*$/),
    buildFlavor: manifestIdSchema,
  })
  .superRefine((descriptor, context) => {
    const fail = (message: string) =>
      context.addIssue({ code: 'custom', message })
    if (
      descriptor.expiresAt <= descriptor.issuedAt ||
      descriptor.expiresAt - descriptor.issuedAt > 2_592_000
    )
      fail('Package validity must be positive and at most 30 days')
    if (
      descriptor.files.reduce((size, file) => size + file.size, 0) >
      MAX_PACKAGE_BYTES
    )
      fail('Package exceeds the expanded file budget')
    const paths = descriptor.files.map(file => file.path)
    if (
      new Set(paths).size !== paths.length ||
      paths.some(
        path => path === 'manifest.json' || path.startsWith('manifest.json/')
      )
    )
      fail('Duplicate or reserved package path')
    const pathSet = new Set(paths)
    if (
      paths.some(path =>
        path
          .split('/')
          .slice(0, -1)
          .some((_, index) =>
            pathSet.has(
              path
                .split('/')
                .slice(0, index + 1)
                .join('/')
            )
          )
      )
    )
      fail('File and directory paths conflict')
  })

export type PackageDescriptorV1 = z.infer<typeof packageDescriptorSchema>

/** Serialize once, then sign and transmit these exact UTF-8 bytes. */
export function serializePackageDescriptor(value: unknown): Uint8Array {
  const descriptor = packageDescriptorSchema.parse(value)
  const bytes = new TextEncoder().encode(JSON.stringify(descriptor))
  if (bytes.byteLength > MAX_PACKAGE_PAYLOAD_BYTES)
    throw new Error('Package descriptor exceeds the payload budget')
  return bytes
}

/** DSSE 1.0.2 PAE. No hashing, verification, IO or code loading occurs here. */
export function packagePreAuthenticationEncoding(
  payloadType: typeof PACKAGE_PAYLOAD_TYPE | typeof ROOT_PAYLOAD_TYPE,
  payload: Uint8Array
): Uint8Array {
  if (payloadType !== PACKAGE_PAYLOAD_TYPE && payloadType !== ROOT_PAYLOAD_TYPE)
    throw new Error('Unsupported package payload type')
  if (payload.byteLength > MAX_PACKAGE_PAYLOAD_BYTES)
    throw new Error('Package payload exceeds the budget')
  const encoder = new TextEncoder()
  const typeBytes = encoder.encode(payloadType)
  const prefix = encoder.encode(
    `DSSEv1 ${typeBytes.byteLength} ${payloadType} ${payload.byteLength} `
  )
  const result = new Uint8Array(prefix.byteLength + payload.byteLength)
  result.set(prefix)
  result.set(payload, prefix.byteLength)
  return result
}
