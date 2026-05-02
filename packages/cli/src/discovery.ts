/**
 * Plugin discovery — scan plugins/ directory and load CLI-compatible plugins.
 * Loads index.tsx / index.ts directly. Only plugins with a `run()` function
 * are CLI-available.
 */

import type { CLIPluginInfo } from './types'

import { readdirSync, statSync, readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

interface PluginModule {
  default: {
    type: 'app' | 'tool'
    meta: {
      id: string
      name: string
      version: string
      description?: string
      permissions?: string[]
      tags?: string[]
      category?: string
    }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    inputSchema?: { shape: unknown; safeParse?: (data: unknown) => unknown }
    run?: (...args: unknown[]) => unknown
    setup?: () => unknown
  }
}

const PLUGINS_DIR = resolve(import.meta.dir, '..', '..', '..', 'plugins')

/**
 * Scan plugins directory and return info about CLI-available plugins.
 */
export function scanPlugins(): CLIPluginInfo[] {
  const plugins: CLIPluginInfo[] = []

  let entries: string[]
  try {
    entries = readdirSync(PLUGINS_DIR)
  } catch {
    console.error('Error: plugins/ directory not found at', PLUGINS_DIR)
    return []
  }

  for (const entry of entries) {
    const pluginDir = join(PLUGINS_DIR, entry)
    try {
      const stat = statSync(pluginDir)
      if (!stat.isDirectory() || !entry.startsWith('plugin-')) continue
    } catch {
      continue
    }

    const meta = extractMetaFromSource(pluginDir)
    if (!meta) continue

    plugins.push(meta)
  }

  return plugins
}

function extractMetaFromSource(pluginDir: string): CLIPluginInfo | null {
  for (const filename of ['index.tsx', 'index.ts']) {
    const filePath = join(pluginDir, filename)
    if (!fileExists(filePath)) continue

    const content = readFileSync(filePath, 'utf-8')

    const typeMatch = content.match(/type:\s*['"](\w+)['"]/)
    if (!typeMatch) continue
    const type = typeMatch[1] as 'app' | 'tool'

    const id = extractStringField(content, 'id')
    const name = extractStringField(content, 'name')
    const version = extractStringField(content, 'version')
    const description = extractStringField(content, 'description')

    if (!id || !name || !version) continue

    const hasRun = /run\s*[<(]/.test(content) || /run\s*:/.test(content)
    const hasSchema = /inputSchema:\s*z\.object/.test(content)

    return { id, name, version, description, type, hasRun, hasSchema }
  }

  return null
}

function fileExists(path: string): boolean {
  try {
    statSync(path)
    return true
  } catch {
    return false
  }
}

function extractStringField(
  content: string,
  field: string
): string | undefined {
  const match = content.match(new RegExp(`${field}:\\s*['"\`]([^'"\`]+)['"\`]`))
  return match ? match[1] : undefined
}

/**
 * Dynamically import a plugin module and return its run-capable object.
 */
export async function loadPlugin(
  pluginId: string
): Promise<PluginModule['default'] | null> {
  const pluginDir = join(PLUGINS_DIR, pluginId)

  for (const filename of ['index.tsx', 'index.ts']) {
    const filePath = join(pluginDir, filename)
    if (!fileExists(filePath)) continue

    try {
      const mod = (await import(filePath)) as PluginModule
      const plugin = mod.default

      if (!plugin?.run) {
        console.error(
          `Plugin ${pluginId} has no run() function — CLI unavailable`
        )
        return null
      }

      return plugin
    } catch (err) {
      console.error(`Failed to load plugin ${pluginId}:`, err)
      return null
    }
  }

  console.error(`Plugin ${pluginId}: no index.ts/tsx found`)
  return null
}
