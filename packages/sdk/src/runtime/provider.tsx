import type { PluginRuntimeContextValue } from '../types/ctx'
import type { PropsWithChildren } from 'react'

import { FlowToolRuntimeContext } from './context'

export interface FlowToolRuntimeProviderProps extends PropsWithChildren<{
  value: PluginRuntimeContextValue
}> {}

/**
 * Host runtime provider that injects plugin context for SDK hooks.
 */
export function FlowToolRuntimeProvider({
  value,
  children,
}: FlowToolRuntimeProviderProps) {
  return (
    <FlowToolRuntimeContext.Provider value={value}>
      {children}
    </FlowToolRuntimeContext.Provider>
  )
}
