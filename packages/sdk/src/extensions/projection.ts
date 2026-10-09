import type { PluginRegistry } from '../registry/plugin-registry'
import type { RegisteredPlugin } from '../registry/types'

import { ExtensionContributionRegistry } from './registry'

interface ProjectionLease {
  dispose: () => void
}

/**
 * Project Host-supplied documents for enabled fixed T1 entries only. The reader
 * retrieves declarations; this helper never invokes a loader or plugin run().
 * Invalid declarations fail activation rather than publishing partial data.
 */
export function projectPluginContributions(
  plugins: PluginRegistry,
  contributions: ExtensionContributionRegistry,
  readDocument: (entry: RegisteredPlugin) => unknown
): () => void {
  const projected = new Map<string, ProjectionLease>()
  const sync = (pluginId: string) => {
    const previous = projected.get(pluginId)
    const pending: ProjectionLease = { dispose: () => {} }
    projected.set(pluginId, pending)
    previous?.dispose()
    if (projected.get(pluginId) !== pending) return
    const entry = plugins.get(pluginId)
    if (
      entry?.state !== 'enabled' ||
      !entry.plugin ||
      entry.manifest.dependenciesSatisfied === false
    ) {
      projected.delete(pluginId)
      return
    }
    const document = readDocument(entry)
    if (
      projected.get(pluginId) !== pending ||
      plugins.get(pluginId) !== entry
    ) {
      if (projected.get(pluginId) === pending) projected.delete(pluginId)
      return
    }
    if (document === undefined) {
      projected.delete(pluginId)
      return
    }
    const owner = contributions.createOwner(entry.id, entry.manifest.version)
    // Notifications are synchronous. Reserve cleanup before replace publishes
    // so nested disable/uninstall can revoke even before replace returns.
    pending.dispose = () => {
      contributions.revoke(owner)
    }
    const dispose = contributions.replace(owner, document)
    if (projected.get(pluginId) !== pending) {
      dispose()
      return
    }
    if (plugins.get(pluginId) !== entry) {
      projected.delete(pluginId)
      dispose()
      return
    }
    pending.dispose = dispose
  }
  const unsubscribe = plugins.subscribe(event => sync(event.pluginId))
  const dispose = () => {
    unsubscribe()
    const leases = [...projected.values()]
    projected.clear()
    for (const lease of leases) lease.dispose()
  }
  try {
    for (const entry of plugins.getAll()) sync(entry.id)
  } catch (error) {
    dispose()
    throw error
  }
  return dispose
}
