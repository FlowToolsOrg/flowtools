import { z } from 'zod'

/** Presentation only; execution always validates the original schema. */
export function describeInputSchema(schema?: z.ZodType): unknown {
  if (!schema) return undefined
  try {
    return z.toJSONSchema(schema)
  } catch {
    return {
      notice:
        'Schema cannot be represented as JSON; runtime validation remains enabled',
    }
  }
}
