import type {
  DBRow,
  DialogFilter,
  FileWriteData,
  Permission,
  PluginRuntimeContextValue,
  RuntimeMode,
} from '@flowtools/sdk/types'

import { pickCapability } from '@flowtools/sdk/utils/capability'

import { invoke } from '@tauri-apps/api/core'
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager'
import {
  open as openDialog,
  save as saveDialog,
} from '@tauri-apps/plugin-dialog'
import { readTextFile, writeFile, writeTextFile } from '@tauri-apps/plugin-fs'
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'
import Database from '@tauri-apps/plugin-sql'
import { LazyStore } from '@tauri-apps/plugin-store'

interface DesktopRuntimeOptions {
  pluginId: string
  pluginType: 'app' | 'tool'
  permissions?: readonly Permission[]
  mode?: RuntimeMode
}

const storagePrefix = 'flowtools:desktop:plugin'
const nativeStores = new Map<string, LazyStore>()
const sqlDatabase = Database.get('sqlite:flowtools.db')

export function isTauriRuntime(): boolean {
  return Boolean(
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  )
}

export function resolveRuntimeMode(): RuntimeMode {
  if (import.meta.env.MODE === 'test') return 'test'
  return import.meta.env.DEV ? 'development' : 'production'
}

export function createDesktopRuntimeContext(
  options: DesktopRuntimeOptions
): PluginRuntimeContextValue {
  const allowedPermissions = new Set(options.permissions ?? [])

  return {
    env: {
      pluginId: options.pluginId,
      pluginType: options.pluginType,
      platform: 'desktop',
      mode: options.mode ?? resolveRuntimeMode(),
    },
    ui: createUiCapability(options.pluginId),
    fs: pickCapability('fs', allowedPermissions, createFsCapability),
    request: pickCapability(
      'network',
      allowedPermissions,
      createRequestCapability
    ),
    clipboard: pickCapability(
      'clipboard',
      allowedPermissions,
      createClipboardCapability
    ),
    dialog: pickCapability(
      'dialog',
      allowedPermissions,
      createDialogCapability
    ),
    notification: pickCapability(
      'notification',
      allowedPermissions,
      createNotificationCapability
    ),
    storage: pickCapability('storage', allowedPermissions, () =>
      createStorageCapability(options.pluginId)
    ),
    db: pickCapability('db', allowedPermissions, createDbCapability),
    native: pickCapability(
      'native',
      allowedPermissions,
      createNativeCapability
    ),
    utils: {
      now: () => Date.now(),
    },
  }
}

function createUiCapability(pluginId: string): PluginRuntimeContextValue['ui'] {
  return {
    toast: input => {
      window.dispatchEvent(
        new CustomEvent('flowtools:toast', {
          detail: {
            pluginId,
            ...input,
          },
        })
      )
    },
    openPanel: options => {
      window.dispatchEvent(
        new CustomEvent('flowtools:open-panel', {
          detail: {
            pluginId,
            ...options,
          },
        })
      )
    },
    closePanel: panelId => {
      window.dispatchEvent(
        new CustomEvent('flowtools:close-panel', {
          detail: {
            pluginId,
            panelId,
          },
        })
      )
    },
  }
}

function createRequestCapability(): PluginRuntimeContextValue['request'] {
  return (input, init) => fetch(input, init)
}

function createClipboardCapability(): PluginRuntimeContextValue['clipboard'] {
  return {
    readText: async () => {
      if (isTauriRuntime()) {
        return await readText()
      }

      return await navigator.clipboard.readText()
    },
    writeText: async value => {
      if (isTauriRuntime()) {
        await writeText(value)
        return
      }

      await navigator.clipboard.writeText(value)
    },
  }
}

function createDialogCapability(): PluginRuntimeContextValue['dialog'] {
  return {
    openFile: async options => {
      const selected = await openDialog({
        title: options?.title,
        multiple: options?.multiple,
        filters: toTauriDialogFilters(options?.filters),
        fileAccessMode: 'scoped',
      })

      if (selected === null) return null
      return Array.isArray(selected) ? selected : [selected]
    },
    saveFile: async options => {
      return await saveDialog({
        title: options?.title,
        defaultPath: options?.defaultPath,
        filters: toTauriDialogFilters(options?.filters),
      })
    },
  }
}

function createNotificationCapability(): PluginRuntimeContextValue['notification'] {
  return {
    notify: async input => {
      if (isTauriRuntime()) {
        let permissionGranted = await isPermissionGranted()

        if (!permissionGranted) {
          permissionGranted = (await requestPermission()) === 'granted'
        }

        if (permissionGranted) {
          sendNotification({
            title: input.title,
            body: input.message,
          })
        }

        return
      }

      if (!('Notification' in window)) return

      let permission = Notification.permission
      if (permission === 'default') {
        permission = await Notification.requestPermission()
      }

      if (permission === 'granted') {
        new Notification(input.title, { body: input.message })
      }
    },
  }
}

function createFsCapability(): PluginRuntimeContextValue['fs'] {
  return {
    readFile: path => readTextFile(path),
    writeFile: async (path, data) => {
      if (isTextWriteData(data)) {
        await writeTextFile(path, data)
        return
      }

      await writeFile(path, data)
    },
  }
}

function createStorageCapability(
  pluginId: string
): PluginRuntimeContextValue['storage'] {
  return {
    get: key => readJsonStorageValue(toStorageKey(pluginId, key)),
    set: (key, value) => {
      const storageKey = toStorageKey(pluginId, key)
      const serialized = JSON.stringify(value)
      window.localStorage.setItem(storageKey, serialized)
      void persistNativeStoreValue(pluginId, storageKey, value)
    },
    remove: key => {
      const storageKey = toStorageKey(pluginId, key)
      window.localStorage.removeItem(storageKey)
      void removeNativeStoreValue(pluginId, storageKey)
    },
    zustand: namespace => ({
      getItem: name => {
        return window.localStorage.getItem(
          toStorageKey(pluginId, name, namespace)
        )
      },
      setItem: (name, value) => {
        const storageKey = toStorageKey(pluginId, name, namespace)
        window.localStorage.setItem(storageKey, value)
        void persistNativeStoreValue(pluginId, storageKey, value)
      },
      removeItem: name => {
        const storageKey = toStorageKey(pluginId, name, namespace)
        window.localStorage.removeItem(storageKey)
        void removeNativeStoreValue(pluginId, storageKey)
      },
    }),
  }
}

function createDbCapability(): PluginRuntimeContextValue['db'] {
  return {
    query: async (sql, params) => {
      return await sqlDatabase.select(sql, toSqlParams(params))
    },
    insert: async (table, data) => {
      const entries = Object.entries(data)
      if (entries.length === 0) {
        throw new Error('Cannot insert an empty row')
      }

      const columns = entries.map(([column]) => quoteIdentifier(column))
      const placeholders = entries.map((_, index) => `$${index + 1}`)
      const result = await sqlDatabase.execute(
        `INSERT INTO ${quoteIdentifier(table)} (${columns.join(
          ', '
        )}) VALUES (${placeholders.join(', ')})`,
        entries.map(([, value]) => value)
      )

      return result.rowsAffected
    },
    update: async (table, data, where) => {
      const dataEntries = Object.entries(data)
      if (dataEntries.length === 0) {
        throw new Error('Cannot update with an empty row')
      }

      const whereClause = createWhereClause(where, dataEntries.length)
      const assignments = dataEntries.map(([column], index) => {
        return `${quoteIdentifier(column)} = $${index + 1}`
      })
      const result = await sqlDatabase.execute(
        `UPDATE ${quoteIdentifier(table)} SET ${assignments.join(', ')} ${
          whereClause.sql
        }`,
        [...dataEntries.map(([, value]) => value), ...whereClause.values]
      )

      return result.rowsAffected
    },
    delete: async (table, where) => {
      const whereClause = createWhereClause(where, 0)
      const result = await sqlDatabase.execute(
        `DELETE FROM ${quoteIdentifier(table)} ${whereClause.sql}`,
        whereClause.values
      )

      return result.rowsAffected
    },
  }
}

function createNativeCapability(): PluginRuntimeContextValue['native'] {
  return {
    invoke: async (command, payload) => {
      return await invoke(command, payload)
    },
  }
}

function toTauriDialogFilters(
  filters?: readonly DialogFilter[]
): Array<{ name: string; extensions: string[] }> | undefined {
  return filters?.map(filter => ({
    name: filter.name,
    extensions: [...filter.extensions],
  }))
}

function isTextWriteData(data: FileWriteData): data is string {
  return typeof data === 'string'
}

function toStorageKey(
  pluginId: string,
  key: string,
  namespace = 'root'
): string {
  return [
    storagePrefix,
    sanitizeKeySegment(pluginId),
    sanitizeKeySegment(namespace),
    key,
  ].join(':')
}

function readJsonStorageValue<T = unknown>(key: string): T | undefined {
  const raw = window.localStorage.getItem(key)
  if (raw === null) return undefined

  try {
    return JSON.parse(raw) as T
  } catch {
    return undefined
  }
}

async function persistNativeStoreValue(
  pluginId: string,
  key: string,
  value: unknown
): Promise<void> {
  if (!isTauriRuntime()) return

  const store = getNativeStore(pluginId)
  await store.set(key, value)
  await store.save()
}

async function removeNativeStoreValue(
  pluginId: string,
  key: string
): Promise<void> {
  if (!isTauriRuntime()) return

  const store = getNativeStore(pluginId)
  await store.delete(key)
  await store.save()
}

function getNativeStore(pluginId: string): LazyStore {
  const path = `flowtools-plugin-${sanitizeKeySegment(pluginId)}.json`
  const cached = nativeStores.get(path)
  if (cached) return cached

  const store = new LazyStore(path, {
    defaults: {},
    autoSave: 100,
  })
  nativeStores.set(path, store)
  return store
}

function toSqlParams(params?: readonly unknown[]): unknown[] | undefined {
  return params ? [...params] : undefined
}

function createWhereClause(
  where: DBRow,
  offset: number
): { sql: string; values: unknown[] } {
  const entries = Object.entries(where)
  if (entries.length === 0) {
    throw new Error('Refusing to write without a where condition')
  }

  return {
    sql: `WHERE ${entries
      .map(([column], index) => {
        return `${quoteIdentifier(column)} = $${offset + index + 1}`
      })
      .join(' AND ')}`,
    values: entries.map(([, value]) => value),
  }
}

function quoteIdentifier(value: string): string {
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(value)) {
    throw new Error(`Invalid SQL identifier: ${value}`)
  }

  return `"${value}"`
}

function sanitizeKeySegment(value: string): string {
  return value.replace(/[^A-Za-z0-9_.-]/g, '_') || 'plugin'
}
