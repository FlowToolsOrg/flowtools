import { createHash } from 'node:crypto'
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  htmlPluginCatalogSchema,
  portablePluginPathSchema,
  type HtmlCatalogPlugin,
  type HtmlPluginCatalog,
} from '../packages/sdk/src/compat/catalog'
import {
  normalizeHtmlPluginManifest,
  type HtmlPluginManifest,
} from '../packages/sdk/src/compat/html-plugin'

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const fixturePath = 'scripts/fixtures/html-catalog/static-entry-v1.json'
interface CategoryMapping {
  title: string
  list: string[]
}
const digest = (contents: string | Buffer) =>
  createHash('sha256').update(contents).digest('hex')
const portable = (path: string) => path.split(sep).join('/')

function assertContained(root: string, path: string): string {
  const canonical = realpathSync(path)
  const rel = relative(realpathSync(root), canonical)
  if (isAbsolute(rel) || rel === '..' || rel.startsWith(`..${sep}`))
    throw new Error('Catalog artifact is outside package')
  return canonical
}

function inspectEntry(
  pluginPath: string,
  manifestPath: string,
  main?: string
): {
  assetDir?: string
  main?: string
  evidence: HtmlCatalogPlugin['evidence']
} {
  if (!main) return { evidence: { status: 'indexed', reason: 'absent' } }
  if (/^https?:\/\//i.test(main))
    return { evidence: { status: 'indexed', reason: 'remote' } }
  if (!portablePluginPathSchema.safeParse(main).success)
    return { evidence: { status: 'indexed', reason: 'unsafe' } }
  if (/\.[jt]sx?$/i.test(main))
    return { evidence: { status: 'indexed', reason: 'source-only' } }
  const manifestDir = portable(relative(pluginPath, dirname(manifestPath)))
  const candidates = [
    ...new Set(
      manifestDir === 'public' ? ['dist', 'public', ''] : [manifestDir, '']
    ),
  ]
  let sourceOnly = false
  for (const candidate of candidates) {
    const entry = join(pluginPath, candidate, main)
    if (!existsSync(entry)) continue
    const canonical = assertContained(pluginPath, entry)
    if (!statSync(canonical).isFile())
      throw new Error('Catalog entry is not a regular file')
    const content = readFileSync(canonical)
    const scripts = [
      ...content
        .toString()
        .matchAll(/<script\b[^>]*\bsrc\s*=\s*["']([^"']+)["']/gi),
    ]
    if (
      scripts.some(match =>
        /(?:^|\/)src\/|(?:^|\/)@vite\/client|\.(?:tsx?|jsx)(?:[?#]|$)|^\/main\.[jt]sx?(?:[?#]|$)/i.test(
          match[1]!
        )
      )
    ) {
      sourceOnly = true
      continue
    }
    const path = [candidate, main].filter(Boolean).join('/')
    return {
      main,
      assetDir: candidate || undefined,
      evidence: {
        status: 'entry-resolved',
        fixtureId: 'static-entry-v1',
        path: portablePluginPathSchema.parse(path),
        sha256: digest(content),
      },
    }
  }
  return {
    main: sourceOnly ? undefined : main,
    evidence: {
      status: 'indexed',
      reason: sourceOnly ? 'source-only' : 'missing',
    },
  }
}

/** Read-only scan. Evidence covers file resolution, never compatibility/security. */
export function inspectHtmlPluginCatalog(
  sourceRoot: string
): HtmlPluginCatalog {
  const pluginRoot = join(sourceRoot, 'plugins')
  const categoriesPath = join(sourceRoot, 'categories-mapping.json')
  const categories: CategoryMapping[] = existsSync(categoriesPath)
    ? (JSON.parse(readFileSync(categoriesPath, 'utf8')) as CategoryMapping[])
    : []
  const plugins = readdirSync(pluginRoot, { withFileTypes: true })
    .filter(entry => entry.isDirectory() || entry.isSymbolicLink())
    .flatMap(entry => {
      const pluginPath = join(pluginRoot, entry.name)
      assertContained(pluginRoot, pluginPath)
      if (!statSync(pluginPath).isDirectory()) return []
      portablePluginPathSchema.parse(entry.name)
      const manifestPath = [
        'dist/plugin.json',
        'plugin.json',
        'public/plugin.json',
      ]
        .map(path => join(pluginPath, path))
        .find(existsSync)
      if (!manifestPath) return []
      assertContained(pluginPath, manifestPath)
      const contents = readFileSync(manifestPath)
      const manifest = JSON.parse(contents.toString()) as HtmlPluginManifest
      const entryInfo = inspectEntry(pluginPath, manifestPath, manifest.main)
      const normalized = normalizeHtmlPluginManifest(manifest, {
        sourceDir: entry.name,
        category: categories.find(category =>
          category.list.includes(entry.name)
        )?.title,
      })
      const html = { ...normalized.html }
      delete html.developmentMain
      const invalidBoolean = html.features.some(
        feature =>
          (feature.mainPush !== undefined &&
            typeof feature.mainPush !== 'boolean') ||
          (feature.mainHide !== undefined &&
            typeof feature.mainHide !== 'boolean')
      )
      if (invalidBoolean)
        html.compatibility.notes.push(
          '非布尔 mainPush/mainHide 未映射；仅索引元数据，不证明旧 API 行为。'
        )
      html.features = html.features.map(feature => ({
        ...feature,
        mainPush:
          typeof feature.mainPush === 'boolean' ? feature.mainPush : undefined,
        mainHide:
          typeof feature.mainHide === 'boolean' ? feature.mainHide : undefined,
      }))
      return [
        {
          ...normalized,
          maturity: 'prototype' as const,
          package: {
            kind: 'legacy-html' as const,
            id: normalized.id,
            root: `plugins/${entry.name}`,
            manifestSha256: digest(contents),
          },
          evidence: entryInfo.evidence,
          html: {
            ...html,
            sourceDir: entry.name,
            main: entryInfo.main,
            assetDir: entryInfo.assetDir,
            mainAvailable: entryInfo.evidence.status === 'entry-resolved',
            hasDevelopmentMain: Boolean(manifest.development?.main),
            preload: manifest.preload
              ? portablePluginPathSchema.parse(manifest.preload)
              : undefined,
          },
        },
      ]
    })
    .sort((a, b) => a.id.localeCompare(b.id))
  const byCompatibility: Record<string, number> = {}
  const byCategory: Record<string, number> = {}
  for (const plugin of plugins) {
    const level = plugin.html.compatibility.level
    const category = plugin.category ?? '未分类'
    byCompatibility[level] = (byCompatibility[level] ?? 0) + 1
    byCategory[category] = (byCategory[category] ?? 0) + 1
  }
  return htmlPluginCatalogSchema.parse({
    formatVersion: 1,
    source: 'legacy-html-checkout',
    fixtures: [
      {
        id: 'static-entry-v1',
        path: fixturePath,
        sha256: digest(
          readFileSync(join(repositoryRoot, fixturePath), 'utf8').replaceAll(
            '\r\n',
            '\n'
          )
        ),
      },
    ],
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
  })
}

if (import.meta.main) {
  const sourceRoot = process.argv[2]
    ? resolve(process.argv[2])
    : join(repositoryRoot, ['Z', 'Tools-plugins'].join(''))
  const catalog = inspectHtmlPluginCatalog(sourceRoot)
  const formatted = Bun.spawnSync(
    [
      process.execPath,
      join(repositoryRoot, 'node_modules/oxfmt/bin/oxfmt'),
      '--stdin-filepath',
      join(repositoryRoot, 'docs/html-plugin-catalog.json'),
    ],
    {
      cwd: repositoryRoot,
      stdin: Buffer.from(JSON.stringify(catalog, null, 2)),
      stdout: 'pipe',
      stderr: 'pipe',
    }
  )
  if (formatted.exitCode !== 0)
    throw new Error(`Catalog formatting failed: ${formatted.stderr.toString()}`)
  const content = formatted.stdout.toString()
  for (const path of [
    'apps/desktop/src/data/html-plugin-catalog.json',
    'docs/html-plugin-catalog.json',
  ]) {
    const target = join(repositoryRoot, path)
    mkdirSync(dirname(target), { recursive: true })
    writeFileSync(target, content)
  }
  process.stdout.write(
    `Indexed ${catalog.totals.plugins} plugins / ${catalog.totals.commands} commands; file evidence only.\n`
  )
}
