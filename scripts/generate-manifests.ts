#!/usr/bin/env bun

/**
 * 自动生成 manifests.ts 脚本
 * 扫描 plugins 目录，提取插件 meta 信息，生成 manifests.ts
 */

import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs'
import { join, dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

interface PluginMeta {
  id: string
  name: string
  version: string
  description?: string
  type: 'app' | 'tool'
  permissions?: string[]
  tags?: string[]
  category?: string
  cliAvailable: boolean
}

const PLUGINS_DIR = resolve(__dirname, '..', 'plugins')
const OUTPUT_FILE = resolve(
  __dirname,
  '..',
  'apps/web-vite/src/plugin/manifests.ts'
)

function extractMeta(content: string, filePath: string): PluginMeta | null {
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

  if (!id || !name || !version) {
    console.warn(`⚠️  ${filePath}: 缺少必要的 meta 字段 (id, name, version)`)
    return null
  }

  // 提取 permissions 数组
  const permissions = extractArrayField(metaStr, 'permissions')

  // 提取 tags 数组
  const tags = extractArrayField(metaStr, 'tags')

  // 提取 category
  const category = extractStringField(metaStr, 'category')

  // 检测是否有 run 函数（CLI 可用）
  const cliAvailable = /run\s*[<(]/.test(content) || /run\s*:/.test(content)

  // 检测是否有 Zod inputSchema
  const hasSchema = /inputSchema:\s*z\.object/.test(content)

  return {
    id,
    name,
    version,
    description,
    type,
    permissions,
    tags,
    category,
    cliAvailable,
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

function scanPlugins(): PluginMeta[] {
  const plugins: PluginMeta[] = []

  try {
    const entries = readdirSync(PLUGINS_DIR)

    for (const entry of entries) {
      const pluginDir = join(PLUGINS_DIR, entry)
      const stat = statSync(pluginDir)

      if (!stat.isDirectory() || !entry.startsWith('plugin-')) {
        continue
      }

      // 查找 index.ts 或 index.tsx
      let filePath = join(pluginDir, 'index.tsx')
      try {
        statSync(filePath)
      } catch {
        filePath = join(pluginDir, 'index.ts')
        try {
          statSync(filePath)
        } catch {
          console.warn(`⚠️  ${entry}: 未找到 index.ts 或 index.tsx`)
          continue
        }
      }

      const content = readFileSync(filePath, 'utf-8')
      const meta = extractMeta(content, filePath)

      if (meta) {
        plugins.push(meta)
        console.log(`✅ ${meta.id}`)
      }
    }
  } catch (err) {
    console.error('❌ 扫描 plugins 目录失败:', err)
    process.exit(1)
  }

  return plugins
}

function generateManifests(plugins: PluginMeta[]): string {
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
    version: '${p.version}',${p.description ? `\n    description: '${p.description}',` : ''}
    type: '${p.type}',${p.permissions && p.permissions.length > 0 ? `\n    permissions: [${p.permissions.map(p => `'${p}'`).join(', ')}],` : ''}${p.tags && p.tags.length > 0 ? `\n    tags: [${p.tags.map(t => `'${t}'`).join(', ')}],` : ''}${p.category ? `\n    category: '${p.category}',` : ''}
    cliAvailable: ${p.cliAvailable},
    loader: () => import('@plugins/${p.id}'),
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

  return `import type { PluginManifestEntry } from '@flow-tool/sdk'

export const builtInManifests: PluginManifestEntry[] = [
${manifestsEntries},
]

export const pluginCategories = [
${categoryEntries},
] as const
`
}

// 主流程
console.log('🔍 扫描 plugins 目录...\n')

const plugins = scanPlugins()

console.log(`\n📦 共发现 ${plugins.length} 个插件\n`)
console.log('📝 生成 manifests.ts...\n')

const content = generateManifests(plugins)

writeFileSync(OUTPUT_FILE, content, 'utf-8')

console.log(`✅ 已生成: ${OUTPUT_FILE}`)
console.log('\n🎉 完成!')
