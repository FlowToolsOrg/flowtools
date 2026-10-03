/**
 * Plugin discovery — scan plugins/ directory and load CLI-compatible plugins.
 * Loads index.tsx / index.ts directly. Only plugins with a `run()` function
 * are CLI-available.
 */

import type { CLIPluginInfo } from './types'
import type { FlowToolPlugin } from '@flowtools/sdk/types'

import { createHash } from 'node:crypto'
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

interface PluginModule {
  default: FlowToolPlugin
}

const PLUGINS_DIR = resolve(import.meta.dir, '..', '..', '..', 'plugins')
const HEADLESS_CACHE_DIR = join(tmpdir(), 'flowtools-cli-headless')

/**
 * Scan plugins directory and return info about CLI-available plugins.
 */
export function scanPlugins(): CLIPluginInfo[] {
  const plugins: CLIPluginInfo[] = []

  let entries: string[]
  try {
    entries = readdirSync(PLUGINS_DIR)
  } catch {
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

    const metaBlock = extractObjectBlock(content, 'meta') ?? content
    const id = extractStringField(metaBlock, 'id')
    const name = extractStringField(metaBlock, 'name')
    const version = extractStringField(metaBlock, 'version')
    const description = extractStringField(metaBlock, 'description')

    if (!id || !name || !version) continue

    const hasRun = /run\s*[<(]/.test(content) || /run\s*:/.test(content)
    const hasSchema =
      /inputSchema\s*=\s*z\.object/.test(content) ||
      /inputSchema:\s*z\.object/.test(content)

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

function extractObjectBlock(content: string, field: string): string | null {
  const fieldIndex = content.search(new RegExp(`${field}:\\s*{`))
  if (fieldIndex === -1) return null

  const openBraceIndex = content.indexOf('{', fieldIndex)
  if (openBraceIndex === -1) return null

  let depth = 0
  for (let i = openBraceIndex; i < content.length; i++) {
    const char = content[i]
    if (char === '{') depth++
    if (char === '}') depth--
    if (depth === 0) {
      return content.slice(openBraceIndex, i + 1)
    }
  }

  return null
}

/**
 * Dynamically import a plugin module and return its run-capable object.
 */
export async function loadPlugin(
  pluginId: string
): Promise<PluginModule['default'] | null> {
  const pluginDir = join(PLUGINS_DIR, pluginId)

  const distPath = join(PLUGINS_DIR, 'dist', `${pluginId}.js`)
  if (fileExists(distPath)) {
    const plugin = await importPlugin(distPath)
    if (plugin) return plugin
  }

  for (const filename of ['index.tsx', 'index.ts']) {
    const filePath = join(pluginDir, filename)
    if (!fileExists(filePath)) continue

    const plugin = await importPlugin(filePath)
    if (plugin) return plugin

    const headlessPlugin = await importHeadlessPlugin(filePath)
    if (headlessPlugin) return headlessPlugin
  }

  return null
}

async function importPlugin(
  filePath: string
): Promise<PluginModule['default'] | null> {
  try {
    const mod = (await import(pathToFileURL(filePath).href)) as PluginModule
    const plugin = mod.default

    if (!plugin?.run) {
      return null
    }

    return plugin
  } catch {
    return null
  }
}

async function importHeadlessPlugin(
  filePath: string
): Promise<PluginModule['default'] | null> {
  try {
    const source = readFileSync(filePath, 'utf-8')
    const headlessSource = toHeadlessSource(source)
    const hash = createHash('sha256')
      .update(filePath)
      .update(headlessSource)
      .digest('hex')
      .slice(0, 16)
    const outPath = join(HEADLESS_CACHE_DIR, `${hash}.ts`)

    mkdirSync(HEADLESS_CACHE_DIR, { recursive: true })
    if (!existsSync(outPath)) {
      writeFileSync(outPath, headlessSource, 'utf-8')
    }

    const mod = (await import(
      `${pathToFileURL(outPath).href}?v=${hash}`
    )) as PluginModule
    const plugin = mod.default

    if (!plugin?.run) {
      return null
    }

    return plugin
  } catch {
    return null
  }
}

function toHeadlessSource(source: string): string {
  const withoutImports = stripRuntimeImports(source)
  const withoutSetup = stripSetupProperty(withoutImports)

  return `${headlessRuntime()}\n${withoutSetup}`
}

function stripRuntimeImports(source: string): string {
  const blocked = [
    '@flowtools/sdk',
    '@flowtools/sdk/result',
    '@flowtools/ui/plugin',
    'react',
  ]
  const lines = source.split(/\r?\n/)
  const output: string[] = []

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    if (!line.trimStart().startsWith('import')) {
      output.push(line)
      continue
    }

    const importLines = [line]
    while (!importLines.at(-1)?.includes(' from ') && i + 1 < lines.length) {
      i++
      importLines.push(lines[i])
    }

    const importText = importLines.join('\n')
    if (blocked.some(moduleName => importText.includes(`'${moduleName}'`))) {
      continue
    }

    output.push(...importLines)
  }

  return output.join('\n')
}

function stripSetupProperty(source: string): string {
  const setupIndex = source.indexOf('\n  setup()')
  if (setupIndex === -1) return source

  const openBraceIndex = source.indexOf('{', setupIndex)
  if (openBraceIndex === -1) return source

  let depth = 0
  let endIndex = -1
  for (let i = openBraceIndex; i < source.length; i++) {
    const char = source[i]
    if (char === '{') depth++
    if (char === '}') depth--
    if (depth === 0) {
      endIndex = i + 1
      break
    }
  }

  if (endIndex === -1) return source

  const removeStart = source.lastIndexOf(',', setupIndex)
  const start = removeStart === -1 ? setupIndex : removeStart
  let end = endIndex
  while (source[end] === ',' || source[end] === '\r' || source[end] === '\n') {
    end++
  }

  return `${source.slice(0, start)}\n${source.slice(end)}`
}

function headlessRuntime(): string {
  return `
const definePlugin = plugin => plugin
const definePluginStore = store => store
const result = {
  text: text => ({ type: 'text', text }),
  json: value => ({ type: 'json', value }),
  table: (columns, rows) => ({ type: 'table', columns, rows }),
  open: target => ({ type: 'open', target }),
  multi: items => ({ type: 'multi', items }),
}
`
}
