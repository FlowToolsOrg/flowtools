import { createHash } from 'node:crypto'
import { lstatSync, readdirSync, readFileSync, realpathSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'

import { parsePluginManifest, type ManifestTarget } from './schema'

/** Read-only staging/T1 verification. Hashes are not publisher provenance or a grant. */
export function verifyManifestPackage(
  root: string,
  value: unknown,
  target: ManifestTarget
) {
  const manifest = parsePluginManifest(value, target)
  const absolute = resolve(root)
  // Refuse redirects in every ancestor, including junctions above the package root.
  let ancestor = absolute
  while (true) {
    const stat = lstatSync(ancestor)
    if (
      !stat.isDirectory() ||
      stat.isSymbolicLink() ||
      realpathSync(ancestor).toLowerCase() !== ancestor.toLowerCase()
    )
      throw new Error('Redirected package root')
    const parent = resolve(ancestor, '..')
    if (parent === ancestor) break
    ancestor = parent
  }
  const expected = new Map(manifest.files.map(file => [file.path, file]))
  const seen = new Set<string>()
  const walk = (directory: string): void => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name)
      const portable = relative(absolute, path).replaceAll('\\', '/')
      const stat = lstatSync(path)
      if (
        stat.isSymbolicLink() ||
        realpathSync(path).toLowerCase() !== path.toLowerCase()
      )
        throw new Error('Redirected package file')
      if (stat.isDirectory()) {
        walk(path)
        continue
      }
      const file = expected.get(portable)
      if (!stat.isFile() || !file || stat.size !== file.size)
        throw new Error('Unexpected or invalid package file')
      const bytes = readFileSync(path)
      if (createHash('sha256').update(bytes).digest('hex') !== file.sha256)
        throw new Error('Package file integrity mismatch')
      seen.add(portable)
    }
  }
  walk(absolute)
  if (seen.size !== expected.size) throw new Error('Missing package file')
  return manifest
}
