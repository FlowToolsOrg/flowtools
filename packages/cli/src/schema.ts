/**
 * Zod schema → Commander flags converter.
 * Uses Zod 4's z.toJSONSchema() for introspection.
 */

import type { Command } from 'commander'

import { z } from 'zod'

/**
 * Convert camelCase or snake_case to kebab-case.
 */
export function toKebab(str: string): string {
  return str
    .replace(/([a-z])([A-Z])/g, '$1-$2')
    .replace(/_/g, '-')
    .toLowerCase()
}

export interface FieldMeta {
  type: 'string' | 'number' | 'boolean' | 'enum' | 'array'
  description?: string
  required: boolean
  default?: unknown
  enum?: string[]
  itemType?: 'string' | 'number'
}

/**
 * Convert a Zod schema to a JSON Schema and extract field metadata.
 */
export function introspectSchema(
  schema: z.ZodObject<any>
): Record<string, FieldMeta> {
  const jsonSchema = z.toJSONSchema(schema, { target: 'draft-7' })
  const properties = (jsonSchema as any).properties ?? {}
  const required = new Set((jsonSchema as any).required ?? [])
  const fields: Record<string, FieldMeta> = {}

  for (const [key, prop] of Object.entries(properties)) {
    const p = prop as Record<string, unknown>
    const field: FieldMeta = {
      type: 'string',
      required: required.has(key),
    }

    // Extract description
    if (typeof p.description === 'string') {
      field.description = p.description
    }

    // Extract default
    if ('default' in p) {
      field.default = p.default
      field.required = false
    }

    // Determine type
    if (p.type === 'boolean') {
      field.type = 'boolean'
    } else if (p.type === 'integer' || p.type === 'number') {
      field.type = 'number'
    } else if (p.enum) {
      field.type = 'enum'
      field.enum = p.enum as string[]
    } else if (p.type === 'array') {
      field.type = 'array'
      const items = p.items as Record<string, unknown> | undefined
      if (items?.type === 'number' || items?.type === 'integer') {
        field.itemType = 'number'
      } else {
        field.itemType = 'string'
      }
    } else {
      field.type = 'string'
    }

    fields[key] = field
  }

  return fields
}

/**
 * Add CLI options to a Commander command from a Zod input schema.
 */
export function addSchemaFlags(
  command: Command,
  schema: z.ZodObject<any>
): void {
  const fields = introspectSchema(schema)

  for (const [key, def] of Object.entries(fields)) {
    const flag = `--${toKebab(key)}`
    const desc = def.description ?? key

    switch (def.type) {
      case 'boolean':
        command.option(flag, desc, def.default ? true : false)
        break
      case 'number':
        command.option(`${flag} <number>`, desc, String(def.default))
        break
      case 'enum': {
        const choices = def.enum ?? []
        command.option(
          `${flag} <value>`,
          `${desc} (choices: ${choices.join(', ')})`,
          def.default != null ? String(def.default) : undefined
        )
        break
      }
      case 'array':
        command.option(
          `${flag} <value>`,
          `${desc} (repeatable)`,
          (val: string, prev: string[]) => [...prev, val],
          []
        )
        break
      default:
        command.option(
          `${flag} <value>`,
          desc,
          def.default != null ? String(def.default) : undefined
        )
        break
    }
  }
}

/**
 * Build input object from Commander options using a Zod schema.
 * Maps kebab-case CLI flags back to camelCase schema keys.
 * Coerces types based on JSON Schema metadata.
 */
export function buildInputFromOptions(
  opts: Record<string, unknown>,
  schema: z.ZodObject<any>
): Record<string, unknown> {
  const fields = introspectSchema(schema)
  const input: Record<string, unknown> = {}

  for (const [key, def] of Object.entries(fields)) {
    const kebabKey = toKebab(key)
    const value = opts[kebabKey]

    if (value !== undefined) {
      // Coerce types
      switch (def.type) {
        case 'number':
          input[key] = Number(value)
          break
        case 'boolean':
          input[key] = value === 'true' || value === true
          break
        case 'array': {
          // value may already be an array (from Commander collect),
          // or a single string from parseUnknownArgs
          const arr = Array.isArray(value) ? value : [value]
          if (def.itemType === 'number') {
            input[key] = arr.map(Number)
          } else {
            input[key] = arr
          }
          break
        }
        default:
          input[key] = value
          break
      }
    } else if (def.default !== undefined) {
      input[key] = def.default
    }
  }

  return input
}

/**
 * Generate a mock input object from a Zod schema using json-schema-faker.
 * Converts Zod → JSON Schema → mock data. Handles complex types
 * (email, url, uuid, etc.) automatically via JSON Schema format support.
 */
export async function generateMockFromSchema(
  schema: z.ZodObject<any>
): Promise<Record<string, unknown>> {
  const { generate } = await import('json-schema-faker')

  const jsonSchema = z.toJSONSchema(schema, { target: 'draft-7' }) as Record<
    string,
    unknown
  >

  // Force all properties as required so json-schema-faker generates values
  // for optional fields too
  const props = (jsonSchema as any).properties ?? {}
  jsonSchema.required = Object.keys(props)

  // Keep examples concise: 2 items per array
  return (await generate(jsonSchema, {
    minItems: 2,
    maxItems: 2,
  })) as Record<string, unknown>
}

/**
 * Build CLI flag string from fields and mock values.
 */
export function buildFlagExample(
  fields: Record<string, FieldMeta>,
  mock: Record<string, unknown>
): string {
  const parts: string[] = []
  for (const [key, field] of Object.entries(fields)) {
    const flag = `--${toKebab(key)}`
    const value = mock[key]
    if (field.type === 'boolean') {
      if (value) parts.push(flag)
    } else if (field.type === 'array' && Array.isArray(value)) {
      for (const item of value) {
        parts.push(`${flag} ${JSON.stringify(item)}`)
      }
    } else {
      parts.push(`${flag} ${JSON.stringify(value)}`)
    }
  }
  return parts.join(' ')
}

/**
 * Parse a JSON input string and validate against a schema.
 */
export function parseJsonInput(
  jsonStr: string,
  schema?: z.ZodObject<any>
): Record<string, unknown> {
  let parsed: unknown
  try {
    parsed = JSON.parse(jsonStr)
  } catch (err) {
    process.exit(1)
  }

  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
    return { value: parsed }
  }

  if (schema) {
    const result = schema.safeParse(parsed)
    if (!result.success) {
      for (const issue of result.error.issues) {
      }
      process.exit(1)
    }
    return result.data as Record<string, unknown>
  }

  return parsed as Record<string, unknown>
}
