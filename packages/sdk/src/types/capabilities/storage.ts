/**
 * Namespaced key-value storage capability contract.
 */
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
}
