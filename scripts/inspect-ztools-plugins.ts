import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from 'node:fs'
import { dirname, join } from 'node:path'

import {
  normalizeZToolsManifest,
  type FlowToolsZToolsManifest,
  type ZToolsPluginManifest,
} from '../packages/sdk/src/compat/ztools.ts'

interface CategoryMapping {
  key: string
  title: string
  list: string[]
}

interface PluginCatalog {
  generatedAt: string
  source: string
  totals: {
    plugins: number
    commands: number
    appPlugins: number
    toolPlugins: number
  }
  byCompatibility: Record<string, number>
  byCategory: Record<string, number>
  plugins: FlowToolsZToolsManifest[]
}

const repoRoot = process.cwd()
const defaultSource = join(repoRoot, 'ZTools-plugins')
const sourceRoot = process.argv[2]
  ? join(repoRoot, process.argv[2])
  : defaultSource
const pluginRoot = join(sourceRoot, 'plugins')
const categoryFile = join(sourceRoot, 'categories-mapping.json')
const catalogFiles = [
  join(
    repoRoot,
    'apps',
    'desktop',
    'src',
    'data',
    'plugin-catalog.ztools.json'
  ),
  join(repoRoot, 'docs', 'plugin-catalog.ztools.json'),
]

function readJson<T>(path: string): T {
  return JSON.parse(readFileSync(path, 'utf8')) as T
}

function getCategoryByPlugin(
  pluginId: string,
  categories: CategoryMapping[]
): string | undefined {
  return categories.find(category => category.list.includes(pluginId))?.title
}

function increment(bucket: Record<string, number>, key: string): void {
  bucket[key] = (bucket[key] ?? 0) + 1
}

function main(): void {
  if (!existsSync(pluginRoot)) {
    throw new Error(`ZTools plugin directory not found: ${pluginRoot}`)
  }

  const categories = existsSync(categoryFile)
    ? readJson<CategoryMapping[]>(categoryFile)
    : []

  const plugins = readdirSync(pluginRoot, { withFileTypes: true })
    .filter(entry => entry.isDirectory())
    .map(entry => entry.name)
    .flatMap(pluginDir => {
      const manifestPath = join(pluginRoot, pluginDir, 'plugin.json')
      const publicManifestPath = join(
        pluginRoot,
        pluginDir,
        'public',
        'plugin.json'
      )
      const path = existsSync(manifestPath)
        ? manifestPath
        : existsSync(publicManifestPath)
          ? publicManifestPath
          : null

      if (!path) {
        return []
      }

      const manifest = readJson<ZToolsPluginManifest>(path)
      return normalizeZToolsManifest(manifest, {
        sourceDir: pluginDir,
        category: getCategoryByPlugin(pluginDir, categories),
      })
    })
    .sort((a, b) => a.id.localeCompare(b.id))

  const byCompatibility: Record<string, number> = {}
  const byCategory: Record<string, number> = {}

  for (const plugin of plugins) {
    increment(byCompatibility, plugin.ztools.compatibility.level)
    increment(byCategory, plugin.category ?? '未分类')
  }

  const catalog: PluginCatalog = {
    generatedAt: new Date().toISOString(),
    source: sourceRoot,
    totals: {
      plugins: plugins.length,
      commands: plugins.reduce(
        (count, plugin) => count + plugin.ztools.commands.length,
        0
      ),
      appPlugins: plugins.filter(plugin => plugin.type === 'app').length,
      toolPlugins: plugins.filter(plugin => plugin.type === 'tool').length,
    },
    byCompatibility,
    byCategory,
    plugins,
  }

  for (const catalogFile of catalogFiles) {
    mkdirSync(dirname(catalogFile), { recursive: true })
    writeFileSync(catalogFile, `${JSON.stringify(catalog, null, 2)}\n`)
  }

  console.log(
    `Scanned ${catalog.totals.plugins} ZTools plugins and ${catalog.totals.commands} commands.`
  )
  console.log('Catalog written to:')
  for (const catalogFile of catalogFiles) {
    console.log(`- ${catalogFile}`)
  }
}

main()
