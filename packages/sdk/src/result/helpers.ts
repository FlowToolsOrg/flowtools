import type {
  CommandResult,
  JsonCommandResult,
  JsonValue,
  MultiCommandResult,
  OpenCommandResult,
  TableColumn,
  TableCommandResult,
  TextCommandResult,
} from './types'

/**
 * Helper collection for building structured command results.
 */
export const result = {
  /**
   * Build text result.
   */
  text(text: string): TextCommandResult {
    return {
      type: 'text',
      text,
    }
  },

  /**
   * Build JSON result.
   */
  json(value: JsonValue): JsonCommandResult {
    return {
      type: 'json',
      value,
    }
  },

  /**
   * Build table result.
   */
  table(
    columns: readonly TableColumn[],
    rows: readonly Record<string, unknown>[]
  ): TableCommandResult {
    return {
      type: 'table',
      columns,
      rows,
    }
  },

  /**
   * Build open-target result.
   */
  open(target: string): OpenCommandResult {
    return {
      type: 'open',
      target,
    }
  },

  /**
   * Build multi result.
   */
  multi(items: readonly CommandResult[]): MultiCommandResult {
    return {
      type: 'multi',
      items,
    }
  },
}
