import { afterEach, expect, test } from 'bun:test'
import { createHash } from 'node:crypto'
import {
  mkdirSync,
  mkdtempSync,
  writeFileSync,
  rmSync,
  rmdirSync,
  realpathSync,
  symlinkSync,
} from 'node:fs'
import { dirname, join, resolve } from 'node:path'

import { verifyManifestPackage } from '../src/manifest/package'

import { manifestFixture, manifestTarget } from './fixtures/manifest-v1'

const parent = resolve(import.meta.dir, '../node_modules/.tmp')
mkdirSync(parent, { recursive: true })
const roots: string[] = []
const fixture = () => {
  const root = mkdtempSync(join(parent, 'manifest-package-'))
  roots.push(root)
  mkdirSync(join(root, 'dist'))
  const bytes = 'throw new Error("CODE_MUST_NOT_RUN")\n'
  writeFileSync(join(root, 'dist/commands.js'), bytes)
  const manifest = manifestFixture()
  manifest.files[0] = {
    path: 'dist/commands.js',
    size: Buffer.byteLength(bytes),
    sha256: createHash('sha256').update(bytes).digest('hex'),
  }
  return { root, manifest }
}
afterEach(() => {
  for (const root of roots.splice(0)) {
    const canonical = realpathSync(root)
    expect(dirname(canonical)).toBe(realpathSync(parent))
    rmSync(canonical, { recursive: true })
  }
})

test('package verification hashes actual bytes without importing code', () => {
  const { root, manifest } = fixture()
  expect(verifyManifestPackage(root, manifest, manifestTarget)).toEqual(
    manifest
  )
})
for (const kind of ['tampered', 'missing', 'extra', 'junction'] as const)
  test(`package verification refuses ${kind} before execution`, () => {
    const { root, manifest } = fixture()
    if (kind === 'tampered')
      writeFileSync(join(root, 'dist/commands.js'), 'malicious')
    if (kind === 'missing') rmSync(join(root, 'dist/commands.js'))
    if (kind === 'extra') writeFileSync(join(root, 'extra.js'), 'throw 1')
    if (kind === 'junction') {
      const outside = fixture().root
      rmSync(join(root, 'dist/commands.js'))
      rmdirSync(join(root, 'dist'))
      symlinkSync(join(outside, 'dist'), join(root, 'dist'), 'junction')
    }
    expect(() =>
      verifyManifestPackage(root, manifest, manifestTarget)
    ).toThrow()
  })
