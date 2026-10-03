/**
 * Stdio output formatter.
 *
 * Renders CommandResult values as human-readable plain text.
 *
 * Strategy:
 *   1. Extract `value` from CommandResult (json/text/table/open/multi)
 *   2. If value has a `results` array of objects → render as table
 *   3. Any non-`results` keys → render as summary header
 *   4. If `results` is a simple value → render directly
 *   5. If no `results` key → render the whole value
 */

interface TableLike {
  columns: readonly { key: string; title: string }[]
  rows: readonly Record<string, unknown>[]
}

// ─── Public API ──────────────────────────────────────────────────────

/**
 * Format a CommandResult as plain text for stdio output.
 */
export function formatStdio(result: unknown): string {
  if (!result || typeof result !== 'object') return formatPrimitive(result)

  const res = result as Record<string, unknown>
  const value = extractValue(res)

  if (value === null || value === undefined) return ''
  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean')
    return String(value)

  if (Array.isArray(value)) {
    return renderArray(value)
  }

  if (typeof value === 'object') {
    return renderObject(value as Record<string, unknown>)
  }

  return formatPrimitive(value)
}

// ─── Value extraction ────────────────────────────────────────────────

function extractValue(result: Record<string, unknown>): unknown {
  if (typeof result.stdio === 'string') return result.stdio

  const type = result.type

  switch (type) {
    case 'text':
      return result.text ?? ''
    case 'json':
      return result.value ?? null
    case 'table':
      return result.columns && result.rows
        ? { columns: result.columns, rows: result.rows }
        : null
    case 'open':
      return result.target ?? result.path ?? ''
    case 'file':
      return result.path ?? ''
    case 'multi':
      return Array.isArray(result.items) ? result.items : []
    default:
      return result
  }
}

// ─── Renderers ───────────────────────────────────────────────────────

/**
 * Render a plain array (no `results` key).
 *   - array of primitives → one per line
 *   - array of objects → table
 */
function renderArray(arr: unknown[]): string {
  if (arr.length === 0) return '(empty)'

  // Nested multi results
  if (
    arr.length > 0 &&
    arr[0] &&
    typeof arr[0] === 'object' &&
    'type' in (arr[0] as Record<string, unknown>)
  ) {
    return arr
      .map(item => formatStdio(item))
      .filter(Boolean)
      .join('\n')
  }

  if (isObjectArray(arr)) {
    return renderTable(arr as Record<string, unknown>[])
  }

  return arr.map(String).join('\n')
}

/**
 * Render an object value.
 *   - has `results` key → split into summary header + table
 *   - is a table shape ({columns, rows}) → render as table
 *   - otherwise → JSON
 */
function renderObject(obj: Record<string, unknown>): string {
  // Table-shaped object (from result.table)
  if ('columns' in obj && 'rows' in obj && Array.isArray(obj.rows)) {
    const table = obj as unknown as TableLike
    return renderTable(table.rows as Record<string, unknown>[], table.columns)
  }

  // Has 'result' key — prioritize this new convention
  if ('result' in obj) {
    return renderDisplayValue(obj.result)
  }

  // Has `results` key — split into summary + table
  if ('results' in obj) {
    const { results, ...meta } = obj
    const parts: string[] = []

    // Summary header from non-results keys
    const summary = renderSummary(meta)
    if (summary) parts.push(summary)

    // Table from results
    if (Array.isArray(results)) {
      if (results.length > 0 && isObjectArray(results)) {
        parts.push(renderTable(results as Record<string, unknown>[]))
      } else if (results.length > 0) {
        parts.push(results.map(String).join('\n'))
      } else {
        parts.push('(empty results)')
      }
    } else if (results !== null && results !== undefined) {
      parts.push(formatCell(results))
    }

    return parts.join('\n')
  }

  // Plain object → JSON
  return JSON.stringify(obj, null, 2)
}

function renderDisplayValue(value: unknown): string {
  if (value === null || value === undefined) return ''

  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean')
    return String(value)

  if (Array.isArray(value)) {
    if (value.length === 0) return '(empty)'
    if (isObjectArray(value)) {
      return renderTable(value as Record<string, unknown>[])
    }
    return value.map(String).join('\n')
  }

  if (typeof value === 'object') {
    const obj = value as Record<string, unknown>
    const keys = Object.keys(obj)
    if (keys.length === 0) return '{}'
    return keys.map(k => `${k}: ${formatCell(obj[k])}`).join('\n')
  }

  return formatPrimitive(value)
}

// ─── Table renderer ──────────────────────────────────────────────────

function renderTable(
  rows: Record<string, unknown>[],
  columnDefs?: readonly { key: string; title: string }[]
): string {
  if (rows.length === 0) return '(empty)'

  // Derive columns from first row if not provided
  const columns =
    columnDefs ?? Object.keys(rows[0]).map(key => ({ key, title: key }))

  if (columns.length === 0) return '(empty)'

  // Compute column widths
  const widths = new Map<string, number>()
  for (const col of columns) {
    widths.set(col.key, col.title.length)
  }
  for (const row of rows) {
    for (const col of columns) {
      const val = formatCell(row[col.key])
      const w = widths.get(col.key) ?? 0
      widths.set(col.key, Math.max(w, val.length))
    }
  }

  const sep = columns
    .map(col => '─'.repeat(widths.get(col.key) ?? 0))
    .join('─┼─')
  const header = columns
    .map(col => padR(col.title, widths.get(col.key) ?? 0))
    .join(' │ ')

  const body = rows
    .map(row =>
      columns
        .map(col => padR(formatCell(row[col.key]), widths.get(col.key) ?? 0))
        .join(' │ ')
    )
    .join('\n')

  return `${header}\n${sep}\n${body}`
}

// ─── Summary renderer ────────────────────────────────────────────────

function renderSummary(meta: Record<string, unknown>): string {
  const keys = Object.keys(meta)
  if (keys.length === 0) return ''

  return keys.map(k => `${k}: ${formatCell(meta[k])}`).join(', ')
}

// ─── Helpers ─────────────────────────────────────────────────────────

function isObjectArray(arr: unknown[]): boolean {
  return (
    arr.length > 0 &&
    arr.every(
      item => item !== null && typeof item === 'object' && !Array.isArray(item)
    )
  )
}

function formatCell(value: unknown): string {
  if (value === null || value === undefined) return ''
  if (typeof value === 'object') return JSON.stringify(value)
  return formatPrimitive(value)
}

function padR(s: string, width: number): string {
  return s.length >= width ? s : s + ' '.repeat(width - s.length)
}

function formatPrimitive(value: unknown): string {
  if (value === null || value === undefined) return ''

  switch (typeof value) {
    case 'string':
      return value
    case 'number':
    case 'boolean':
    case 'bigint':
      return value.toString()
    case 'symbol':
      return value.description ? `Symbol(${value.description})` : 'Symbol()'
    default:
      return ''
  }
}
