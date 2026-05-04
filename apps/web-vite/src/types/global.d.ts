interface FlowToolsGlobal {
  sdk: Record<string, unknown>
}

declare global {
  interface Window {
    __FLOWTOOLS__?: FlowToolsGlobal
  }
}

export {}
