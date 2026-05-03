/**
 * Output formatter — top-level entry point.
 *
 * Two output formats:
 *   json  — full CommandResult as JSON (with type wrapper)
 *   stdio — human-readable plain text via format-stdio
 *
 * The format layer is intentionally thin: it delegates to format-stdio
 * for stdio rendering, so the output strategy can be changed in one place.
 */

import type { OutputFormat } from './types'

import { formatStdio } from './format-stdio'

interface CommandResultShape {
  type: string
  text?: string
  value?: unknown
  columns?: readonly { key: string; title: string }[]
  rows?: readonly Record<string, unknown>[]
  target?: string
  path?: string
  name?: string
  items?: readonly CommandResultShape[]
}

/**
 * Format a command result for CLI output.
 *
 * - `json`:  full CommandResult as JSON
 * - `stdio`: plain text via format-stdio
 */
export function formatResult(
  result: CommandResultShape,
  format: OutputFormat
): string {
  if (format === 'json') {
    return JSON.stringify(result, null, 2)
  }
  return formatStdio(result)
}

/**
 * Format a raw (non-CommandResult) value for CLI output.
 */
export function formatRaw(value: unknown, format: OutputFormat): string {
  if (format === 'json') {
    return JSON.stringify(value, null, 2)
  }
  if (value === null || value === undefined) return ''
  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean')
    return String(value)
  return formatStdio(value)
}
