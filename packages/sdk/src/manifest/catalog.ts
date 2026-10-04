import type { ManifestTarget } from './schema'

import { z } from 'zod'

import { isJsonValue } from './json-schema'
import { parsePluginManifest } from './schema'

/** Bounded pure data for a host-compiled inventory; it never adds loader paths. */
export function parseManifestCatalog(
  value: unknown,
  target: ManifestTarget,
  expectedIds: readonly string[]
) {
  if (!isJsonValue(value) || JSON.stringify(value).length > 4_194_304)
    throw new Error('Invalid manifest catalog JSON budget')
  const catalog = z
    .strictObject({
      formatVersion: z.literal(1),
      plugins: z.array(z.unknown()).min(1).max(1024),
    })
    .parse(value)
  const manifests = catalog.plugins.map(item =>
    parsePluginManifest(item, target)
  )
  const ids = manifests.map(item => item.id)
  if (
    new Set(ids).size !== ids.length ||
    JSON.stringify([...ids].sort()) !== JSON.stringify([...expectedIds].sort())
  )
    throw new Error('Manifest catalog does not match the host inventory')
  return manifests
}
