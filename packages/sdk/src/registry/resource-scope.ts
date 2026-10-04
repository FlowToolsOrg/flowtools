export type PluginResourceOwner = 'load' | 'activation' | 'view' | 'runner'
export type PluginResourceCleanup = () => void | Promise<void>

/** Cooperative T1 ownership; this cannot forcibly stop hostile code. */
export class PluginResourceScope {
  private cleanups: PluginResourceCleanup[] = []
  private closed = false

  add(cleanup: PluginResourceCleanup): void {
    if (this.closed) throw new Error('Plugin resource scope is closed')
    this.cleanups.push(cleanup)
  }

  get size(): number {
    return this.cleanups.length
  }

  async close(): Promise<Error[]> {
    this.closed = true
    const errors: Error[] = []
    const failed: PluginResourceCleanup[] = []
    for (const cleanup of this.cleanups.splice(0).reverse()) {
      try {
        await cleanup()
      } catch (error) {
        errors.push(error instanceof Error ? error : new Error(String(error)))
        failed.unshift(cleanup)
      }
    }
    this.cleanups = failed
    return errors
  }
}
