import { describe, expect, test } from 'bun:test'

import { Command } from 'commander'
import { z } from 'zod'

import {
  addSchemaFlags,
  buildInputFromOptions,
  buildFlagExample,
  CLIInputError,
  introspectSchema,
  parseJsonInput,
  toKebab,
} from './schema'

const inputSchema = z.object({
  displayName: z.string().describe('Display name'),
  retry_count: z.number(),
  enabled: z.boolean().default(false),
  mode: z.enum(['fast', 'safe']),
  tags: z.array(z.string()),
  weights: z.array(z.number()),
  limit: z.number().default(3),
})

describe('schema flags and coercion', () => {
  test('converts camelCase and snake_case names to kebab-case', () => {
    expect(toKebab('displayName')).toBe('display-name')
    expect(toKebab('retry_count')).toBe('retry-count')
  })

  test('introspects field types, defaults, and required state', () => {
    const fields = introspectSchema(inputSchema)

    expect(fields.displayName).toMatchObject({
      type: 'string',
      description: 'Display name',
      required: true,
    })
    expect(fields.retry_count).toMatchObject({
      type: 'number',
      required: true,
    })
    expect(fields.enabled).toMatchObject({
      type: 'boolean',
      required: false,
      default: false,
    })
    expect(fields.mode).toMatchObject({
      type: 'enum',
      enum: ['fast', 'safe'],
      required: true,
    })
    expect(fields.weights).toMatchObject({
      type: 'array',
      itemType: 'number',
      required: true,
    })
  })

  test('coerces actual Commander flags to the schema shape', () => {
    const command = new Command().exitOverride()
    addSchemaFlags(command, inputSchema)

    command.parse([
      'node',
      'schema-test',
      '--display-name',
      'FlowTools',
      '--retry-count',
      '2',
      '--enabled',
      '--mode',
      'fast',
      '--tags',
      'one',
      '--tags',
      'two',
      '--weights',
      '1.5',
      '--weights',
      '2',
    ])

    expect(buildInputFromOptions(command.opts(), inputSchema)).toEqual({
      displayName: 'FlowTools',
      retry_count: 2,
      enabled: true,
      mode: 'fast',
      tags: ['one', 'two'],
      weights: [1.5, 2],
      limit: 3,
    })
  })

  test('does not synthesize an undefined number flag', () => {
    const schema = z.object({ count: z.number(), limit: z.number().default(3) })
    const command = new Command().exitOverride()
    addSchemaFlags(command, schema)
    command.parse(['node', 'schema-test'])

    expect(command.opts()).toEqual({ limit: '3' })
    expect(buildInputFromOptions(command.opts(), schema)).toEqual({ limit: 3 })
  })

  test('builds quoted and repeatable flag examples', () => {
    expect(
      buildFlagExample(
        {
          displayName: { type: 'string', required: true },
          tags: { type: 'array', required: false, itemType: 'string' },
          enabled: { type: 'boolean', required: false },
        },
        {
          displayName: 'Flow Tools',
          tags: ['one', 'two'],
          enabled: true,
        }
      )
    ).toBe('--display-name "Flow Tools" --tags "one" --tags "two" --enabled')
  })
})

describe('parseJsonInput', () => {
  test('parses object input', () => {
    expect(parseJsonInput('{"name":"FlowTools"}')).toEqual({
      name: 'FlowTools',
    })
  })

  test('returns schema-validated data', () => {
    const schema = z.object({ count: z.number().int().positive() })

    expect(parseJsonInput('{"count":2}', schema)).toEqual({ count: 2 })
  })

  test('throws a stable error for malformed JSON', () => {
    try {
      parseJsonInput('{')
      throw new Error('Expected parseJsonInput to throw')
    } catch (error) {
      expect(error).toBeInstanceOf(CLIInputError)
      expect(error).toMatchObject({
        name: 'CLIInputError',
        code: 'INVALID_JSON',
      })
      expect((error as CLIInputError).message).toStartWith(
        'Invalid JSON input:'
      )
    }
  })

  test('rejects primitive and array JSON input', () => {
    for (const input of ['42', 'null', '[]']) {
      try {
        parseJsonInput(input)
        throw new Error('Expected parseJsonInput to throw')
      } catch (error) {
        expect(error).toBeInstanceOf(CLIInputError)
        expect(error).toMatchObject({ code: 'INVALID_INPUT_SHAPE' })
      }
    }
  })

  test('throws a stable error containing schema issue paths', () => {
    const schema = z.object({ count: z.number().positive() })

    try {
      parseJsonInput('{"count":-1}', schema)
      throw new Error('Expected parseJsonInput to throw')
    } catch (error) {
      expect(error).toBeInstanceOf(CLIInputError)
      expect(error).toMatchObject({ code: 'SCHEMA_VALIDATION' })
      expect((error as CLIInputError).message).toContain('count:')
    }
  })
})
