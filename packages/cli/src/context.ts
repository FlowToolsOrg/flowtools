import type {
  Permission,
  StorageCapability,
  ToolContext,
} from '@flowtools/sdk/types'

import { createHash } from 'node:crypto'
import {
  existsSync,
  mkdirSync,
  readFileSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

interface CLIContextOptions {
  pluginType?: 'app' | 'tool'
  permissions?: readonly Permission[]
  signal?: AbortSignal
}

function createCLIStorage(pluginId: string): StorageCapability {
  const dir = join(tmpdir(), 'flowtools', pluginId, 'storage')
  mkdirSync(dir, { recursive: true })
  const fileFor = (key: string) => {
    if (
      !/^[a-zA-Z0-9][a-zA-Z0-9_-]*$/.test(key) ||
      /^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/i.test(key)
    ) {
      throw new Error('Invalid storage key')
    }
    return join(dir, key + '.json')
  }
  const storage: StorageCapability = {
    get<T>(key: string): T | undefined {
      const file = fileFor(key)
      if (!existsSync(file)) return undefined
      return JSON.parse(readFileSync(file, 'utf-8')) as T
    },
    set(key, value) {
      const serialized = JSON.stringify(value)
      if (serialized === undefined)
        throw new Error('Storage value must be JSON serializable')
      writeFileSync(fileFor(key), serialized, 'utf-8')
    },
    remove(key) {
      const file = fileFor(key)
      if (existsSync(file)) unlinkSync(file)
    },
    zustand(namespace = '') {
      const keyFor = (name: string) =>
        'zustand-' +
        createHash('sha256')
          .update(JSON.stringify([namespace, name]))
          .digest('hex')
      return {
        getItem: name => storage.get<string>(keyFor(name)) ?? null,
        setItem: (name, value) => storage.set(keyFor(name), value),
        removeItem: name => storage.remove(keyFor(name)),
      }
    },
  }
  return storage
}

/** Trusted built-in adapter, not an authorization broker or OS sandbox. */
export function createCLIToolContext(
  pluginId: string,
  options: CLIContextOptions = {}
): ToolContext {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(pluginId))
    throw new Error('Invalid plugin id')
  const permissions = new Set(options.permissions ?? [])
  return {
    env: {
      pluginId,
      pluginType: options.pluginType ?? 'app',
      platform: 'desktop',
      mode: 'production',
    },
    ui: { toast: () => {}, openPanel: () => {}, closePanel: () => {} },
    // Execution owns timers; constructing a context must not keep the CLI alive.
    signal: options.signal ?? new AbortController().signal,
    log: () => {},
    storage: permissions.has('storage')
      ? createCLIStorage(pluginId)
      : undefined,
    request: permissions.has('network') ? fetch : undefined,
    utils: { now: Date.now },
  }
}
