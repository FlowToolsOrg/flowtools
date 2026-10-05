import { isJsonValue, type JsonValue } from '../manifest/json-schema'

export interface DataSnapshot {
  key: string
  revision: number
  value: unknown
}

export interface DataMutation {
  key: string
  expectedRevision: number
  value: JsonValue
}

/** Bound by the Host to one plugin; no namespace/path/SQL argument. */
export interface DataCapability {
  read(key: string): Promise<DataSnapshot>
  write(mutation: DataMutation): Promise<DataSnapshot>
  transaction(mutations: DataMutation[]): Promise<DataSnapshot[]>
  watch(key: string, signal: AbortSignal): AsyncIterable<DataSnapshot>
}

export type LegacyDataSource =
  | 'cli-v0'
  | 'desktop-localstorage-v1'
  | 'web-localstorage-v1'

/** Inspection only: callers retain the original, and explicitly import later. */
export async function inspectLegacyTodos(
  raw: string,
  source: LegacyDataSource
) {
  if (new TextEncoder().encode(raw).length > 65_536)
    throw new Error('BUDGET_EXCEEDED')
  const parsed: unknown = JSON.parse(raw)
  const items = Array.isArray(parsed)
    ? parsed
    : parsed && typeof parsed === 'object' && 'todos' in parsed
      ? parsed.todos
      : null
  if (!Array.isArray(items) || items.length > 1000)
    throw new Error('INPUT_INVALID')
  const value: JsonValue[] = items.map(item => {
    if (!item || typeof item !== 'object' || !isJsonValue(item))
      throw new Error('INPUT_INVALID')
    const record = item as Record<string, JsonValue>
    if (
      typeof record.todo !== 'string' ||
      (record.deadline !== undefined && typeof record.deadline !== 'string')
    )
      throw new Error('INPUT_INVALID')
    return { ...record, deadline: record.deadline ?? '' }
  })
  const canonical = (entry: JsonValue): JsonValue =>
    Array.isArray(entry)
      ? entry.map(canonical)
      : entry && typeof entry === 'object'
        ? Object.fromEntries(
            Object.entries(entry)
              .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
              .map(([key, item]) => [key, canonical(item)])
          )
        : entry
  const bytes = new TextEncoder().encode(JSON.stringify(canonical(value)))
  if (bytes.length > 65_536) throw new Error('BUDGET_EXCEEDED')
  const sourceDigest = [
    ...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)),
  ]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
  return { source, sourceDigest, value, count: value.length }
}

/** Explicit asynchronous hydration; the caller exposes conflicts/errors in UI. */
export async function hydratePluginData(
  data: DataCapability,
  key: string,
  apply: (snapshot: DataSnapshot) => void,
  signal: AbortSignal
): Promise<void> {
  const snapshot = await data.read(key)
  if (signal.aborted) return
  apply(snapshot)
  let revision = snapshot.revision
  for await (const update of data.watch(key, signal)) {
    if (signal.aborted) return
    if (update.revision > revision) {
      revision = update.revision
      apply(update)
    }
  }
}
