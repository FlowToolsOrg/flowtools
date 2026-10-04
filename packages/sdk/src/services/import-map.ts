import { GLOBAL_KEY, IMPORT_MAP_ID } from '../constants'

import { assertUnsafeDevelopmentPreview } from './development-policy'

interface FlowToolsGlobal {
  sdk: Record<string, unknown>
}

declare global {
  interface Window {
    [GLOBAL_KEY]?: FlowToolsGlobal
  }
}

/**
 * Get the CDN URL for a package.
 */
function cdnUrl(pkg: string, version: string): string {
  return `https://esm.sh/${pkg}@${version}`
}

/**
 * Create a blob URL bridge module that re-exports the host's SDK
 * from `window.[GLOBAL_KEY]`.
 */
function createSdkBridgeUrl(): string {
  const code = `const g = window['${GLOBAL_KEY}'];export default g.sdk;const{definePlugin,PluginFileLoader,PluginRegistry,PluginLoader,CommandRegistry,PluginLifecycleManager,PluginErrorBoundary,withWatchdog,FlowToolRuntimeContext,FlowToolRuntimeProvider,result,z}=g.sdk;export{definePlugin,PluginFileLoader,PluginRegistry,PluginLoader,CommandRegistry,PluginLifecycleManager,PluginErrorBoundary,withWatchdog,FlowToolRuntimeContext,FlowToolRuntimeProvider,result,z};`
  const blob = new Blob([code], { type: 'application/javascript' })

  return URL.createObjectURL(blob)
}

export interface ImportMapConfig {
  reactVersion?: string
  reactDomVersion?: string
}

/**
 * Inject an import map into the document so that dynamically loaded
 * plugin modules can resolve bare specifiers (`react`, `react-dom`,
 * `@flowtools/sdk`).
 *
 * Call this once during bootstrap, after the host SDK is available.
 */
export function setupImportMap(
  sdk: Record<string, unknown>,
  config?: ImportMapConfig
): void {
  assertUnsafeDevelopmentPreview()
  if (typeof document === 'undefined') {
    return
  }

  if (document.getElementById(IMPORT_MAP_ID)) {
    return
  }

  const reactVer = config?.reactVersion ?? '19'
  const reactDomVer = config?.reactDomVersion ?? '19'

  // Expose host SDK on window for the bridge module
  if (!window[GLOBAL_KEY]) {
    window[GLOBAL_KEY] = { sdk }
  }

  const sdkBridgeUrl = createSdkBridgeUrl()

  const importMap = {
    imports: {
      react: cdnUrl('react', reactVer),
      'react-dom': cdnUrl('react-dom', reactDomVer),
      'react-dom/client': cdnUrl('react-dom', reactDomVer) + '/client',
      'react/jsx-runtime': cdnUrl('react', reactVer) + '/jsx-runtime',
      '@flowtools/sdk': sdkBridgeUrl,
      '@flowtools/sdk/utils': sdkBridgeUrl,
    },
  }

  const el = document.createElement('script')

  el.id = IMPORT_MAP_ID
  el.type = 'importmap'
  el.textContent = JSON.stringify(importMap)
  document.head.prepend(el)
}
