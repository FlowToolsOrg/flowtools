import { expect, test } from 'bun:test'

import {
  commandManifestSchema,
  type OperationSchema,
} from '@flowtools/sdk/manifest'

import {
  commandFlags,
  commandExample,
  parseCommandFlags,
  validateCommandInput,
} from './command-schema'
import { parseRunArguments, requestedRunFormat } from './run-arguments'
import { CLIInputError } from './schema'

function command(
  properties: Record<string, OperationSchema> = {
    text: { type: 'string' },
    offset: { type: 'number', default: 0 },
    active: { type: 'boolean', default: true },
    items: { type: 'array', items: { type: 'integer' }, default: [] },
    config: {
      type: 'object',
      properties: { mode: { type: 'string' } },
      required: ['mode'],
      additionalProperties: false,
    },
  }
) {
  return commandManifestSchema.parse({
    id: 'run',
    name: 'Fixture',
    description: 'Serialized contract fixture',
    inputSchema: {
      type: 'object',
      properties,
      required: ['text'],
      additionalProperties: false,
    },
    outputSchema: { type: 'null' },
    runtimeValidation: { input: 'schema-only', output: 'schema-only' },
    headless: true,
    supportsColdStart: false,
    interaction: 'none',
    effects: [],
    permissions: [],
    resources: { timeoutMs: 30000, maxInputBytes: 1024, maxOutputBytes: 1024 },
  })
}

test('serialized CLI flags preserve negatives, false, arrays and nested JSON', () => {
  expect(
    parseCommandFlags(command(), [
      '--text',
      'hello',
      '--offset',
      '-1.25e2',
      '--active',
      'false',
      '--items',
      '-2',
      '--items=[0,3]',
      '--config',
      '{"mode":"safe"}',
    ])
  ).toEqual({
    text: 'hello',
    offset: -125,
    active: false,
    items: [-2, 0, 3],
    config: { mode: 'safe' },
  })
  expect(
    parseCommandFlags(command(), ['--text=hello', '--no-active'])
  ).toMatchObject({ active: false })
  expect(
    parseCommandFlags(command(), ['--text', 'hello', '--active=false'])
  ).toMatchObject({ active: false })
  expect(
    parseCommandFlags(command(), ['--text', 'hello', '--active'])
  ).toMatchObject({ active: true })
})

test('required/default and CLI metadata come from serialized schema', () => {
  expect(parseCommandFlags(command(), ['--text', 'hello'])).toEqual({
    text: 'hello',
    active: true,
    offset: 0,
    items: [],
  })
  expect(
    commandFlags(command()).find(field => field.property === 'text')
  ).toMatchObject({ flag: '--text', required: true })
  expect(
    commandFlags(command()).find(field => field.property === 'active')
  ).toMatchObject({ required: false, default: true })
  expect(commandExample(command())).toMatchObject({
    text: 'example',
    active: true,
    offset: 0,
  })
})

for (const args of [
  [],
  ['--unknown', 'secret'],
  ['--text'],
  ['--text', 'hello', '--text', 'duplicate'],
  ['--text', 'hello', 'extra'],
  ['--text', 'hello', '--active', 'not-a-boolean'],
  ['--text', 'hello', '--offset', 'NaN'],
  ['--text', 'hello', '--offset', 'Infinity'],
  ['--text', 'hello', '--items', '1.5'],
  ['--text', 'hello', '--config', '{'],
  ['--text', 'hello', '--no-active=true'],
]) {
  test(`schema parser refuses malformed flags ${JSON.stringify(args)}`, () => {
    expect(() => parseCommandFlags(command(), args)).toThrow(CLIInputError)
  })
}

test('JSON schema rejects unknown fields, lossy JSON, wrong shapes and budget overruns', () => {
  for (const input of [
    { text: 'hello', unknown: true },
    [],
    null,
    { text: 3 },
    { text: undefined },
    { text: 'x'.repeat(1100) },
  ])
    expect(() => validateCommandInput(command(), input)).toThrow(CLIInputError)
})

test('default expansion is checked against the input budget during preparation', () => {
  const value = command({ text: { type: 'string', default: 'x'.repeat(1100) } })
  expect(() => validateCommandInput(value, {})).toThrow(CLIInputError)
  expect(() => parseCommandFlags(value, [])).toThrow(CLIInputError)
})

test('ambiguous/reserved generated flags are rejected', () => {
  expect(() =>
    commandFlags(
      command({
        text: { type: 'string' },
        active: { type: 'boolean' },
        noActive: { type: 'boolean' },
      })
    )
  ).toThrow('ambiguous')
  expect(() =>
    commandFlags(
      command({ text: { type: 'string' }, format: { type: 'string' } })
    )
  ).toThrow('ambiguous')
  expect(() =>
    commandFlags(
      command({
        text: { type: 'string' },
        camelCase: { type: 'string' },
        camel_case: { type: 'string' },
      })
    )
  ).toThrow('ambiguous')
})

test('sample generation is bounded for otherwise valid huge schemas', () => {
  const value = command({ text: { type: 'string', minLength: 1000000 } })
  expect(commandExample(value)).toBeNull()
})

test('host argument parser keeps operation flags and selects the declared command', () => {
  expect(
    parseRunArguments([
      '--command=run',
      '--format=json',
      '--text',
      'hello',
      '--offset',
      '-2',
    ])
  ).toMatchObject({
    commandId: 'run',
    format: 'json',
    flags: ['--text', 'hello', '--offset', '-2'],
  })
  expect(parseRunArguments(['--batch-input', '[{}]']).format).toBe('json')
  expect(parseRunArguments(['--input', '{}']).format).toBe('json')
})

for (const args of [
  ['--format'],
  ['--format', 'xml'],
  ['--format', 'json', '-f', 'text'],
  ['--input', '{}', '--text', 'ignored'],
  ['--input', '{}', '--batch-input', '[{}]'],
  ['--command', '../outside'],
]) {
  test(`host parser refuses ambiguous options ${JSON.stringify(args)}`, () => {
    expect(() => parseRunArguments(args)).toThrow(CLIInputError)
  })
}

test('JSON format is retained for malformed invocations', () => {
  expect(requestedRunFormat(['--unknown', '--format=json'])).toBe('json')
  expect(requestedRunFormat(['--input', '{secret'])).toBe('json')
})
