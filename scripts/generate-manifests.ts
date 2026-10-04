#!/usr/bin/env bun

/**
 * 自动生成 manifests.ts 脚本
 * 扫描 plugins 目录，提取插件 meta 信息，生成 manifests.ts
 */

import type { Permission, PluginMeta } from '../packages/sdk'
import type { FormatConfig } from 'oxfmt'

import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs'
import { join, dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { format } from 'oxfmt'

import { resolvePluginMaturity } from '../packages/sdk/src/types/maturity'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

const PLUGINS_DIR = resolve(__dirname, '..', 'plugins')
const OUTPUT_FILES = ['web-vite', 'desktop'].map(host =>
  resolve(__dirname, '..', 'apps', host, 'src/plugin/manifests.ts')
)
const CLI_OUTPUT = resolve(
  __dirname,
  '..',
  'packages/cli/src/builtin-manifests.ts'
)

type ComputedPluginMeta = PluginMeta & {
  type: 'app' | 'tool'
  cliAvailable: boolean
  hasSchema: boolean
}

function extractMeta(content: string): ComputedPluginMeta | null {
  // 提取 type
  const typeMatch = content.match(/type:\s*['"](\w+)['"]/)
  if (!typeMatch) return null
  const type = typeMatch[1] as 'app' | 'tool'

  // 提取 meta 对象内容
  const metaMatch = content.match(/meta:\s*\{([^}]+)\}/s)
  if (!metaMatch) return null

  const metaStr = metaMatch[1]

  // 提取各个字段
  const id = extractStringField(metaStr, 'id')
  const name = extractStringField(metaStr, 'name')
  const version = extractStringField(metaStr, 'version')
  const description = extractStringField(metaStr, 'description')
  const maturity = resolvePluginMaturity(
    extractStringField(metaStr, 'maturity')
  )

  if (!id || !name || !version) {
    return null
  }

  // 提取 permissions 数组
  const permissions = (extractArrayField(metaStr, 'permissions') ||
    []) as Permission[]

  // 提取 tags 数组
  const tags = extractArrayField(metaStr, 'tags')

  // 提取 category
  const category = extractStringField(metaStr, 'category')

  // 检测是否有 run 函数（CLI 可用）
  const cliAvailable = /run\s*[<(]/.test(content) || /run\s*:/.test(content)
  const hasSchema =
    /inputSchema\s*=\s*z\.object/.test(content) ||
    /inputSchema:\s*z\.object/.test(content)

  return {
    id,
    name,
    version,
    maturity,
    description,
    type,
    permissions,
    tags,
    category,
    cliAvailable,
    hasSchema,
  }
}

function extractStringField(str: string, field: string): string | undefined {
  const match = str.match(new RegExp(`${field}:\\s*['"]([^'"]+)['"]`))
  return match ? match[1] : undefined
}

function extractArrayField(str: string, field: string): string[] | undefined {
  const match = str.match(new RegExp(`${field}:\\s*\\[([^\\]]*)\\]`))
  if (!match) return undefined

  const items = match[1]
    .split(',')
    .map(s => s.trim().replace(/['"]/g, ''))
    .filter(Boolean)

  return items.length > 0 ? items : undefined
}

function scanPlugins(): ComputedPluginMeta[] {
  const plugins: ComputedPluginMeta[] = []

  try {
    const entries = readdirSync(PLUGINS_DIR).sort()

    for (const entry of entries) {
      const pluginDir = join(PLUGINS_DIR, entry)
      const stat = statSync(pluginDir)

      if (!stat.isDirectory() || !entry.startsWith('plugin-')) {
        continue
      }

      // 查找 index.ts 或 index.tsx
      let filePath = join(pluginDir, 'commands.ts')
      try {
        statSync(filePath)
      } catch {
        filePath = join(pluginDir, 'index.ts')
        try {
          statSync(filePath)
        } catch {
          continue
        }
      }

      const content = readFileSync(filePath, 'utf-8')
      const meta = extractMeta(content)

      if (meta) {
        plugins.push(meta)
      }
    }
  } catch {
    process.exit(1)
  }

  return plugins
}

function generateManifests(
  plugins: ComputedPluginMeta[],
  host: string
): string {
  // 按 category 分组生成 categories
  const categoryMap = new Map<string, string[]>()

  for (const plugin of plugins) {
    const cat = plugin.category || '其他'
    if (!categoryMap.has(cat)) {
      categoryMap.set(cat, [])
    }
    categoryMap.get(cat)!.push(plugin.id)
  }

  // 生成 manifests 数组
  const manifestsEntries = plugins
    .map(
      p => `  {
    id: '${p.id}',
    name: '${p.name}',
    version: '${p.version}',
    maturity: '${p.maturity}',${p.description ? `\n    description: '${p.description}',` : ''}
    type: '${p.type}',${p.permissions && p.permissions.length > 0 ? `\n    permissions: [${p.permissions.map(p => `'${p}'`).join(', ')}],` : ''}${p.tags && p.tags.length > 0 ? `\n    tags: [${p.tags.map(t => `'${t}'`).join(', ')}],` : ''}${p.category ? `\n    category: '${p.category}',` : ''}
    cliAvailable: ${p.cliAvailable},
    loader: () => loadManifestModule(
      builtInManifestData.find(value => value.id === '${p.id}'),
      { hostVersion: '0.1.0', sdkVersion: '0.0.0', platform: '${host === 'desktop' ? 'windows' : 'web'}', arch: '${host === 'desktop' ? 'x64' : 'wasm32'}' },
      { publisher: 'flowtools', id: '${p.id}', version: '${p.version}', type: '${p.type}', maturity: '${p.maturity}' },
      () => import('@flowtools/plugins/${p.id}')
    ),
  }`
    )
    .join(',\n')

  // 生成 categories 数组
  const categoryEntries = Array.from(categoryMap.entries())
    .map(
      ([category, ids]) => `  {
    id: '${category}',
    label: '${category}',
    pluginIds: [${ids.map(id => `'${id}'`).join(', ')}],
  }`
    )
    .join(',\n')

  return `import type { PluginManifestEntry } from '@flowtools/sdk'
import { loadManifestModule, parseManifestCatalog } from '@flowtools/sdk/manifest'
import serializedData from '../../../../plugins/.generated/builtin-manifests.json'

export const builtInManifestData = parseManifestCatalog(
  serializedData as unknown,
  { hostVersion: '0.1.0', sdkVersion: '0.0.0', platform: '${host === 'desktop' ? 'windows' : 'web'}', arch: '${host === 'desktop' ? 'x64' : 'wasm32'}' },
  ${JSON.stringify(plugins.map(plugin => plugin.id))}
)

export const builtInManifests: PluginManifestEntry[] = [
${manifestsEntries},
]

export const pluginCategories = [
${categoryEntries},
] as const
`
}

// 主流程

const plugins = scanPlugins()

const cliInventory = plugins.map(plugin => ({
  id: plugin.id,
  name: plugin.name,
  version: plugin.version,
  maturity: plugin.maturity,
  description: plugin.description,
  type: plugin.type,
  hasRun: plugin.cliAvailable,
  hasSchema: plugin.hasSchema,
}))
const cliSource = `// Generated by scripts/generate-manifests.ts; do not edit.
// This tracked host inventory is not a signature or third-party certification.
import type { CLIPluginInfo } from './types'

export const builtInCLIManifests = ${JSON.stringify(cliInventory, null, 2)} as const satisfies readonly CLIPluginInfo[]
`
const outputs = [
  ...OUTPUT_FILES.map(path => ({
    path,
    source: generateManifests(
      plugins,
      path.includes('desktop') ? 'desktop' : 'web'
    ),
  })),
  { path: CLI_OUTPUT, source: cliSource },
]
const formatOptions = JSON.parse(
  readFileSync(resolve(__dirname, '..', '.oxfmtrc.json'), 'utf-8')
) as FormatConfig

for (const { path: outputFile, source } of outputs) {
  // One process uses the installed formatter API/config for all outputs. Do not
  // pay three cold CLI startups (or increase the bounded contract-test timeout).
  const formatted = await format(outputFile, source, formatOptions)
  if (formatted.errors.length > 0)
    throw new Error(`Manifest formatting failed: ${outputFile}`)
  const content = formatted.code
  if (process.argv.includes('--check')) {
    if (readFileSync(outputFile, 'utf-8').replaceAll('\r\n', '\n') !== content)
      throw new Error(`Generated manifest drift: ${outputFile}`)
  } else writeFileSync(outputFile, content, 'utf-8')
}
