import type { PluginRuntimeContextValue } from '../types/ctx'

import { createMissingCapabilityError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'

// ---------------------------------------------------------------------------
// Factories
// ---------------------------------------------------------------------------

type ContextKey = keyof PluginRuntimeContextValue

export function createRequiredCapabilityHook<K extends ContextKey>(
  key: K
): () => PluginRuntimeContextValue[K] {
  return function () {
    const ctx = useRuntime()
    return ctx[key]
  }
}

export function createOptionalCapabilityHook<K extends ContextKey>(
  key: K
): () => NonNullable<PluginRuntimeContextValue[K]> {
  return function () {
    const ctx = useRuntime()
    const value = ctx[key]

    if (!value) {
      throw createMissingCapabilityError(key)
    }

    return value as NonNullable<PluginRuntimeContextValue[K]>
  }
}

// ---------------------------------------------------------------------------
// Optional capability hooks
// ---------------------------------------------------------------------------

export const useClipboard = createOptionalCapabilityHook('clipboard')
export const useDB = createOptionalCapabilityHook('db')
export const useDialog = createOptionalCapabilityHook('dialog')
export const useFS = createOptionalCapabilityHook('fs')
export const useNative = createOptionalCapabilityHook('native')
export const useNotification = createOptionalCapabilityHook('notification')
export const useRequest = createOptionalCapabilityHook('request')
export const useStorage = createOptionalCapabilityHook('storage')

// ---------------------------------------------------------------------------
// Required capability hooks
// ---------------------------------------------------------------------------

export const useEnv = createRequiredCapabilityHook('env')
export const useUI = createRequiredCapabilityHook('ui')

// ---------------------------------------------------------------------------
// Re-exports (files with independent logic)
// ---------------------------------------------------------------------------

export {
  useCapability,
  type CapabilitySelector,
  type FullCapability,
  type SelectedCapability,
} from './useCapability'

export { usePluginStore } from './usePluginStore'
export { usePluginStoreApi } from './usePluginStoreApi'
