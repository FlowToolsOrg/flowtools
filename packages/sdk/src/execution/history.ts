import type { PluginExecutionResult } from './executor'

import { z } from 'zod'

const recordSchema = z
  .object({
    id: z.string().max(256),
    pluginId: z
      .string()
      .regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/)
      .max(256),
    pluginName: z.string().max(256),
    pluginVersion: z.string().max(64).nullable(),
    startedAt: z.number().nonnegative().finite(),
    finishedAt: z.number().nonnegative().finite(),
    durationMs: z.number().nonnegative().finite(),
    inputSummary: z.object({
      kind: z.string().max(32),
      size: z.number().int().nonnegative(),
    }),
    status: z.enum(['success', 'error', 'cancelled']),
    resultType: z.string().max(32).optional(),
    errorCode: z
      .enum([
        'INPUT_INVALID',
        'NOT_RUNNABLE',
        'PLUGIN_ID_MISMATCH',
        'PLUGIN_NOT_FOUND',
        'LOAD_FAILED',
        'CONTEXT_FAILED',
        'EXECUTION_FAILED',
        'OUTPUT_INVALID',
        'ABORTED',
        'TIMEOUT',
        'TIMEOUT_INVALID',
      ])
      .optional(),
  })
  .refine(
    record =>
      record.finishedAt >= record.startedAt &&
      record.durationMs === record.finishedAt - record.startedAt &&
      (record.status === 'success'
        ? !record.errorCode && Boolean(record.resultType)
        : !record.resultType &&
          (record.status === 'cancelled'
            ? record.errorCode === 'ABORTED'
            : Boolean(record.errorCode) && record.errorCode !== 'ABORTED'))
  )

export type ExecutionHistoryRecord = z.infer<typeof recordSchema>
export interface ExecutionHistorySnapshot {
  entries: readonly ExecutionHistoryRecord[]
  persistenceError: string | null
}
export interface ExecutionHistoryStorage {
  getItem: (key: string) => string | null
  setItem: (key: string, value: string) => void
}
export interface ExecutionHistory {
  getSnapshot: () => ExecutionHistorySnapshot
  subscribe: (listener: () => void) => () => void
  record: (result: PluginExecutionResult, pluginName: string) => void
  clear: (pluginId?: string) => void
}

function resultType(data: unknown): string {
  try {
    if (
      data &&
      typeof data === 'object' &&
      'type' in data &&
      ['text', 'json', 'table', 'file', 'open', 'multi'].includes(
        String(data.type)
      )
    ) {
      return String(data.type)
    }
  } catch {
    return 'unknown'
  }
  return data === null ? 'null' : typeof data
}

/** Metadata only. Never persist actual inputs, output values or error messages. */
export function createExecutionHistory(
  storage: ExecutionHistoryStorage | undefined,
  key: string,
  limit = 200
): ExecutionHistory {
  if (!Number.isInteger(limit) || limit < 1 || limit > 200)
    throw new Error('Invalid history limit')
  let snapshot: ExecutionHistorySnapshot = {
    entries: [],
    persistenceError: null,
  }
  try {
    const raw = storage?.getItem(key)
    if (raw) {
      const parsed = z
        .object({ formatVersion: z.literal(1), entries: z.array(recordSchema) })
        .safeParse(JSON.parse(raw))
      snapshot = parsed.success
        ? {
            entries: parsed.data.entries.slice(0, limit),
            persistenceError: null,
          }
        : {
            entries: [],
            persistenceError: 'Stored history could not be verified',
          }
    }
  } catch {
    snapshot = {
      entries: [],
      persistenceError: 'Stored history could not be read',
    }
  }
  const listeners = new Set<() => void>()
  const update = (entries: readonly ExecutionHistoryRecord[]) => {
    let persistenceError: string | null = null
    try {
      storage?.setItem(key, JSON.stringify({ formatVersion: 1, entries }))
    } catch {
      persistenceError = 'History could not be saved; current session only'
    }
    snapshot = { entries, persistenceError }
    for (const listener of listeners) listener()
  }
  return {
    getSnapshot: () => snapshot,
    subscribe: listener => {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    record: (result, pluginName) => {
      const record = recordSchema.safeParse({
        id: crypto.randomUUID(),
        pluginId: result.pluginId,
        pluginName,
        pluginVersion: result.pluginVersion,
        startedAt: result.startedAt,
        finishedAt: result.finishedAt,
        durationMs: result.durationMs,
        inputSummary: result.inputSummary,
        status: result.success
          ? 'success'
          : result.error.code === 'ABORTED'
            ? 'cancelled'
            : 'error',
        resultType: result.success ? resultType(result.data) : undefined,
        errorCode: result.success ? undefined : result.error.code,
      })
      if (!record.success) {
        snapshot = {
          ...snapshot,
          persistenceError: 'Execution metadata could not be recorded',
        }
        for (const listener of listeners) listener()
        return
      }
      update([record.data, ...snapshot.entries].slice(0, limit))
    },
    clear: pluginId =>
      update(
        pluginId
          ? snapshot.entries.filter(entry => entry.pluginId !== pluginId)
          : []
      ),
  }
}

/** Host-specific key must be new; leave unverified legacy records untouched. */
export function createBrowserExecutionHistory(key: string): ExecutionHistory {
  const storage: ExecutionHistoryStorage = {
    getItem: name =>
      typeof localStorage === 'undefined' ? null : localStorage.getItem(name),
    setItem: (name, value) => {
      if (typeof localStorage === 'undefined')
        throw new Error('Storage unavailable')
      localStorage.setItem(name, value)
    },
  }
  return createExecutionHistory(storage, key)
}
