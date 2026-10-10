import type { CLIPluginInfo } from './types'
/** Fixed, host-built inventory. Never discover or execute workspace source. */
import type { ExecutablePlugin } from '@flowtools/sdk/execution'
import type {
  ManifestTarget,
  PluginManifestV1,
  CommandManifestV1,
} from '@flowtools/sdk/manifest'
import type { PluginMeta } from '@flowtools/sdk/types'
import type { z as Zod } from 'zod'

import { lstatSync, realpathSync, readFileSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { parsePluginManifest, equalJsonValues } from '@flowtools/sdk/manifest'
import {
  manifestPackageDigest,
  verifyManifestPackage,
} from '@flowtools/sdk/manifest/package'
import { resolvePluginMaturity } from '@flowtools/sdk/types'
import { z } from 'zod'

import { builtInCLIManifests } from './builtin-manifests'

const PLUGINS_DIR = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  '..',
  'plugins'
)
const DIST_DIR = join(PLUGINS_DIR, 'dist')
const CATALOG_DIR = join(PLUGINS_DIR, '.generated')
const inventory = new Map<string, CLIPluginInfo>(
  builtInCLIManifests.map(info => [info.id, { ...info }])
)

export interface CLICommandPlugin extends ExecutablePlugin {
  type: 'app' | 'tool'
  meta: PluginMeta
  inputSchema: Zod.ZodObject<Zod.ZodRawShape>
  outputSchema: Zod.ZodType
  command: CommandManifestV1
  manifest: PluginManifestV1
}

export const cliManifestTarget: ManifestTarget = {
  hostVersion: '0.1.0',
  sdkVersion: '0.0.0',
  platform:
    process.platform === 'win32'
      ? 'windows'
      : process.platform === 'darwin'
        ? 'macos'
        : 'linux',
  arch: process.arch === 'arm64' ? 'arm64' : 'x64',
}

/** Fixed build data only. It cannot add inventory IDs, paths, or an execution grant. */
function readBuiltinManifests(): Map<string, PluginManifestV1> {
  for (const directory of [PLUGINS_DIR, CATALOG_DIR]) {
    const stat = lstatSync(directory)
    if (
      !stat.isDirectory() ||
      stat.isSymbolicLink() ||
      realpathSync(directory).toLowerCase() !== directory.toLowerCase()
    )
      throw new Error('Redirected built-in catalog')
  }
  const path = join(CATALOG_DIR, 'builtin-manifests.json')
  const stat = lstatSync(path)
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 4_194_304)
    throw new Error('Invalid built-in catalog file')
  const catalog = z
    .strictObject({
      formatVersion: z.literal(1),
      plugins: z.array(z.unknown()).length(inventory.size),
    })
    .parse(JSON.parse(readFileSync(path, 'utf8')))
  const manifests = new Map<string, PluginManifestV1>()
  for (const value of catalog.plugins) {
    const manifest = parsePluginManifest(value, cliManifestTarget)
    const expected = inventory.get(manifest.id)
    if (
      !expected ||
      manifests.has(manifest.id) ||
      manifest.publisher !== 'flowtools' ||
      manifest.version !== expected.version ||
      manifest.type !== expected.type ||
      manifest.maturity !== expected.maturity ||
      manifest.entries.executor !== `${expected.id}.commands.js` ||
      manifest.commands.length !== 1 ||
      manifest.commands[0]?.id !== 'run'
    )
      throw new Error('Built-in manifest inventory mismatch')
    manifests.set(manifest.id, manifest)
  }
  return manifests
}

export function getBuiltinCommandManifest(id: string): PluginManifestV1 | null {
  if (!inventory.has(id)) return null
  if (!compiledArtifact(id))
    throw new Error('Built-in command artifact unavailable')
  return readBuiltinManifests().get(id) ?? null
}

/** Pure metadata lookup: unknown IDs are rejected before filesystem access. */
export function getBuiltinPluginInfo(id: string): CLIPluginInfo | null {
  const info = inventory.get(id)
  return info ? { ...info } : null
}

/** Inventory discovery checks artifact presence, not compatibility or safety. */
export function scanPlugins(): CLIPluginInfo[] {
  readBuiltinManifests()
  return [...inventory.values()].map(info => {
    if (!compiledArtifact(info.id))
      throw new Error(
        'Built-in compiled artifacts are unavailable; build packages'
      )
    return { ...info }
  })
}

function compiledArtifact(
  id: string,
  kind: 'commands' | 'services' = 'commands'
): string | null {
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
    const entry = join(dist, `${id}.${kind}.js`)
    const stat = lstatSync(entry)
    if (!stat.isFile() || stat.isSymbolicLink()) return null
    const canonical = realpathSync(entry)
    return relative(dist, canonical) === `${id}.${kind}.js` ? canonical : null
  } catch {
    return null
  }
}

/** The same fixed T1 inventory and package verification, with a separate service artifact. */
export async function loadBuiltinServices(
  id: string,
  expectedPackageDigest: string
) {
  if (!inventory.has(id)) return null
  if (!/^[a-f0-9]{64}$/.test(expectedPackageDigest)) return null
  const manifest = getBuiltinCommandManifest(id)
  if (
    !manifest?.services?.length ||
    manifest.entries.services !== `${id}.services.js` ||
    manifestPackageDigest(manifest) !== expectedPackageDigest
  )
    return null
  const artifact = compiledArtifact(id, 'services')
  if (!artifact) return null
  verifyManifestPackage(DIST_DIR, manifest, cliManifestTarget)
  const imported = (await import(pathToFileURL(artifact).href)) as {
    default?: unknown
  }
  if (!imported.default || typeof imported.default !== 'object') return null
  const services = imported.default as Record<string, unknown>
  if (
    Object.keys(services).sort().join(',') !==
    manifest.services
      .map(service => service.id)
      .sort()
      .join(',')
  )
    return null
  for (const service of manifest.services) {
    const handlers = services[service.id]
    if (!handlers || typeof handlers !== 'object') return null
    const operations = handlers as Record<string, unknown>
    if (
      Object.keys(operations).sort().join(',') !==
      service.operations
        .map(operation => operation.id)
        .sort()
        .join(',')
    )
      return null
    for (const operation of service.operations) {
      const implementation = operations[operation.id]
      if (!implementation || typeof implementation !== 'object') return null
      const handler = implementation as {
        meta?: { id?: unknown; version?: unknown }
        run?: unknown
      }
      if (
        handler.meta?.id !== manifest.id ||
        handler.meta?.version !== manifest.version ||
        typeof handler.run !== 'function'
      )
        return null
    }
  }
  return {
    manifest,
    implementations:
      imported.default as import('@flowtools/sdk/services').ServiceImplementations,
  }
}

/**
 * Only load the compiled T1 module declared by this host build. This is not a
 * package signature, an OS sandbox, or protection against replacing T1 files.
 */
export async function loadPlugin(id: string): Promise<CLICommandPlugin | null> {
  const expected = getBuiltinPluginInfo(id)
  if (!expected?.hasRun) return null
  const artifact = compiledArtifact(id)
  if (!artifact) return null
  try {
    const manifest = getBuiltinCommandManifest(id)
    if (!manifest) return null
    verifyManifestPackage(DIST_DIR, manifest, cliManifestTarget)
    const imported: unknown = await import(pathToFileURL(artifact).href)
    const module = imported as { default?: CLICommandPlugin }
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
        typeof plugin.inputSchema?.safeParse !== 'function') ||
      typeof plugin.outputSchema?.safeParse !== 'function' ||
      !equalJsonValues(plugin.command, manifest.commands[0])
    )
      return null
    return { ...plugin, manifest }
  } catch {
    // Broken/missing compiled artifacts never enable source or rewrite fallbacks.
    return null
  }
}
