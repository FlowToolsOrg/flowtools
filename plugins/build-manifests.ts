import type {
  CommandManifestV1,
  PluginManifestV1,
} from '@flowtools/sdk/manifest'
import type { PluginMeta } from '@flowtools/sdk/types'

import { createHash } from 'node:crypto'
import {
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from 'node:fs'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

import { parsePluginManifest } from '@flowtools/sdk/manifest'

import { getPluginEntries } from './plugin-entries'

export async function buildBuiltinManifests(root = import.meta.dir) {
  const dist = resolve(root, 'dist')
  const files: PluginManifestV1['files'] = []
  const walk = (directory: string, prefix = '') => {
    for (const entry of readdirSync(directory, { withFileTypes: true }).sort(
      (a, b) => a.name.localeCompare(b.name)
    )) {
      const path = join(directory, entry.name),
        name = `${prefix}${entry.name}`
      if (lstatSync(path).isSymbolicLink())
        throw new Error('Redirected build artifact')
      if (entry.isDirectory()) {
        walk(path, `${name}/`)
        continue
      }
      if (!entry.isFile()) throw new Error('Special build artifact')
      const bytes = readFileSync(path)
      files.push({
        path: name,
        size: bytes.length,
        sha256: createHash('sha256').update(bytes).digest('hex'),
      })
    }
  }
  walk(dist)
  const manifests: PluginManifestV1[] = []
  for (const id of Object.keys(getPluginEntries(root))) {
    // Build-time import of actual pure command output, never CLI source execution.
    const module = (await import(
      pathToFileURL(join(dist, `${id}.commands.js`)).href
    )) as {
      default: { meta: PluginMeta; type: 'app'; command: CommandManifestV1 }
    }
    const plugin = module.default
    if (plugin.meta.id !== id)
      throw new Error('Built-in artifact identity mismatch')
    manifests.push(
      parsePluginManifest({
        formatVersion: 1,
        publisher: 'flowtools',
        id,
        name: plugin.meta.name,
        description: plugin.meta.description ?? `执行${plugin.meta.name}`,
        version: plugin.meta.version,
        type: plugin.type,
        maturity: plugin.meta.maturity,
        engines: { host: '^0.1.0', sdk: '>=0.0.0 <1.0.0' },
        targets: [
          { platform: 'windows', arch: 'x64' },
          { platform: 'web', arch: 'wasm32' },
        ],
        entries: { ui: `${id}.js`, executor: `${id}.commands.js` },
        files,
        commands: [plugin.command],
        dependencies: { services: [], tools: [] },
        signature: { status: 'unsigned' },
      })
    )
  }
  const output = resolve(root, '.generated')
  mkdirSync(output, { recursive: true })
  writeFileSync(
    join(output, 'builtin-manifests.json'),
    JSON.stringify({ formatVersion: 1, plugins: manifests }, null, 2) + '\n'
  )
  return manifests
}

if (import.meta.main) {
  const manifests = await buildBuiltinManifests()
  process.stdout.write(
    `Built ${manifests.length} serialized T1 command manifests; no package signature or grant.\n`
  )
}
