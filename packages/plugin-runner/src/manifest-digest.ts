import { createHash } from 'node:crypto'

export function manifestDigest(manifest: unknown): string {
  const canonical = (value: unknown): unknown => {
    if (Array.isArray(value)) return value.map(canonical)
    if (value && typeof value === 'object')
      return Object.fromEntries(
        Object.entries(value)
          .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
          .map(([key, item]) => [key, canonical(item)])
      )
    return value
  }
  return createHash('sha256')
    .update(JSON.stringify(canonical(manifest)))
    .digest('hex')
}
