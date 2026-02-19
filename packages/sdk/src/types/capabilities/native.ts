/**
 * Host-native capability contract.
 */
export interface NativeCapability {
  /**
   * Invoke a host-defined native command.
   */
  invoke: <Out = unknown>(
    command: string,
    payload?: Record<string, unknown>
  ) => Promise<Out>
}
