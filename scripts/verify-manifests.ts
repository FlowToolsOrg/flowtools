import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { builtInCLIManifests } from '../packages/cli/src/builtin-manifests'
import {
  parsePluginManifest,
  commandIdentity,
} from '../packages/sdk/src/manifest'
import { verifyManifestPackage } from '../packages/sdk/src/manifest/package'
import {
  manifestFixture,
  manifestTarget,
} from '../packages/sdk/test/fixtures/manifest-v1'

/** Read-only protocol gate; it does not import or execute package entries. */
export function verifyManifestContracts() {
  const manifest = parsePluginManifest(
    JSON.parse(JSON.stringify(manifestFixture())),
    manifestTarget
  )
  const identities = manifest.commands.map(command =>
    commandIdentity(manifest, command)
  )
  if (new Set(identities).size !== identities.length)
    throw new Error('Duplicate command identity')
  return identities
}

export function verifyBuiltinManifests() {
  const root = resolve(import.meta.dir, '../plugins')
  const catalog: unknown = JSON.parse(
    readFileSync(resolve(root, '.generated/builtin-manifests.json'), 'utf8')
  )
  if (
    !catalog ||
    typeof catalog !== 'object' ||
    Object.keys(catalog).sort().join(',') !== 'formatVersion,plugins' ||
    !('formatVersion' in catalog) ||
    catalog.formatVersion !== 1 ||
    !('plugins' in catalog) ||
    !Array.isArray(catalog.plugins) ||
    catalog.plugins.length !== builtInCLIManifests.length
  )
    throw new Error('Invalid built-in manifest catalog')
  const manifests = catalog.plugins.map(value =>
    parsePluginManifest(value, manifestTarget)
  )
  if (
    JSON.stringify(manifests.map(item => item.id).sort()) !==
    JSON.stringify(builtInCLIManifests.map(item => item.id).sort())
  )
    throw new Error('Built-in command inventory mismatch')
  for (const manifest of manifests)
    verifyManifestPackage(resolve(root, 'dist'), manifest, manifestTarget)
  return manifests
}

if (import.meta.main) {
  verifyManifestContracts()
  const manifests = verifyBuiltinManifests()
  process.stdout.write(
    `Manifest v1 protocol and ${manifests.length} built-in packages passed; not signing or an execution grant.\n`
  )
}
