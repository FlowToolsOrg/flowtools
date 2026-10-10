import { expect, test } from 'bun:test'
import { createHash, createPublicKey, verify } from 'node:crypto'
import { fileURLToPath } from 'node:url'

import { z } from 'zod'

import { parsePluginManifest } from '../src/manifest/schema'
import {
  MAX_PACKAGE_PAYLOAD_BYTES,
  PACKAGE_PAYLOAD_TYPE,
  ROOT_PAYLOAD_TYPE,
  packageDescriptorSchema,
  packagePreAuthenticationEncoding,
  serializePackageDescriptor,
} from '../src/manifest/signed-package'

import fixture from './fixtures/signed-package-v1.json'

const payload = Buffer.from(fixture.packageEnvelope.payload, 'base64')
const sha256 = (bytes: Uint8Array) =>
  createHash('sha256').update(bytes).digest('hex')

test('Node-signed golden pins exact bytes, manifest, ZIP and package identity', () => {
  expect(serializePackageDescriptor(fixture.descriptor)).toEqual(
    new Uint8Array(payload)
  )
  const pae = packagePreAuthenticationEncoding(PACKAGE_PAYLOAD_TYPE, payload)
  expect(Buffer.from(pae).toString('base64')).toBe(fixture.paeBase64)
  expect(sha256(payload)).toBe(fixture.descriptorDigest)
  const rootBytes = Buffer.from(fixture.rootEnvelope.payload, 'base64')
  expect(sha256(rootBytes)).toBe(fixture.rootDigest)
  const rootInput: unknown = JSON.parse(rootBytes.toString('utf8'))
  const root = z
    .object({
      publishers: z.array(
        z.object({ keys: z.array(z.object({ publicKey: z.string() })) })
      ),
    })
    .parse(rootInput)
  const rawPublicKey = Buffer.from(
    root.publishers[0]!.keys[0]!.publicKey,
    'base64'
  )
  const publicKey = createPublicKey({
    key: Buffer.concat([
      Buffer.from('302a300506032b6570032100', 'hex'),
      rawPublicKey,
    ]),
    type: 'spki',
    format: 'der',
  })
  const signature = Buffer.from(
    fixture.packageEnvelope.signatures[0]!.sig,
    'base64'
  )
  expect(verify(null, pae, publicKey, signature)).toBe(true)
  expect(
    verify(
      null,
      packagePreAuthenticationEncoding(ROOT_PAYLOAD_TYPE, payload),
      publicKey,
      signature
    )
  ).toBe(false)
  expect(
    verify(
      null,
      packagePreAuthenticationEncoding(
        PACKAGE_PAYLOAD_TYPE,
        Buffer.concat([payload, Buffer.from(' ')])
      ),
      publicKey,
      signature
    )
  ).toBe(false)
  const manifestBytes = Buffer.from(fixture.manifestBase64, 'base64')
  expect(sha256(manifestBytes)).toBe(fixture.descriptor.manifest.sha256)
  expect(manifestBytes.length).toBe(fixture.descriptor.manifest.size)
  const manifest = parsePluginManifest(
    JSON.parse(manifestBytes.toString('utf8'))
  )
  expect(manifest.files).toEqual(fixture.descriptor.files)
  expect(manifest.publisher).toBe(fixture.descriptor.publisher)
  expect(manifest.id).toBe(fixture.descriptor.id)
  const archive = Buffer.from(fixture.archiveBase64, 'base64')
  expect(sha256(archive)).toBe(fixture.descriptor.archive.sha256)
  expect(sha256(archive)).toBe(fixture.packageDigest)
  expect(archive.length).toBe(fixture.descriptor.archive.size)
})

test('PAE uses exact payload length and leaves bytes intact', () => {
  const utf8 = new TextEncoder().encode('签名\r\n')
  const result = packagePreAuthenticationEncoding(PACKAGE_PAYLOAD_TYPE, utf8)
  expect(Buffer.from(result).toString('utf8')).toBe(
    `DSSEv1 ${PACKAGE_PAYLOAD_TYPE.length} ${PACKAGE_PAYLOAD_TYPE} 8 签名\r\n`
  )
  expect(() =>
    packagePreAuthenticationEncoding(
      PACKAGE_PAYLOAD_TYPE,
      new Uint8Array(MAX_PACKAGE_PAYLOAD_BYTES + 1)
    )
  ).toThrow()
})

for (const path of [
  '../escape.js',
  '/absolute.js',
  'c:/escape.js',
  'dist\\escape.js',
  'dist//file.js',
  'dist/./file.js',
  'dist/file.js:stream',
  'dist/file.js.',
  'dist/con.js',
  'dist/lpt1',
  'dist/com0.txt',
  'dist/FILE.js',
  'dist/%2e.js',
  'manifest.json',
  'manifest.json/child.js',
  'dist/汉字.js',
])
  test(`authoring rejects unsafe or ambiguous path ${path}`, () => {
    const value = structuredClone(fixture.descriptor)
    value.files[0]!.path = path
    expect(() => serializePackageDescriptor(value)).toThrow()
  })

test('authoring rejects unknown fields, expiry, oversized and conflicting inventory', () => {
  expect(
    packageDescriptorSchema.safeParse({
      ...fixture.descriptor,
      certified: true,
    }).success
  ).toBe(false)
  for (const value of [
    { ...fixture.descriptor, formatVersion: 2 },
    { ...fixture.descriptor, releaseSequence: 0 },
    { ...fixture.descriptor, version: '01.0.0' },
    { ...fixture.descriptor, version: '1.0.0+changed' },
    { ...fixture.descriptor, issuedAt: 0, expiresAt: 2_592_001 },
    { ...fixture.descriptor, expiresAt: fixture.descriptor.issuedAt },
    {
      ...fixture.descriptor,
      files: [...fixture.descriptor.files, ...fixture.descriptor.files],
    },
    {
      ...fixture.descriptor,
      files: [{ ...fixture.descriptor.files[0], size: 67_108_865 }],
    },
    {
      ...fixture.descriptor,
      files: [
        { ...fixture.descriptor.files[0], path: 'dist' },
        ...fixture.descriptor.files,
      ],
    },
  ])
    expect(() => serializePackageDescriptor(value)).toThrow()
})

test('published authoring helpers import in Node without React or execution services', async () => {
  const script = `import {serializePackageDescriptor} from './dist/manifest.js'; const fixture = JSON.parse(await (await import('node:fs/promises')).readFile('./test/fixtures/signed-package-v1.json', 'utf8')); if (serializePackageDescriptor(fixture.descriptor).length !== Buffer.from(fixture.packageEnvelope.payload,'base64').length) process.exit(1)`
  const child = Bun.spawn(['node', '--input-type=module', '-e', script], {
    cwd: fileURLToPath(new URL('..', import.meta.url)),
    stdout: 'pipe',
    stderr: 'pipe',
  })
  expect(await new Response(child.stderr).text()).toBe('')
  expect(await child.exited).toBe(0)
}, 30_000)
