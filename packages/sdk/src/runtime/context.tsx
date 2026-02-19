import type { PluginRuntimeContextValue } from '../types/ctx'

import { createContext } from 'react'

/**
 * Runtime-injected context consumed by SDK hooks.
 */
export const FlowToolRuntimeContext =
  createContext<PluginRuntimeContextValue | null>(null)
