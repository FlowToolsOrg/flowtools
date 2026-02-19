/**
 * JSON primitive value type.
 */
export type JsonPrimitive = string | number | boolean | null

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
   * JSON payload.
   */
  value: JsonValue
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
