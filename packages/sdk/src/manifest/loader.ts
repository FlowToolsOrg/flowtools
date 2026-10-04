import type { ManifestTarget, PluginManifestV1 } from './schema'

import { resolvePluginMaturity } from '../types/maturity'

import { equalJsonValues } from './json-schema'
import { parsePluginManifest } from './schema'

type BoundIdentity = Pick<
  PluginManifestV1,
  'id' | 'publisher' | 'version' | 'maturity' | 'type'
>
interface ManifestModule {
  default: {
    type: 'app' | 'tool'
    meta: { id: string; version: string; maturity?: unknown }
  }
}

/** Validate metadata before a host-bound T1 callback; never resolve a package path.
 * Browser bundlers bind artifacts. This is not signature/file verification or a grant.
 */
export async function loadManifestModule<T extends ManifestModule>(
  value: unknown,
  target: ManifestTarget,
  expected: BoundIdentity,
  load: () => Promise<T>
): Promise<T> {
  const manifest = parsePluginManifest(value, target)
  if (
    manifest.id !== expected.id ||
    manifest.publisher !== expected.publisher ||
    manifest.version !== expected.version ||
    manifest.maturity !== expected.maturity ||
    manifest.type !== expected.type ||
    !manifest.entries.ui
  )
    throw new Error('Manifest does not match the host-bound UI identity')
  const module = await load()
  const plugin = module?.default
  if (
    !plugin ||
    plugin.type !== manifest.type ||
    plugin.meta?.id !== manifest.id ||
    plugin.meta.version !== manifest.version ||
    resolvePluginMaturity(plugin.meta.maturity) !== manifest.maturity
  )
    throw new Error('UI module does not match its manifest identity')
  if (
    manifest.commands.length &&
    (!('command' in plugin) ||
      manifest.commands.length !== 1 ||
      !equalJsonValues(plugin.command, manifest.commands[0]))
  )
    throw new Error('UI module does not match its adapted command contract')
  return module
}
