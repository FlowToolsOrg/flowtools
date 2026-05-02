/**
 * Output formatter — converts CommandResult to CLI-friendly output.
 */

import type { OutputFormat } from './types'

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
 */
export function formatResult(
  result: CommandResultShape,
  format: OutputFormat
): string {
  if (format === 'json') {
    return JSON.stringify(result, null, 2)
  }

  return formatResultText(result)
}

function formatResultText(result: CommandResultShape): string {
  switch (result.type) {
    case 'text':
      return result.text ?? ''

    case 'json':
      return JSON.stringify(result.value, null, 2)

    case 'table': {
      if (!result.columns || !result.rows) return '(empty table)'
      return formatTable(result.columns, result.rows)
    }

    case 'open':
      return `Open: ${result.target ?? result.path ?? '(unknown)'}`

    case 'file':
      return `File: ${result.path ?? '(unknown)'}${result.name ? ` (${result.name})` : ''}`

    case 'multi': {
      if (!result.items) return '(empty)'
      return result.items.map(item => formatResultText(item)).join('\n---\n')
    }

    default:
      return JSON.stringify(result, null, 2)
  }
}

function formatTable(
  columns: readonly { key: string; title: string }[],
  rows: readonly Record<string, unknown>[]
): string {
  if (rows.length === 0) return '(empty table)'

  // Calculate column widths
  const widths = new Map<string, number>()
  for (const col of columns) {
    widths.set(col.key, col.title.length)
  }
  for (const row of rows) {
    for (const col of columns) {
      const val = String(row[col.key] ?? '')
      const current = widths.get(col.key) ?? 0
      widths.set(col.key, Math.max(current, val.length))
    }
  }

  // Header
  const header = columns
    .map(col => col.title.padEnd(widths.get(col.key) ?? 0))
    .join('  ')

  const separator = columns
    .map(col => '─'.repeat(widths.get(col.key) ?? 0))
    .join('──')

  // Rows
  const body = rows
    .map(row =>
      columns
        .map(col => String(row[col.key] ?? '').padEnd(widths.get(col.key) ?? 0))
        .join('  ')
    )
    .join('\n')

  return `${header}\n${separator}\n${body}`
}
