import type { CLIPluginInfo } from './types'
/** Fixed, host-built inventory. Never discover or execute workspace source. */
import type { FlowToolPlugin } from '@flowtools/sdk/types'

import { lstatSync, realpathSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

import { resolvePluginMaturity } from '@flowtools/sdk/types'

import { builtInCLIManifests } from './builtin-manifests'

const PLUGINS_DIR = resolve(import.meta.dir, '..', '..', '..', 'plugins')
const DIST_DIR = join(PLUGINS_DIR, 'dist')
const inventory = new Map<string, CLIPluginInfo>(
  builtInCLIManifests.map(info => [info.id, { ...info }])
)

/** Pure metadata lookup: unknown IDs are rejected before filesystem access. */
export function getBuiltinPluginInfo(id: string): CLIPluginInfo | null {
  const info = inventory.get(id)
  return info ? { ...info } : null
}

/** Inventory discovery checks artifact presence, not compatibility or safety. */
export function scanPlugins(): CLIPluginInfo[] {
  return [...inventory.values()].map(info => {
    if (!compiledArtifact(info.id))
      throw new Error(
        'Built-in compiled artifacts are unavailable; build packages'
      )
    return { ...info }
  })
}

function compiledArtifact(id: string): string | null {
  try {
    // A caller-provided path is never joined until host inventory lookup succeeds.
    if (!inventory.has(id)) return null
    for (const dir of [PLUGINS_DIR, DIST_DIR]) {
      const stat = lstatSync(dir)
      if (!stat.isDirectory() || stat.isSymbolicLink()) return null
    }
    const root = realpathSync(PLUGINS_DIR)
    const dist = realpathSync(DIST_DIR)
    if (relative(root, dist) !== 'dist') return null
    const entry = join(dist, `${id}.js`)
    const stat = lstatSync(entry)
    if (!stat.isFile() || stat.isSymbolicLink()) return null
    const canonical = realpathSync(entry)
    return relative(dist, canonical) === `${id}.js` ? canonical : null
  } catch {
    return null
  }
}

/**
 * Only load the compiled T1 module declared by this host build. This is not a
 * package signature, an OS sandbox, or protection against replacing T1 files.
 */
export async function loadPlugin(id: string): Promise<FlowToolPlugin | null> {
  const expected = getBuiltinPluginInfo(id)
  if (!expected?.hasRun) return null
  const artifact = compiledArtifact(id)
  if (!artifact) return null
  try {
    const imported: unknown = await import(pathToFileURL(artifact).href)
    const module = imported as { default?: FlowToolPlugin }
    const plugin = module.default
    if (
      !plugin ||
      plugin.type !== expected.type ||
      plugin.meta?.id !== expected.id ||
      plugin.meta.version !== expected.version ||
      resolvePluginMaturity(plugin.meta.maturity) !== expected.maturity ||
      typeof plugin.run !== 'function' ||
      Boolean(plugin.inputSchema) !== expected.hasSchema ||
      (expected.hasSchema &&
        typeof plugin.inputSchema?.safeParse !== 'function')
    )
      return null
    return plugin
  } catch {
    // Broken/missing compiled artifacts never enable source or rewrite fallbacks.
    return null
  }
}
