/**
 * Namespaced key-value storage capability contract.
 */
export interface ZustandStorageAdapter {
  /**
   * Read a persisted string value by name.
   * Compatible with zustand persist `StateStorage`.
   */
  getItem: (name: string) => string | null
  /**
   * Persist a string value by name.
   * Compatible with zustand persist `StateStorage`.
   */
  setItem: (name: string, value: string) => void
  /**
   * Remove a persisted value by name.
   * Compatible with zustand persist `StateStorage`.
   */
  removeItem: (name: string) => void
}

export interface StorageCapability {
  /**
   * Read a value by key.
   */
  get: <T = unknown>(key: string) => T | undefined
  /**
   * Persist a value by key.
   */
  set: <T = unknown>(key: string, value: T) => void
  /**
   * Remove a value by key.
   */
  remove: (key: string) => void
  /**
   * Create a zustand persist-compatible storage adapter.
   * The optional namespace isolates multiple stores in one plugin.
   * Most plugins can use a single root store and omit namespace.
   */
  zustand: (namespace?: string) => ZustandStorageAdapter
}
