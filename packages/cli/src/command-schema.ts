import type {
  CommandManifestV1,
  JsonValue,
  OperationSchema,
} from '@flowtools/sdk/manifest'

import { isJsonValue, validateOperationValue } from '@flowtools/sdk/manifest'

import { CLIInputError, toKebab } from './schema'

const reserved = new Set([
  'format',
  'input',
  'batch-input',
  'timeout',
  'command',
  'help',
  'profile',
])
const invalid = () =>
  new CLIInputError('SCHEMA_VALIDATION', 'Input does not match command schema')

export interface CommandFlag {
  property: string
  flag: string
  type: string
  required: boolean
  description?: string
  default?: JsonValue
  enum?: JsonValue[]
  schema: OperationSchema
}

/** CLI presentation and parsing consume the same serialized schema, never Zod code. */
export function commandFlags(command: CommandManifestV1): CommandFlag[] {
  const flags = new Set<string>()
  const names = new Set(
    Object.keys(command.inputSchema.properties ?? {}).map(toKebab)
  )
  return Object.entries(command.inputSchema.properties ?? {}).map(
    ([property, schema]) => {
      const flag = toKebab(property)
      if (
        !/^[a-z][a-z0-9-]*$/.test(flag) ||
        reserved.has(flag) ||
        flags.has(flag) ||
        (schema.type === 'boolean' && names.has(`no-${flag}`))
      )
        throw new Error('Command schema has an ambiguous CLI flag')
      flags.add(flag)
      return {
        property,
        flag: `--${flag}`,
        schema,
        type: schema.enum
          ? 'enum'
          : schema.type === 'integer'
            ? 'number'
            : (schema.type ?? 'json'),
        required:
          (command.inputSchema.required ?? []).includes(property) &&
          schema.default === undefined,
        ...(schema.description !== undefined
          ? { description: schema.description }
          : {}),
        ...(schema.default !== undefined ? { default: schema.default } : {}),
        ...(schema.enum ? { enum: schema.enum } : {}),
      }
    }
  )
}

function valueFromFlag(schema: OperationSchema, value: string): JsonValue {
  if (
    schema.type === 'string' ||
    schema.enum?.every(item => typeof item === 'string')
  )
    return value
  if (schema.type === 'number' || schema.type === 'integer') {
    if (!/^-?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/i.test(value))
      throw invalid()
    const number = Number(value)
    if (!Number.isFinite(number)) throw invalid()
    return number
  }
  if (schema.type === 'boolean') {
    if (value !== 'true' && value !== 'false') throw invalid()
    return value === 'true'
  }
  try {
    const parsed: unknown = JSON.parse(value)
    if (!isJsonValue(parsed)) throw invalid()
    return parsed
  } catch {
    throw invalid()
  }
}

export function validateCommandInput(
  command: CommandManifestV1,
  input: unknown
): Record<string, JsonValue> {
  if (
    !isJsonValue(input) ||
    new TextEncoder().encode(JSON.stringify(input)).length >
      command.resources.maxInputBytes
  )
    throw invalid()
  const parsed = validateOperationValue(command.inputSchema, input, true)
  if (
    !parsed.success ||
    !parsed.data ||
    typeof parsed.data !== 'object' ||
    Array.isArray(parsed.data) ||
    new TextEncoder().encode(JSON.stringify(parsed.data)).length >
      command.resources.maxInputBytes
  )
    throw invalid()
  return parsed.data
}

/** Reject unknown/duplicate scalar flags; only declared arrays can repeat. */
export function parseCommandFlags(
  command: CommandManifestV1,
  args: string[]
): Record<string, JsonValue> {
  const fields = new Map(
    commandFlags(command).map(field => [field.flag, field])
  )
  const input: Record<string, JsonValue> = {}
  for (let index = 0; index < args.length; index++) {
    const token = args[index]!
    const equal = token.indexOf('=')
    const name = equal === -1 ? token : token.slice(0, equal)
    let field = fields.get(name)
    let negate = false
    if (!field && name.startsWith('--no-')) {
      field = fields.get('--' + name.slice(5))
      negate = true
    }
    if (!field || (negate && field.schema.type !== 'boolean')) throw invalid()
    if (
      Object.prototype.hasOwnProperty.call(input, field.property) &&
      field.schema.type !== 'array'
    )
      throw invalid()
    let raw = equal === -1 ? undefined : token.slice(equal + 1)
    if (negate) {
      if (raw !== undefined) throw invalid()
      raw = 'false'
    } else if (field.schema.type === 'boolean' && raw === undefined) {
      if (['true', 'false'].includes(args[index + 1] ?? '')) raw = args[++index]
      else raw = 'true'
    } else if (raw === undefined) {
      raw = args[++index]
      if (raw === undefined || raw.startsWith('--')) throw invalid()
    }
    if (field.schema.type === 'array') {
      const value = raw!.startsWith('[')
        ? valueFromFlag(field.schema, raw!)
        : [valueFromFlag(field.schema.items!, raw!)]
      if (!Array.isArray(value)) throw invalid()
      input[field.property] = [
        ...((input[field.property] as JsonValue[]) ?? []),
        ...value,
      ]
    } else input[field.property] = valueFromFlag(field.schema, raw!)
  }
  return validateCommandInput(command, input)
}

function exampleValue(schema: OperationSchema): JsonValue {
  if (schema.default !== undefined) return structuredClone(schema.default)
  if (schema.const !== undefined) return structuredClone(schema.const)
  if (schema.enum) return structuredClone(schema.enum[0]!)
  if (schema.anyOf) return exampleValue(schema.anyOf[0]!)
  switch (schema.type) {
    case 'object': {
      const result: Record<string, JsonValue> = {}
      for (const [key, child] of Object.entries(schema.properties ?? {}))
        if (schema.required?.includes(key) || child.default !== undefined)
          result[key] = exampleValue(child)
      return result
    }
    case 'array':
      if ((schema.minItems ?? 0) > 100) throw invalid()
      return Array.from({ length: schema.minItems ?? 0 }, () =>
        exampleValue(schema.items!)
      )
    case 'string':
      if ((schema.minLength ?? 0) > 4096) throw invalid()
      return 'example'
        .padEnd(schema.minLength ?? 0, 'x')
        .slice(0, schema.maxLength)
    case 'number':
    case 'integer': {
      let value =
        schema.minimum ??
        (schema.exclusiveMinimum !== undefined
          ? schema.exclusiveMinimum + 1
          : 0)
      if (schema.maximum !== undefined) value = Math.min(value, schema.maximum)
      if (schema.exclusiveMaximum !== undefined)
        value = Math.min(value, schema.exclusiveMaximum - 1)
      return schema.type === 'integer' ? Math.ceil(value) : value
    }
    case 'boolean':
      return false
    default:
      return null
  }
}

/** Schema-valid sample only; declared runtime validators remain authoritative. */
export function commandExample(command: CommandManifestV1) {
  try {
    return validateCommandInput(command, exampleValue(command.inputSchema))
  } catch {
    return null
  }
}

export function commandFlagHelp(command: CommandManifestV1): string {
  return commandFlags(command)
    .map(field => {
      const value =
        field.schema.type === 'boolean' ? ' [true|false]' : ` <${field.type}>`
      return (
        `  ${field.flag}${value}${field.required ? ' [required]' : ''}` +
        (field.default !== undefined
          ? ` [default: ${JSON.stringify(field.default)}]`
          : '') +
        (field.enum ? ` [choices: ${field.enum.map(String).join(', ')}]` : '') +
        (field.schema.type === 'array' ? ' [repeatable or JSON array]' : '') +
        `\n    ${field.description ?? field.property}`
      )
    })
    .join('\n')
}
