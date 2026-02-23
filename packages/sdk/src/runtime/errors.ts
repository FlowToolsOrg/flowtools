/**
 * Error prefix for SDK runtime guard failures.
 */
const SDK_ERROR_PREFIX = '[flow-tool-sdk]'

/**
 * Error thrown when hooks run outside the runtime provider.
 */
export class MissingRuntimeContextError extends Error {
  constructor() {
    super(
      `${SDK_ERROR_PREFIX} Runtime context is missing. ` +
        'Make sure the host runtime wraps plugin UI with provider.'
    )
    this.name = 'MissingRuntimeContextError'
  }
}

/**
 * Error thrown when a plugin requests a capability without permission.
 */
export class MissingCapabilityError extends Error {
  constructor(capability: string) {
    super(
      `${SDK_ERROR_PREFIX} Capability "${capability}" is unavailable. ` +
        'Declare the corresponding permission and ensure runtime injects it.'
    )
    this.name = 'MissingCapabilityError'
  }
}

/**
 * Error thrown when app store hook is used but host did not inject store.
 */
export class MissingPluginStoreError extends Error {
  constructor() {
    super(
      `${SDK_ERROR_PREFIX} Plugin store is unavailable. ` +
        'Ensure host runtime injects store for app plugins.'
    )
    this.name = 'MissingPluginStoreError'
  }
}

/**
 * Create a standard missing runtime context error.
 */
export function createMissingRuntimeContextError(): MissingRuntimeContextError {
  return new MissingRuntimeContextError()
}

/**
 * Create a standard missing capability error.
 */
export function createMissingCapabilityError(
  capability: string
): MissingCapabilityError {
  return new MissingCapabilityError(capability)
}

/**
 * Create a standard missing plugin store error.
 */
export function createMissingPluginStoreError(): MissingPluginStoreError {
  return new MissingPluginStoreError()
}
