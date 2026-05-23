/**
 * CLI ToolContext factory.
 * Creates a lightweight context for running plugins in CLI mode.
 * No React dependency — only provides non-UI capabilities.
 */

import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

interface ToolContextShape {
  env: {
    pluginId: string
    pluginType: 'app' | 'tool'
    platform: 'desktop'
    mode: 'production'
  }
  ui: {
    toast: () => void
    openPanel: () => void
    closePanel: () => void
    log: () => void
  }
  signal: AbortSignal
  log: (
    level: string,
    message: string,
    details?: Record<string, unknown>
  ) => void
  storage: {
    get: (key: string) => unknown
    set: (key: string, value: unknown) => void
    delete: (key: string) => void
    clear: () => void
  }
  request: typeof fetch
  utils: { now: () => number }
  fs: undefined
  clipboard: undefined
  dialog: undefined
  notification: undefined
  db: undefined
  native: undefined
}

function getStorageDir(pluginId: string): string {
  const dir = join(tmpdir(), 'flowtools', pluginId, 'storage')
  mkdirSync(dir, { recursive: true })
  return dir
}

function createCLIStorage(pluginId: string) {
  const dir = getStorageDir(pluginId)

  return {
    get(key: string): unknown {
      const file = join(dir, `${key}.json`)
      if (!existsSync(file)) return undefined
      try {
        const raw = readFileSync(file, 'utf-8')
        return JSON.parse(raw) as unknown
      } catch {
        return undefined
      }
    },
    set(key: string, value: unknown): void {
      const file = join(dir, `${key}.json`)
      writeFileSync(file, JSON.stringify(value, null, 2), 'utf-8')
    },
    delete(key: string): void {
      const file = join(dir, `${key}.json`)
      if (existsSync(file)) {
        unlinkSync(file)
      }
    },
    clear(): void {
      if (existsSync(dir)) {
        rmSync(dir, { recursive: true, force: true })
        mkdirSync(dir, { recursive: true })
      }
    },
  }
}

/**
 * Create a ToolContext for CLI execution.
 * Provides storage, fetch, and logging. UI capabilities are no-ops.
 */
export function createCLIToolContext(pluginId: string): ToolContextShape {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 30_000)

  // Clear timeout when signal fires
  controller.signal.addEventListener('abort', () => clearTimeout(timeout), {
    once: true,
  })

  return {
    env: {
      pluginId,
      pluginType: 'app',
      platform: 'desktop',
      mode: 'production',
    },
    ui: {
      toast: () => {},
      openPanel: () => {},
      closePanel: () => {},
      log: () => {},
    },
    signal: controller.signal,
    log: (
      level: string,
      message: string,
      details?: Record<string, unknown>
    ) => {
      const prefix = `[${pluginId}] [${level}]`
      if (details) {
        process.stderr.write(
          `${prefix} ${message} ${JSON.stringify(details)}\n`
        )
      } else {
        process.stderr.write(`${prefix} ${message}\n`)
      }
    },
    storage: createCLIStorage(pluginId),
    request: ((url: string | URL | Request, init?: RequestInit) =>
      fetch(url, init)) as typeof fetch,
    utils: { now: () => Date.now() },
    fs: undefined,
    clipboard: undefined,
    dialog: undefined,
    notification: undefined,
    db: undefined,
    native: undefined,
  }
}
