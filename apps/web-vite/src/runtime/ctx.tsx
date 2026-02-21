import type {
  ClipboardCapability,
  DBCapability,
  DialogCapability,
  FSCapability,
  FileWriteData,
  NativeCapability,
  NotificationCapability,
  Permission,
  PluginRuntimeContextValue,
  PluginType,
  RequestCapability,
  RuntimeMode,
  StorageCapability,
  ToolContext,
  ToolLogLevel,
  UICapability,
} from '@flow-tool/sdk'
import type { PropsWithChildren } from 'react'

import { FlowToolRuntimeProvider } from '@flow-tool/sdk'
import { pickCapability } from '@flow-tool/sdk/utils'

const RUNTIME_PREFIX = '[flow-tool-web-runtime]'
const STORAGE_PREFIX = 'flow-tool'

export interface CreateWebRuntimeContextOptions {
  pluginId: string
  pluginType: PluginType
  permissions?: readonly Permission[]
  mode?: RuntimeMode
}

export interface CreateWebToolContextOptions {
  pluginId: string
  permissions?: readonly Permission[]
  mode?: RuntimeMode
  signal?: AbortSignal
  log?: ToolContext['log']
}

export interface WebPluginRuntimeProviderProps extends PropsWithChildren<CreateWebRuntimeContextOptions> {}

type RuntimeEventName = 'toast' | 'panel-open' | 'panel-close' | 'tool-log'

type RuntimeEventDetail =
  | Record<string, unknown>
  | {
      pluginId: string
      level: ToolLogLevel
      message: string
      details?: Record<string, unknown>
    }

function normalizePermissions(
  permissions?: readonly Permission[]
): readonly Permission[] {
  if (!permissions || permissions.length === 0) {
    return []
  }

  return [...new Set(permissions)]
}

function resolveRuntimeMode(mode?: RuntimeMode): RuntimeMode {
  if (mode) {
    return mode
  }

  if (import.meta.env.MODE === 'test') {
    return 'test'
  }

  return import.meta.env.PROD ? 'production' : 'development'
}

function createCapabilityError(capability: string, message: string): Error {
  return new Error(`${RUNTIME_PREFIX} [${capability}] ${message}`)
}

function createUnsupportedCapabilityError(capability: string): Error {
  return createCapabilityError(capability, 'Not supported on web runtime.')
}

function dispatchRuntimeEvent(
  eventName: RuntimeEventName,
  detail: RuntimeEventDetail
): void {
  if (typeof window === 'undefined') {
    return
  }

  window.dispatchEvent(
    new CustomEvent(`flow-tool:${eventName}`, {
      detail,
    })
  )
}

function createStorageKey(
  pluginId: string,
  kind: 'storage' | 'fs',
  key: string
): string {
  return `${STORAGE_PREFIX}:${pluginId}:${kind}:${key}`
}

function safeGetLocalStorageItem(key: string): string | null {
  if (typeof window === 'undefined') {
    return null
  }

  try {
    return window.localStorage.getItem(key)
  } catch {
    throw createCapabilityError('storage', 'localStorage is unavailable.')
  }
}

function safeSetLocalStorageItem(key: string, value: string): void {
  if (typeof window === 'undefined') {
    throw createCapabilityError('storage', 'window is unavailable.')
  }

  try {
    window.localStorage.setItem(key, value)
  } catch {
    throw createCapabilityError('storage', 'Failed to persist value.')
  }
}

function safeRemoveLocalStorageItem(key: string): void {
  if (typeof window === 'undefined') {
    return
  }

  try {
    window.localStorage.removeItem(key)
  } catch {
    throw createCapabilityError('storage', 'Failed to remove value.')
  }
}

function createUICapability(pluginId: string): UICapability {
  return {
    toast(input) {
      dispatchRuntimeEvent('toast', {
        pluginId,
        ...input,
      })
    },
    openPanel(options) {
      dispatchRuntimeEvent('panel-open', {
        pluginId,
        options: options ?? {},
      })
    },
    closePanel(panelId) {
      dispatchRuntimeEvent('panel-close', {
        pluginId,
        panelId,
      })
    },
  }
}

function createRequestCapability(): RequestCapability {
  return fetch
}

function createClipboardCapability(): ClipboardCapability {
  return {
    async readText() {
      if (typeof navigator === 'undefined' || !navigator.clipboard) {
        throw createUnsupportedCapabilityError('clipboard')
      }

      return navigator.clipboard.readText()
    },
    async writeText(value) {
      if (typeof navigator === 'undefined' || !navigator.clipboard) {
        throw createUnsupportedCapabilityError('clipboard')
      }

      return navigator.clipboard.writeText(value)
    },
  }
}

function createDialogCapability(): DialogCapability {
  return {
    async openFile() {
      throw createUnsupportedCapabilityError('dialog')
    },
    async saveFile() {
      throw createUnsupportedCapabilityError('dialog')
    },
  }
}

function createNotificationCapability(
  pluginId: string
): NotificationCapability {
  return {
    async notify(input) {
      if (
        typeof window === 'undefined' ||
        typeof window.Notification === 'undefined'
      ) {
        dispatchRuntimeEvent('toast', {
          pluginId,
          ...input,
        })
        return
      }

      if (window.Notification.permission === 'granted') {
        new window.Notification(input.title, {
          body: input.message,
        })
        return
      }

      if (window.Notification.permission !== 'default') {
        return
      }

      const permission = await window.Notification.requestPermission()

      if (permission === 'granted') {
        new window.Notification(input.title, {
          body: input.message,
        })
      }
    },
  }
}

function createStorageCapability(pluginId: string): StorageCapability {
  return {
    get<T>(key: string) {
      const storageKey = createStorageKey(pluginId, 'storage', key)
      const raw = safeGetLocalStorageItem(storageKey)

      if (raw === null) {
        return undefined
      }

      try {
        return JSON.parse(raw) as T
      } catch {
        throw createCapabilityError(
          'storage',
          `Value at "${key}" is not valid JSON.`
        )
      }
    },
    set<T>(key: string, value: T) {
      const storageKey = createStorageKey(pluginId, 'storage', key)
      const serialized = JSON.stringify(value)

      safeSetLocalStorageItem(storageKey, serialized)
    },
    remove(key) {
      const storageKey = createStorageKey(pluginId, 'storage', key)

      safeRemoveLocalStorageItem(storageKey)
    },
  }
}

function decodeFileWriteData(data: FileWriteData): string {
  if (typeof data === 'string') {
    return data
  }

  return new TextDecoder().decode(data)
}

function createFSCapability(pluginId: string): FSCapability {
  return {
    async readFile(path) {
      const storageKey = createStorageKey(pluginId, 'fs', path)
      const raw = safeGetLocalStorageItem(storageKey)

      if (raw === null) {
        throw createCapabilityError('fs', `File "${path}" does not exist.`)
      }

      return raw
    },
    async writeFile(path, data) {
      const storageKey = createStorageKey(pluginId, 'fs', path)
      const value = decodeFileWriteData(data)

      safeSetLocalStorageItem(storageKey, value)
    },
  }
}

function createDBCapability(): DBCapability {
  return {
    async query() {
      throw createUnsupportedCapabilityError('db')
    },
    async insert() {
      throw createUnsupportedCapabilityError('db')
    },
    async update() {
      throw createUnsupportedCapabilityError('db')
    },
    async delete() {
      throw createUnsupportedCapabilityError('db')
    },
  }
}

function createNativeCapability(): NativeCapability {
  return {
    async invoke() {
      throw createUnsupportedCapabilityError('native')
    },
  }
}

function createDefaultToolLogger(pluginId: string): ToolContext['log'] {
  return (level, message, details) => {
    dispatchRuntimeEvent('tool-log', {
      pluginId,
      level,
      message,
      details,
    })
  }
}

export function createWebRuntimeContext(
  options: CreateWebRuntimeContextOptions
): PluginRuntimeContextValue {
  const permissions = normalizePermissions(options.permissions)
  const allowedPermissions = new Set<Permission>(permissions)

  return {
    env: {
      mode: resolveRuntimeMode(options.mode),
      platform: 'web',
      pluginId: options.pluginId,
      pluginType: options.pluginType,
    },
    ui: createUICapability(options.pluginId),
    fs: pickCapability('fs', allowedPermissions, () =>
      createFSCapability(options.pluginId)
    ),
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
    notification: pickCapability('notification', allowedPermissions, () =>
      createNotificationCapability(options.pluginId)
    ),
    storage: pickCapability('storage', allowedPermissions, () =>
      createStorageCapability(options.pluginId)
    ),
    db: pickCapability('db', allowedPermissions, createDBCapability),
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

export function createWebToolContext(
  options: CreateWebToolContextOptions
): ToolContext {
  const runtimeContext = createWebRuntimeContext({
    mode: options.mode,
    permissions: options.permissions,
    pluginId: options.pluginId,
    pluginType: 'tool',
  })

  return {
    ...runtimeContext,
    signal: options.signal ?? new AbortController().signal,
    log: options.log ?? createDefaultToolLogger(options.pluginId),
  }
}

export function WebPluginRuntimeProvider({
  pluginId,
  pluginType,
  permissions,
  mode,
  children,
}: WebPluginRuntimeProviderProps) {
  const value = createWebRuntimeContext({
    mode,
    permissions,
    pluginId,
    pluginType,
  })

  return (
    <FlowToolRuntimeProvider value={value}>{children}</FlowToolRuntimeProvider>
  )
}
