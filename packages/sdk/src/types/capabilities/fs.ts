/**
 * Writable file payload type.
 */
export type FileWriteData = string | Uint8Array

/**
 * Filesystem capability contract provided by host runtime.
 */
export interface FSCapability {
  /**
   * Read a UTF-8 text file from the runtime filesystem.
   */
  readFile: (path: string) => Promise<string>
  /**
   * Write text or bytes to a file path managed by runtime.
   */
  writeFile: (path: string, data: FileWriteData) => Promise<void>
}
