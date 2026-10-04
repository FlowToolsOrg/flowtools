import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { parseManifestCatalog } from '@flowtools/sdk/manifest'

import { builtInCLIManifests } from '../cli/src/builtin-manifests'

// Build-owned complete package metadata, not a caller-supplied installation source.
const root = resolve(import.meta.dir, '../..')
const catalog = parseManifestCatalog(
  JSON.parse(
    readFileSync(
      resolve(root, 'plugins/.generated/builtin-manifests.json'),
      'utf8'
    )
  ),
  {
    hostVersion: '0.1.0',
    sdkVersion: '0.0.0',
    platform: 'windows',
    arch: 'x64',
  },
  builtInCLIManifests.map(plugin => plugin.id)
)
const directory = resolve(import.meta.dir, '.generated')
mkdirSync(directory, { recursive: true })
writeFileSync(
  resolve(directory, 'catalog.json'),
  JSON.stringify({ formatVersion: 1, plugins: catalog }) + '\n'
)
