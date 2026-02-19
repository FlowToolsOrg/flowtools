/**
 * Generic database row object.
 */
export type DBRow = Record<string, unknown>

/**
 * Database capability contract.
 */
export interface DBCapability {
  /**
   * Execute a read query and return typed rows.
   */
  query: <T extends DBRow = DBRow>(
    sql: string,
    params?: readonly unknown[]
  ) => Promise<readonly T[]>
  /**
   * Insert one row into a table and return created row count.
   */
  insert: (table: string, data: DBRow) => Promise<number>
  /**
   * Update rows by a where condition and return affected count.
   */
  update: (table: string, data: DBRow, where: DBRow) => Promise<number>
  /**
   * Delete rows by a where condition and return affected count.
   */
  delete: (table: string, where: DBRow) => Promise<number>
}
