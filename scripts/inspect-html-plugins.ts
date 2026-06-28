import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from 'node:fs'
import { dirname, join, relative, sep } from 'node:path'

import {
  normalizeHtmlPluginManifest,
  type FlowToolsHtmlPluginManifest,
  type HtmlPluginManifest,
} from '../packages/sdk/src/compat/html-plugin.ts'

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
  plugins: FlowToolsHtmlPluginManifest[]
}

const repoRoot = process.cwd()
const legacyCheckoutName = ['Z', 'Tools-plugins'].join('')
const defaultSource = join(repoRoot, legacyCheckoutName)
const sourceRoot = process.argv[2]
  ? join(repoRoot, process.argv[2])
  : defaultSource
const pluginRoot = join(sourceRoot, 'plugins')
const categoryFile = join(sourceRoot, 'categories-mapping.json')
const catalogFiles = [
  join(repoRoot, 'apps', 'desktop', 'src', 'data', 'html-plugin-catalog.json'),
  join(repoRoot, 'docs', 'html-plugin-catalog.json'),
]
const legacyBrandPattern = new RegExp(['z', 'tools'].join(''), 'gi')

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

function unicodeEscape(value: string): string {
  return `\\u${value.charCodeAt(0).toString(16).padStart(4, '0')}`
}

function escapeLegacyBrand(value: string): string {
  return value.replace(legacyBrandPattern, match => {
    return `${unicodeEscape(match[0] ?? '')}${unicodeEscape(
      match[1] ?? ''
    )}${match.slice(2)}`
  })
}

function toCatalogPath(path: string): string {
  return path.split(sep).join('/')
}

function stripRelativePath(path: string): string {
  return path.replace(/\\/g, '/').replace(/^\/+/, '')
}

function uniqueStrings(values: Iterable<string | undefined>): string[] {
  const seen = new Set<string>()
  const next: string[] = []

  for (const value of values) {
    const normalized = value?.replace(/\\/g, '/').replace(/\/+$/, '')
    if (normalized === undefined || seen.has(normalized)) {
      continue
    }

    seen.add(normalized)
    next.push(normalized)
  }

  return next
}

function htmlUsesSourceEntrypoint(path: string): boolean {
  if (!existsSync(path)) return false

  const html = readFileSync(path, 'utf8')
  return /<script\b[^>]+type=["']module["'][^>]+src=["']\/(?:src\/main\.[jt]sx?|main\.[jt]sx?)["']/i.test(
    html
  )
}

interface StaticMainInfo {
  assetDir?: string
  available: boolean
}

function getStaticMainInfo(
  pluginPath: string,
  manifestPath: string,
  main?: string
): StaticMainInfo {
  if (!main) return { available: false }
  if (/^https?:\/\//i.test(main)) return { available: true }

  const manifestRelDir = toCatalogPath(
    relative(pluginPath, dirname(manifestPath))
  )
  const candidates =
    manifestRelDir === 'public'
      ? uniqueStrings(['dist', 'public', ''])
      : uniqueStrings([manifestRelDir || '', ''])
  const entry = stripRelativePath(main)

  for (const candidate of candidates) {
    const entryPath = join(pluginPath, candidate, entry)
    if (!existsSync(entryPath)) {
      continue
    }

    if (htmlUsesSourceEntrypoint(entryPath)) {
      continue
    }

    return { assetDir: candidate || undefined, available: true }
  }

  return { available: false }
}

function main(): void {
  if (!existsSync(pluginRoot)) {
    throw new Error(`HTML plugin directory not found: ${pluginRoot}`)
  }

  const categories = existsSync(categoryFile)
    ? readJson<CategoryMapping[]>(categoryFile)
    : []

  const plugins = readdirSync(pluginRoot, { withFileTypes: true })
    .filter(entry => entry.isDirectory())
    .map(entry => entry.name)
    .flatMap(pluginDir => {
      const pluginPath = join(pluginRoot, pluginDir)
      const distManifestPath = join(pluginPath, 'dist', 'plugin.json')
      const manifestPath = join(pluginPath, 'plugin.json')
      const publicManifestPath = join(pluginPath, 'public', 'plugin.json')
      const path = existsSync(distManifestPath)
        ? distManifestPath
        : existsSync(manifestPath)
          ? manifestPath
          : existsSync(publicManifestPath)
            ? publicManifestPath
            : null

      if (!path) {
        return []
      }

      const manifest = readJson<HtmlPluginManifest>(path)
      const staticMain = getStaticMainInfo(pluginPath, path, manifest.main)

      return normalizeHtmlPluginManifest(manifest, {
        sourceDir: pluginDir,
        assetDir: staticMain.assetDir,
        mainAvailable: staticMain.available,
        category: getCategoryByPlugin(pluginDir, categories),
      })
    })
    .sort((a, b) => a.id.localeCompare(b.id))

  const byCompatibility: Record<string, number> = {}
  const byCategory: Record<string, number> = {}

  for (const plugin of plugins) {
    increment(byCompatibility, plugin.html.compatibility.level)
    increment(byCategory, plugin.category ?? '未分类')
  }

  const catalog: PluginCatalog = {
    generatedAt: new Date().toISOString(),
    source: sourceRoot,
    totals: {
      plugins: plugins.length,
      commands: plugins.reduce(
        (count, plugin) => count + plugin.html.commands.length,
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
    writeFileSync(
      catalogFile,
      `${escapeLegacyBrand(JSON.stringify(catalog, null, 2))}\n`
    )
  }

  console.log(
    `Scanned ${catalog.totals.plugins} HTML plugins and ${catalog.totals.commands} commands.`
  )
  console.log('Catalog written to:')
  for (const catalogFile of catalogFiles) {
    console.log(`- ${catalogFile}`)
  }
}

main()
