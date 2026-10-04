import {
  parsePluginManifest,
  commandIdentity,
} from '../packages/sdk/src/manifest'
import {
  manifestFixture,
  manifestTarget,
} from '../packages/sdk/test/fixtures/manifest-v1'

/** Read-only protocol gate; fixed built-in package verification is added in P1.1b. */
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

if (import.meta.main) {
  verifyManifestContracts()
  process.stdout.write(
    'Manifest v1 protocol fixture passed; not package signing or an execution grant.\n'
  )
}
