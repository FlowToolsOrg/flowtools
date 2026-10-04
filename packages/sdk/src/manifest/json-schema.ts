import { z } from 'zod'

function hasOwn(value: object, key: PropertyKey): boolean {
  return Object.prototype.hasOwnProperty.call(value, key)
}

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue }

/** Deliberately finite v1 subset. No refs, regexes, formats or executable checks. */
export interface OperationSchema {
  type?:
    | 'object'
    | 'array'
    | 'string'
    | 'number'
    | 'integer'
    | 'boolean'
    | 'null'
  description?: string
  properties?: Record<string, OperationSchema>
  required?: string[]
  additionalProperties?: boolean
  items?: OperationSchema
  enum?: JsonValue[]
  const?: JsonValue
  default?: JsonValue
  anyOf?: OperationSchema[]
  minLength?: number
  maxLength?: number
  minimum?: number
  maximum?: number
  exclusiveMinimum?: number
  exclusiveMaximum?: number
  minItems?: number
  maxItems?: number
}

/** Refuse lossy JSON serialization, prototypes, accessors, cycles and large trees. */
export function isJsonValue(value: unknown): value is JsonValue {
  const ancestors = new Set<object>()
  let nodes = 0
  const visit = (item: unknown, depth: number): boolean => {
    if (++nodes > 100_000 || depth > 32) return false
    if (item === null || typeof item === 'boolean') return true
    if (typeof item === 'string') return item.length <= 1_048_576
    if (typeof item === 'number') return Number.isFinite(item)
    if (typeof item !== 'object' || ancestors.has(item)) return false
    const array = Array.isArray(item)
    if (
      !array &&
      Object.getPrototypeOf(item) !== Object.prototype &&
      Object.getPrototypeOf(item) !== null
    )
      return false
    ancestors.add(item)
    const keys = Reflect.ownKeys(item)
    if (array && keys.length !== item.length + 1) return false
    const valid = keys.every(key => {
      if (array && key === 'length') return true
      if (
        typeof key !== 'string' ||
        ['__proto__', 'constructor', 'prototype'].includes(key)
      )
        return false
      const descriptor = Object.getOwnPropertyDescriptor(item, key)
      return Boolean(
        descriptor?.enumerable &&
        'value' in descriptor &&
        visit(descriptor.value, depth + 1)
      )
    })
    ancestors.delete(item)
    return valid
  }
  try {
    return visit(value, 0)
  } catch {
    return false
  }
}

const jsonValue = z.custom<JsonValue>(isJsonValue, 'Expected bounded JSON data')
const count = z.number().int().min(0).max(1_048_576)
const nodeSchema: z.ZodType<OperationSchema> = z.lazy(() =>
  z.strictObject({
    type: z
      .enum([
        'object',
        'array',
        'string',
        'number',
        'integer',
        'boolean',
        'null',
      ])
      .optional(),
    description: z.string().max(4096).optional(),
    properties: z.record(z.string().min(1).max(128), nodeSchema).optional(),
    required: z.array(z.string()).max(256).optional(),
    additionalProperties: z.boolean().optional(),
    items: nodeSchema.optional(),
    enum: z.array(jsonValue).min(1).max(256).optional(),
    const: jsonValue.optional(),
    default: jsonValue.optional(),
    anyOf: z.array(nodeSchema).min(1).max(16).optional(),
    minLength: count.optional(),
    maxLength: count.optional(),
    minimum: z.number().optional(),
    maximum: z.number().optional(),
    exclusiveMinimum: z.number().optional(),
    exclusiveMaximum: z.number().optional(),
    minItems: count.optional(),
    maxItems: count.optional(),
  })
)

const same = (a: JsonValue, b: JsonValue): boolean => {
  if (a === b) return true
  if (
    a === null ||
    b === null ||
    typeof a !== 'object' ||
    typeof b !== 'object'
  )
    return false
  if (Array.isArray(a) !== Array.isArray(b)) return false
  const left = Object.keys(a),
    right = Object.keys(b)
  return (
    left.length === right.length &&
    left.every(
      key =>
        hasOwn(b, key) &&
        same(
          (a as Record<string, JsonValue>)[key]!,
          (b as Record<string, JsonValue>)[key]!
        )
    )
  )
}

function matches(schema: OperationSchema, value: JsonValue): boolean {
  if (schema.anyOf && !schema.anyOf.some(branch => matches(branch, value)))
    return false
  if (schema.enum && !schema.enum.some(item => same(item, value))) return false
  if (hasOwn(schema, 'const') && !same(schema.const!, value)) return false
  switch (schema.type) {
    case 'null':
      return value === null
    case 'boolean':
      return typeof value === 'boolean'
    case 'string':
      return (
        typeof value === 'string' &&
        Array.from(value).length >= (schema.minLength ?? 0) &&
        Array.from(value).length <= (schema.maxLength ?? Infinity)
      )
    case 'number':
    case 'integer':
      return (
        typeof value === 'number' &&
        (schema.type !== 'integer' || Number.isInteger(value)) &&
        value >= (schema.minimum ?? -Infinity) &&
        value <= (schema.maximum ?? Infinity) &&
        value > (schema.exclusiveMinimum ?? -Infinity) &&
        value < (schema.exclusiveMaximum ?? Infinity)
      )
    case 'array':
      return (
        Array.isArray(value) &&
        value.length >= (schema.minItems ?? 0) &&
        value.length <= (schema.maxItems ?? Infinity) &&
        value.every(item => matches(schema.items!, item))
      )
    case 'object':
      return (
        value !== null &&
        typeof value === 'object' &&
        !Array.isArray(value) &&
        (schema.required ?? []).every(key => hasOwn(value, key)) &&
        Object.entries(value).every(([key, item]) =>
          schema.properties?.[key]
            ? matches(schema.properties[key], item)
            : schema.additionalProperties === true
        )
      )
    default:
      return true
  }
}

function wellFormed(schema: OperationSchema): boolean {
  if (!schema.type && !schema.anyOf && !schema.enum && !hasOwn(schema, 'const'))
    return false
  if (schema.type === 'object') {
    if (schema.additionalProperties === undefined) return false
    const required = schema.required ?? []
    if (
      new Set(required).size !== required.length ||
      required.some(key => !hasOwn(schema.properties ?? {}, key))
    )
      return false
  } else if (
    schema.properties ||
    schema.required ||
    schema.additionalProperties !== undefined
  )
    return false
  if (
    schema.type === 'array'
      ? !schema.items
      : schema.items ||
        schema.minItems !== undefined ||
        schema.maxItems !== undefined
  )
    return false
  if (
    schema.type !== 'string' &&
    (schema.minLength !== undefined || schema.maxLength !== undefined)
  )
    return false
  if (
    schema.type !== 'number' &&
    schema.type !== 'integer' &&
    [
      schema.minimum,
      schema.maximum,
      schema.exclusiveMinimum,
      schema.exclusiveMaximum,
    ].some(value => value !== undefined)
  )
    return false
  for (const [min, max] of [
    [schema.minLength, schema.maxLength],
    [schema.minItems, schema.maxItems],
    [schema.minimum, schema.maximum],
  ])
    if (min !== undefined && max !== undefined && min > max) return false
  if (
    schema.enum &&
    schema.enum.some((item, index) =>
      schema.enum!.slice(0, index).some(other => same(item, other))
    )
  )
    return false
  const children = [
    ...Object.values(schema.properties ?? {}),
    ...(schema.items ? [schema.items] : []),
    ...(schema.anyOf ?? []),
  ]
  if (!children.every(wellFormed)) return false
  return !hasOwn(schema, 'default') || matches(schema, schema.default!)
}

/** Check JSON before recursive Zod parsing so hostile deep/cyclic trees cannot recurse. */
export const operationSchema = z.custom<OperationSchema>(value => {
  if (!isJsonValue(value) || JSON.stringify(value).length > 65_536) return false
  const parsed = nodeSchema.safeParse(value)
  return parsed.success && wellFormed(parsed.data)
}, 'Unsupported or invalid operation JSON Schema')

function defaults(schema: OperationSchema, value: JsonValue): JsonValue {
  if (
    schema.type === 'object' &&
    value !== null &&
    typeof value === 'object' &&
    !Array.isArray(value)
  ) {
    const output = { ...value }
    for (const [key, child] of Object.entries(schema.properties ?? {})) {
      if (!hasOwn(output, key) && hasOwn(child, 'default'))
        output[key] = structuredClone(child.default!)
      if (hasOwn(output, key)) output[key] = defaults(child, output[key]!)
    }
    return output
  }
  if (schema.type === 'array' && Array.isArray(value))
    return value.map(item => defaults(schema.items!, item))
  return value
}

export function validateOperationValue(
  schema: OperationSchema,
  value: unknown,
  applyDefaults = false
) {
  operationSchema.parse(schema)
  if (!isJsonValue(value)) return { success: false as const }
  const data = applyDefaults ? defaults(schema, value) : value
  return matches(schema, data)
    ? { success: true as const, data }
    : { success: false as const }
}

/** Zod stays the runtime authority. Custom refinement requires an explicit declaration. */
export function exportOperationSchema(
  schema: z.ZodType,
  runtimeValidation: 'required' | 'schema-only' = 'schema-only'
): OperationSchema {
  const seen = new Set<object>()
  const hasCustomCheck = (value: unknown): boolean => {
    if (!value || typeof value !== 'object' || seen.has(value)) return false
    seen.add(value)
    const object = value as Record<string, unknown>
    if (object.check === 'custom' || object.type === 'transform') return true
    // Zod's own definition graph, never manifest-supplied executable values.
    const definition = (object._zod as { def?: unknown } | undefined)?.def
    return definition
      ? hasCustomCheck(definition)
      : Object.values(object).some(hasCustomCheck)
  }
  if (hasCustomCheck(schema) && runtimeValidation !== 'required')
    throw new Error('Explicit runtime validation is required')
  const exported = z.toJSONSchema(schema, {
    io: 'input',
    cycles: 'throw',
    target: 'draft-7',
  })
  const data = { ...exported }
  delete data.$schema
  const closeObjects = (value: unknown): void => {
    if (!value || typeof value !== 'object') return
    const node = value as Record<string, unknown>
    if (node.type === 'object') node.additionalProperties = false
    Object.values(node).forEach(closeObjects)
  }
  closeObjects(data)
  return operationSchema.parse(data)
}
