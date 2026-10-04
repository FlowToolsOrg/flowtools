import type { IndexedCommand } from './html-command-types'

import { htmlPluginCatalogSchema } from '@flowtools/sdk/compat/catalog'

import catalogData from '../data/html-plugin-catalog.json'

import { resolveCatalogEntry } from './catalog-entry'
import { injectHtmlPluginBridge } from './development-html-plugin-bridge'
import { assertUnsafeHtmlPreview } from './html-development-policy'

const catalog = htmlPluginCatalogSchema.parse(catalogData)

/** Development-only resolution. Never accept caller mode/certification/grant. */
export function resolveDevelopmentHtmlTarget(
  command: IndexedCommand
): string | undefined {
  assertUnsafeHtmlPreview()
  if (command.source !== 'html') return undefined
  if (!command.main || command.mainAvailable === false) return undefined
  if (/^https?:\/\//i.test(command.main)) return command.main
  const plugin = catalog.plugins.find(plugin => plugin.id === command.pluginId)
  if (!plugin || plugin.evidence.status !== 'entry-resolved') return undefined
  const root: unknown = import.meta.env.VITE_HTML_PLUGIN_ROOT
  const entry = resolveCatalogEntry(
    { packageRoot: plugin.package.root, entry: plugin.evidence.path },
    {
      development: true,
      checkoutRoot: typeof root === 'string' ? root : undefined,
    }
  )
  return entry ? encodeURI(`/@fs/${entry.replace(/\\/g, '/')}`) : undefined
}

/** Deny before network/HTML/preload reads. Fetch failure never opens a raw src. */
export async function loadDevelopmentHtmlFrame(
  command: IndexedCommand,
  target: string,
  options: { signal?: AbortSignal; request?: typeof fetch } = {}
): Promise<string> {
  assertUnsafeHtmlPreview()
  const request = options.request ?? globalThis.fetch
  const response = await request(target, {
    cache: 'no-store',
    signal: options.signal,
  })
  if (!response.ok)
    throw new Error('HTML development preview could not be loaded')
  const html = await response.text()
  const normalized = target.replace(/\\/g, '/')
  const baseUrl = normalized.slice(0, normalized.lastIndexOf('/') + 1)
  return injectHtmlPluginBridge(html, command, baseUrl)
}
