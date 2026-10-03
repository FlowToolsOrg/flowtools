import { existsSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const pluginEntryFiles = ['index.tsx', 'index.ts'] as const

export function getPluginEntries(packageDir: string): Record<string, string> {
  const entries: Record<string, string> = {}

  for (const directory of readdirSync(packageDir, { withFileTypes: true })) {
    if (!directory.isDirectory() || !directory.name.startsWith('plugin-')) {
      continue
    }

    const entryFile = pluginEntryFiles.find(file =>
      existsSync(join(packageDir, directory.name, file))
    )
    if (!entryFile) {
      throw new Error(`Missing plugin entry point: ${directory.name}`)
    }

    entries[directory.name] = `${directory.name}/${entryFile}`
  }

  return Object.fromEntries(
    Object.entries(entries).sort(([left], [right]) => left.localeCompare(right))
  )
}
