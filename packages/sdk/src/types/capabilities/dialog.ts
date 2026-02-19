/**
 * File extension filter for dialog selections.
 */
export interface DialogFilter {
  /**
   * User visible filter label.
   */
  name: string
  /**
   * Allowed extensions without dot.
   */
  extensions: readonly string[]
}

/**
 * Options for file open dialog.
 */
export interface OpenFileDialogOptions {
  /**
   * Dialog title.
   */
  title?: string
  /**
   * Whether user can pick multiple files.
   */
  multiple?: boolean
  /**
   * Allowed extension filters.
   */
  filters?: readonly DialogFilter[]
}

/**
 * Options for file save dialog.
 */
export interface SaveFileDialogOptions {
  /**
   * Dialog title.
   */
  title?: string
  /**
   * Initial file path suggestion.
   */
  defaultPath?: string
  /**
   * Allowed extension filters.
   */
  filters?: readonly DialogFilter[]
}

/**
 * Dialog capability contract.
 */
export interface DialogCapability {
  /**
   * Show native file-open dialog.
   */
  openFile: (
    options?: OpenFileDialogOptions
  ) => Promise<readonly string[] | null>
  /**
   * Show native file-save dialog.
   */
  saveFile: (options?: SaveFileDialogOptions) => Promise<string | null>
}
