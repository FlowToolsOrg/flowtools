import { SDK_MARKERS } from '@flowtools/sdk'

interface FlowToolsGlobal {
  sdk: Record<string, unknown>
}

declare global {
  interface Window {
    [SDK_MARKERS.GLOBAL_KEY]?: FlowToolsGlobal
  }
}

export {}
