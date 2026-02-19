/**
 * Clipboard capability contract.
 */
export interface ClipboardCapability {
  /**
   * Read text from the system clipboard.
   */
  readText: () => Promise<string>
  /**
   * Write text into the system clipboard.
   */
  writeText: (value: string) => Promise<void>
}
