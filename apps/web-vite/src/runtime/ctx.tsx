import type {
  ClipboardCapability,
  DBCapability,
  DialogCapability,
  FSCapability,
  FileWriteData,
  NativeCapability,
  NotificationCapability,
  Permission,
  PluginStoreCapability,
  PluginStoreShape,
  PluginStoreState,
  PluginStoreUpdater,
  PluginRuntimeContextValue,
  PluginType,
  RequestCapability,
  RuntimeMode,
  StorageCapability,
  ToolContext,
  ToolLogLevel,
  UICapability,
} from '@flowtools/sdk'
import type { PropsWithChildren } from 'react'

import { FlowToolRuntimeProvider, APP_PREFIX } from '@flowtools/sdk'
import { pickCapability } from '@flowtools/sdk/utils'
import { createStore } from 'zustand/vanilla'

import {
  PLUGIN_STORE_STORAGE_KEY,
  PLUGIN_STORE_STORAGE_NAMESPACE,
  RUNTIME_PREFIX,
} from '../constants'

interface InternalPluginStoreState {
  state: PluginStoreState
}

export interface CreateWebRuntimeContextOptions {
  pluginId: string
  pluginType: PluginType
  permissions?: readonly Permission[]
  storeShape?: PluginStoreShape
  mode?: RuntimeMode
}

export interface CreateWebToolContextOptions {
  pluginId: string
  pluginType?: PluginType
  storeShape?: PluginStoreShape
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
    new CustomEvent(`flowtools:${eventName}`, {
      detail,
    })
  )
}

function createStorageKey(
  pluginId: string,
  kind: 'storage' | 'fs',
  key: string
): string {
  return `${APP_PREFIX}:${pluginId}:${kind}:${key}`
}

function createZustandStorageName(namespace: string | undefined, name: string) {
  const normalizedNamespace =
    namespace && namespace.trim().length > 0 ? namespace.trim() : 'default'

  return `zustand:${normalizedNamespace}:${name}`
}

const pluginStoreRegistry = new Map<string, PluginStoreCapability>()

function parsePluginStoreState(raw: string | null): PluginStoreState {
  if (!raw) {
    return {}
  }

  try {
    const parsed = JSON.parse(raw) as { state?: unknown }
    if (!parsed || typeof parsed !== 'object') {
      return {}
    }

    const state = parsed.state
    if (!state || typeof state !== 'object' || Array.isArray(state)) {
      return {}
    }

    return state as PluginStoreState
  } catch {
    return {}
  }
}

function readInitialPluginStoreState(
  pluginId: string,
  initialState: PluginStoreState | undefined,
  persistent: boolean
): PluginStoreState {
  if (!persistent) {
    return initialState ?? {}
  }

  const storage = createStorageCapability(pluginId)
  const raw = storage
    .zustand(PLUGIN_STORE_STORAGE_NAMESPACE)
    .getItem(PLUGIN_STORE_STORAGE_KEY)
  const persistedState = parsePluginStoreState(raw)

  return {
    ...initialState,
    ...persistedState,
  }
}

function persistPluginStoreState(
  pluginId: string,
  persistent: boolean,
  state: PluginStoreState
): void {
  if (!persistent) {
    return
  }

  const storage = createStorageCapability(pluginId)
  storage
    .zustand(PLUGIN_STORE_STORAGE_NAMESPACE)
    .setItem(PLUGIN_STORE_STORAGE_KEY, JSON.stringify({ state }))
}

function resolveNextPluginStoreState(
  current: PluginStoreState,
  updater: PluginStoreUpdater<PluginStoreState>,
  replace: boolean
): PluginStoreState {
  const update = typeof updater === 'function' ? updater(current) : updater

  if (replace) {
    return update as PluginStoreState
  }

  return {
    ...current,
    ...update,
  }
}

function createPluginStoreCapability(
  pluginId: string,
  shape: PluginStoreShape | undefined,
  persistent: boolean
): PluginStoreCapability {
  const initialState = shape?.initialState ?? {}
  const actionsFactory = shape?.actions

  const initialSnapshot = readInitialPluginStoreState(
    pluginId,
    initialState,
    persistent
  )
  const store = createStore<InternalPluginStoreState>(() => ({
    state: initialSnapshot,
  }))

  if (persistent) {
    persistPluginStoreState(pluginId, true, initialSnapshot)
    store.subscribe(snapshot => {
      persistPluginStoreState(pluginId, true, snapshot.state)
    })
  }

  const setState = (
    updater: PluginStoreUpdater<PluginStoreState>,
    replace = false
  ) => {
    const next = resolveNextPluginStoreState(
      store.getState().state,
      updater,
      replace
    )
    store.setState({
      state: next,
    })
  }

  const getState = () => store.getState().state

  const actions = actionsFactory ? actionsFactory(setState, getState) : {}

  const capability: PluginStoreCapability = {
    getState,
    setState,
    subscribe(listener) {
      return store.subscribe(() => {
        listener()
      })
    },
    reset() {
      store.setState({
        state: { ...initialSnapshot },
      })
    },
    actions,
  }

  return capability
}

function getOrCreatePluginStoreCapability(
  pluginId: string,
  shape: PluginStoreShape | undefined,
  persistent: boolean
): PluginStoreCapability {
  const existing = pluginStoreRegistry.get(pluginId)
  if (existing) {
    return existing
  }

  const capability = createPluginStoreCapability(pluginId, shape, persistent)
  pluginStoreRegistry.set(pluginId, capability)

  return capability
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
    zustand(namespace) {
      return {
        getItem(name) {
          const storageName = createZustandStorageName(namespace, name)
          const storageKey = createStorageKey(pluginId, 'storage', storageName)

          return safeGetLocalStorageItem(storageKey)
        },
        setItem(name, value) {
          const storageName = createZustandStorageName(namespace, name)
          const storageKey = createStorageKey(pluginId, 'storage', storageName)

          safeSetLocalStorageItem(storageKey, value)
        },
        removeItem(name) {
          const storageName = createZustandStorageName(namespace, name)
          const storageKey = createStorageKey(pluginId, 'storage', storageName)

          safeRemoveLocalStorageItem(storageKey)
        },
      }
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
  const storePersistent = allowedPermissions.has('storage')
  const storeCapability =
    options.pluginType === 'app'
      ? getOrCreatePluginStoreCapability(
          options.pluginId,
          options.storeShape,
          storePersistent
        )
      : undefined

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
    store: storeCapability,
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
    pluginType: options.pluginType ?? 'tool',
    storeShape: options.storeShape,
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
  storeShape,
  mode,
  children,
}: WebPluginRuntimeProviderProps) {
  const value = createWebRuntimeContext({
    mode,
    permissions,
    storeShape,
    pluginId,
    pluginType,
  })

  return (
    <FlowToolRuntimeProvider value={value}>{children}</FlowToolRuntimeProvider>
  )
}
