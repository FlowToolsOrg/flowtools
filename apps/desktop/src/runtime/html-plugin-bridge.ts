import type {
  Permission,
  PluginRuntimeContextValue,
} from '@flowtools/sdk/types'

import { openPath, openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'

export interface HtmlPluginBridgeCommand {
  id: string
  title: string
  description?: string
  type: string
  pluginId: string
  pluginName: string
  pluginType: 'app' | 'tool'
  permissions: readonly Permission[]
  featureCode?: string
}

export interface HtmlPluginBridgeRequest {
  type: 'flowtools:html-plugin-call'
  id: string
  method: string
  payload?: unknown
}

export interface HtmlPluginBridgeResponse {
  type: 'flowtools:html-plugin-response'
  id: string
  ok: boolean
  value?: unknown
  error?: string
}

const legacyHostApiName = ['z', 'tools'].join('')

export function isHtmlPluginBridgeRequest(
  value: unknown
): value is HtmlPluginBridgeRequest {
  if (!isRecord(value)) return false

  return (
    value.type === 'flowtools:html-plugin-call' &&
    typeof value.id === 'string' &&
    typeof value.method === 'string'
  )
}

export function createHtmlPluginBridgeScript(
  command: HtmlPluginBridgeCommand
): string {
  const metadata = {
    commandId: command.id,
    featureCode: command.featureCode ?? command.id,
    pluginId: command.pluginId,
    pluginName: command.pluginName,
    title: command.title,
    type: command.type,
    permissions: command.permissions,
  }

  return `
;(() => {
  const legacyHostApiName = ${serializeForScript(legacyHostApiName)}
  if (window[legacyHostApiName] && window.utools) return
  const meta = ${serializeForScript(metadata)}
  const pending = new Map()
  const enterHandlers = new Set()
  const outHandlers = new Set()
  const readyHandlers = new Set()
  const dbStoragePrefix = 'flowtools:legacy-html:' + meta.pluginId + ':dbStorage:'
  const dbPrefix = 'flowtools:legacy-html:' + meta.pluginId + ':db:'

  function callHost(method, payload) {
    return new Promise((resolve, reject) => {
      const id =
        Date.now().toString(36) + ':' + Math.random().toString(36).slice(2)
      pending.set(id, { resolve, reject })
      window.parent.postMessage(
        { type: 'flowtools:html-plugin-call', id, method, payload },
        '*'
      )
      window.setTimeout(() => {
        const entry = pending.get(id)
        if (!entry) return
        pending.delete(id)
        entry.reject(new Error('FlowTools host call timed out: ' + method))
      }, 30000)
    })
  }

  function parseStoredValue(raw) {
    if (raw === null) return undefined
    try {
      return JSON.parse(raw)
    } catch {
      return raw
    }
  }

  function writeStoredValue(key, value) {
    window.localStorage.setItem(key, JSON.stringify(value))
  }

  function createRevision() {
    return Date.now().toString(36) + '-' + Math.random().toString(36).slice(2)
  }

  function createId() {
    return 'doc-' + createRevision()
  }

  function getDoc(id) {
    return parseStoredValue(window.localStorage.getItem(dbPrefix + id)) ?? null
  }

  function putDoc(doc) {
    if (!doc || typeof doc !== 'object') {
      throw new Error('utools.db.put requires a document object')
    }

    const id = String(doc._id || createId())
    const next = { ...doc, _id: id, _rev: createRevision() }
    writeStoredValue(dbPrefix + id, next)
    return { ok: true, id, rev: next._rev }
  }

  const dbStorage = {
    getItem(key) {
      return parseStoredValue(window.localStorage.getItem(dbStoragePrefix + key))
    },
    setItem(key, value) {
      writeStoredValue(dbStoragePrefix + key, value)
      return true
    },
    removeItem(key) {
      window.localStorage.removeItem(dbStoragePrefix + key)
      return true
    },
    clear() {
      for (const key of Object.keys(window.localStorage)) {
        if (key.startsWith(dbStoragePrefix)) {
          window.localStorage.removeItem(key)
        }
      }
    },
  }

  const db = {
    get(id) {
      return getDoc(String(id))
    },
    put(doc) {
      return putDoc(doc)
    },
    post(doc) {
      return putDoc(doc)
    },
    remove(docOrId) {
      const id =
        typeof docOrId === 'string' ? docOrId : String(docOrId && docOrId._id)
      if (!id || id === 'undefined') return { ok: false }
      window.localStorage.removeItem(dbPrefix + id)
      return { ok: true, id, rev: createRevision() }
    },
    allDocs() {
      const docs = []
      for (const key of Object.keys(window.localStorage)) {
        if (key.startsWith(dbPrefix)) {
          const value = parseStoredValue(window.localStorage.getItem(key))
          if (value) docs.push(value)
        }
      }
      return docs
    },
    bulkDocs(docs) {
      return Array.isArray(docs) ? docs.map(doc => putDoc(doc)) : []
    },
  }

  function normalizeFilters(filters) {
    if (!Array.isArray(filters)) return undefined
    return filters.map(filter => ({
      name: String(filter.name || filter.label || 'Files'),
      extensions: Array.isArray(filter.extensions)
        ? filter.extensions.map(extension => String(extension).replace(/^\\./, ''))
        : [],
    }))
  }

  function normalizeOpenOptions(options) {
    if (!options || typeof options !== 'object') return {}
    return {
      title: options.title,
      multiple: Boolean(options.multiple),
      filters: normalizeFilters(options.filters),
    }
  }

  function normalizeSaveOptions(options) {
    if (!options || typeof options !== 'object') return {}
    return {
      title: options.title,
      defaultPath: options.defaultPath || options.defaultFilename,
      filters: normalizeFilters(options.filters),
    }
  }

  function runEnterHandlers(payload) {
    const input = payload || {
      code: meta.featureCode,
      type: meta.type,
      payload: '',
      option: {
        pluginId: meta.pluginId,
        commandId: meta.commandId,
      },
    }

    for (const handler of enterHandlers) {
      try {
        handler(input)
      } catch (error) {
        console.error(error)
      }
    }
  }

  const api = {
    db,
    dbStorage,
    isFlowTools: true,
    isHtmlPluginCompat: true,
    onPluginEnter(callback) {
      if (typeof callback === 'function') {
        enterHandlers.add(callback)
        window.queueMicrotask(() => callback({
          code: meta.featureCode,
          type: meta.type,
          payload: '',
          option: {
            pluginId: meta.pluginId,
            commandId: meta.commandId,
          },
        }))
      }
    },
    onPluginOut(callback) {
      if (typeof callback === 'function') outHandlers.add(callback)
    },
    onPluginReady(callback) {
      if (typeof callback === 'function') {
        readyHandlers.add(callback)
        window.queueMicrotask(callback)
      }
    },
    redirect(code, payload) {
      runEnterHandlers({
        code,
        type: meta.type,
        payload,
        option: {
          pluginId: meta.pluginId,
          commandId: meta.commandId,
        },
      })
    },
    outPlugin() {
      for (const handler of outHandlers) {
        try {
          handler()
        } catch (error) {
          console.error(error)
        }
      }
      return callHost('ui.closePanel')
    },
    showMainWindow() {
      return Promise.resolve()
    },
    hideMainWindow() {
      return Promise.resolve()
    },
    setExpendHeight() {
      return true
    },
    setSubInput() {
      return true
    },
    removeSubInput() {
      return true
    },
    setSubInputValue() {
      return true
    },
    subInputFocus() {
      return true
    },
    subInputBlur() {
      return true
    },
    copyText(text) {
      return callHost('clipboard.writeText', { text: String(text ?? '') })
    },
    readText() {
      return callHost('clipboard.readText')
    },
    showNotification(options) {
      const payload =
        typeof options === 'string'
          ? { title: meta.pluginName, message: options }
          : {
              title: options?.title || meta.pluginName,
              message: options?.body || options?.message || options?.content,
            }
      return callHost('notification.notify', payload)
    },
    showOpenDialog(options) {
      return callHost('dialog.openFile', normalizeOpenOptions(options))
    },
    showSaveDialog(options) {
      return callHost('dialog.saveFile', normalizeSaveOptions(options))
    },
    shellOpenExternal(url) {
      return callHost('opener.openUrl', { url: String(url) })
    },
    shellOpenPath(path) {
      return callHost('opener.openPath', { path: String(path) })
    },
    shellShowItemInFolder(path) {
      return callHost('opener.revealItemInDir', { path: String(path) })
    },
    getPath(name) {
      const key = String(name || '')
      const paths = {
        home: '',
        appData: '',
        userData: '',
        temp: '',
        desktop: '',
        documents: '',
        downloads: '',
        music: '',
        pictures: '',
        videos: '',
      }
      return paths[key] ?? ''
    },
    getNativeId() {
      return meta.pluginId
    },
    getUser() {
      return null
    },
    isWindows() {
      return /Win/i.test(window.navigator.platform)
    },
    isMacOS() {
      return /Mac/i.test(window.navigator.platform)
    },
    isLinux() {
      return /Linux/i.test(window.navigator.platform)
    },
    isDarkColors() {
      return window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false
    },
    findInPage() {
      return false
    },
    stopFindInPage() {
      return false
    },
    flowtools: {
      callHost,
      meta,
    },
  }

  window.addEventListener('message', event => {
    const message = event.data
    if (!message || message.type !== 'flowtools:html-plugin-response') return

    const entry = pending.get(message.id)
    if (!entry) return

    pending.delete(message.id)
    if (message.ok) {
      entry.resolve(message.value)
    } else {
      entry.reject(new Error(message.error || 'FlowTools host call failed'))
    }
  })

  window[legacyHostApiName] = api
  window.utools = api
  window.flowtoolsHtmlPluginBridge = api.flowtools

  for (const callback of readyHandlers) {
    try {
      callback()
    } catch (error) {
      console.error(error)
    }
  }
})()
`
}

export function injectHtmlPluginBridge(
  html: string,
  command: HtmlPluginBridgeCommand,
  baseUrl?: string
): string {
  const bridgeScript = createHtmlPluginBridgeScript(command).replace(
    /<\/script/gi,
    '<\\/script'
  )
  const baseTag = baseUrl ? `<base href="${escapeHtmlAttribute(baseUrl)}">` : ''
  const injection = `${baseTag}<script>${bridgeScript}</script>`

  if (/<head(\s[^>]*)?>/i.test(html)) {
    return html.replace(/<head(\s[^>]*)?>/i, match => `${match}${injection}`)
  }

  if (/<html(\s[^>]*)?>/i.test(html)) {
    return html.replace(
      /<html(\s[^>]*)?>/i,
      match => `${match}<head>${injection}</head>`
    )
  }

  return `${injection}${html}`
}

export async function handleHtmlPluginBridgeRequest(
  context: PluginRuntimeContextValue,
  request: HtmlPluginBridgeRequest
): Promise<unknown> {
  const payload = toRecord(request.payload)

  switch (request.method) {
    case 'ui.closePanel':
      context.ui.closePanel(readOptionalString(payload.panelId))
      return true
    case 'ui.toast':
      context.ui.toast({
        title: readString(payload.title, 'FlowTools'),
        message: readOptionalString(payload.message),
      })
      return true
    case 'clipboard.readText':
      return await requireCapability(context.clipboard, 'clipboard').readText()
    case 'clipboard.writeText':
      await requireCapability(context.clipboard, 'clipboard').writeText(
        readString(payload.text)
      )
      return true
    case 'notification.notify':
      await requireCapability(context.notification, 'notification').notify({
        title: readString(payload.title, context.env.pluginId),
        message: readOptionalString(payload.message),
      })
      return true
    case 'dialog.openFile':
      return await requireCapability(context.dialog, 'dialog').openFile({
        title: readOptionalString(payload.title),
        multiple: Boolean(payload.multiple),
        filters: readDialogFilters(payload.filters),
      })
    case 'dialog.saveFile':
      return await requireCapability(context.dialog, 'dialog').saveFile({
        title: readOptionalString(payload.title),
        defaultPath: readOptionalString(payload.defaultPath),
        filters: readDialogFilters(payload.filters),
      })
    case 'fs.readFile':
      return await requireCapability(context.fs, 'fs').readFile(
        readString(payload.path)
      )
    case 'fs.writeFile':
      await requireCapability(context.fs, 'fs').writeFile(
        readString(payload.path),
        readString(payload.data)
      )
      return true
    case 'db.query':
      return await requireCapability(context.db, 'db').query(
        readString(payload.sql),
        readUnknownArray(payload.params)
      )
    case 'native.invoke':
      return await requireCapability(context.native, 'native').invoke(
        readString(payload.command),
        toRecord(payload.payload)
      )
    case 'opener.openUrl':
      await openUrl(readString(payload.url))
      return true
    case 'opener.openPath':
      await openPath(readString(payload.path))
      return true
    case 'opener.revealItemInDir':
      await revealItemInDir(readString(payload.path))
      return true
    default:
      throw new Error(
        `Unsupported HTML plugin bridge method: ${request.method}`
      )
  }
}

function requireCapability<T>(capability: T | undefined, name: string): T {
  if (!capability) {
    throw new Error(`${name} capability is not granted for this plugin`)
  }

  return capability
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function toRecord(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {}
}

function readString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback
}

function readOptionalString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined
}

function readUnknownArray(value: unknown): readonly unknown[] | undefined {
  return Array.isArray(value) ? value : undefined
}

function readDialogFilters(value: unknown) {
  if (!Array.isArray(value)) return undefined

  return value
    .map(filter => {
      if (!isRecord(filter)) return undefined
      const extensions = Array.isArray(filter.extensions)
        ? filter.extensions.filter((extension): extension is string => {
            return typeof extension === 'string'
          })
        : []

      return {
        name: readString(filter.name, 'Files'),
        extensions,
      }
    })
    .filter((filter): filter is { name: string; extensions: string[] } => {
      return Boolean(filter)
    })
}

function serializeForScript(value: unknown): string {
  return JSON.stringify(value).replace(/</g, '\\u003c')
}

function escapeHtmlAttribute(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/"/g, '&quot;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}
