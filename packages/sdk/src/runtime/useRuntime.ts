import type { PluginRuntimeContextValue } from '../types/ctx'

import { useContext } from 'react'

import { FlowToolRuntimeContext } from './context'
import { createMissingRuntimeContextError } from './errors'

/**
 * Read runtime context for SDK hooks.
 */
export function useRuntime(): PluginRuntimeContextValue {
  const ctx = useContext(FlowToolRuntimeContext)

  if (!ctx) {
    throw createMissingRuntimeContextError()
  }

  return ctx
}
