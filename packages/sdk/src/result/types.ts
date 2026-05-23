/**
 * JSON primitive value type.
 */
export type JsonPrimitive = string | number | boolean | null | undefined | any

/**
 * Recursive JSON value type.
 */
export type JsonValue =
  | JsonPrimitive
  | { [key: string]: JsonValue }
  | readonly JsonValue[]

/**
 * Text command result.
 */
export interface TextCommandResult {
  /**
   * Result discriminator.
   */
  type: 'text'
  /**
   * Text payload.
   */
  text: string
  /**
   * Optional plain text override for stdio output.
   * When present, stdio format outputs this instead of the default rendering.
   */
  stdio?: string
}

/**
 * Structured JSON payload for CLI results.
 *
 * Convention: every `result.json()` call MUST include a `result` property
 * containing the primary display content. The CLI text formatter extracts
 * this key for clean output:
 *   - primitive → printed directly
 *   - array    → each item on a separate line
 *   - object   → each key-value pair as `key: value` per line
 *
 * Additional metadata keys (e.g. `count`, `algorithm`) are preserved in
 * JSON output but ignored in text/stdio mode.
 */
export interface JsonResultPayload {
  /**
   * Primary display content shown in CLI text mode.
   */
  result: JsonValue
  /**
   * Additional metadata (preserved in JSON output, ignored in text mode).
   */
  [key: string]: JsonValue
}

/**
 * JSON command result.
 */
export interface JsonCommandResult {
  /**
   * Result discriminator.
   */
  type: 'json'
  /**
   * JSON payload. Must contain a `result` key for CLI display.
   */
  value: JsonResultPayload
  /**
   * Optional plain text override for stdio output.
   * When present, stdio format outputs this instead of JSON.stringify.
   */
  stdio?: string
}

/**
 * Table column definition.
 */
export interface TableColumn {
  /**
   * Stable column key.
   */
  key: string
  /**
   * Human readable column title.
   */
  title: string
}

/**
 * Table command result.
 */
export interface TableCommandResult {
  /**
   * Result discriminator.
   */
  type: 'table'
  /**
   * Table columns.
   */
  columns: readonly TableColumn[]
  /**
   * Table rows.
   */
  rows: readonly Record<string, unknown>[]
}

/**
 * File command result.
 */
export interface FileCommandResult {
  /**
   * Result discriminator.
   */
  type: 'file'
  /**
   * File path provided by runtime or plugin.
   */
  path: string
  /**
   * Optional display name.
   */
  name?: string
}

/**
 * Open command result.
 */
export interface OpenCommandResult {
  /**
   * Result discriminator.
   */
  type: 'open'
  /**
   * URL or local path to open.
   */
  target: string
}

/**
 * Multiple combined results.
 */
export interface MultiCommandResult {
  /**
   * Result discriminator.
   */
  type: 'multi'
  /**
   * Combined command results.
   */
  items: readonly CommandResult[]
}

/**
 * Unified command result type.
 */
export type CommandResult =
  | TextCommandResult
  | JsonCommandResult
  | TableCommandResult
  | FileCommandResult
  | OpenCommandResult
  | MultiCommandResult
