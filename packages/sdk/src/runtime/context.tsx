import { createContext } from 'react'

import type { PluginRuntimeContextValue } from '../types/ctx'

/**
 * Runtime-injected context consumed by SDK hooks.
 */
export const FlowToolRuntimeContext =
  createContext<PluginRuntimeContextValue | null>(null)
