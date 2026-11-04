import type { ExecutablePlugin } from '../src/execution/executor'
import type { ToolContext } from '../src/types/ctx'

import { expect, test } from 'bun:test'

import { z } from 'zod'

import {
  commandIdentity,
  parsePluginManifest,
  exportOperationSchema,
  operationSchema,
  validateOperationValue,
  isJsonValue,
  executeManifestCommand,
  migrateLegacyManifest,
} from '../src/manifest'

import { manifestFixture, manifestTarget } from './fixtures/manifest-v1'

const ctx: ToolContext = {
  env: {
    pluginId: 'fixture-plugin',
    pluginType: 'tool',
    platform: 'unknown',
    mode: 'test',
  },
  ui: { toast() {}, openPanel() {}, closePanel() {} },
  signal: new AbortController().signal,
  log() {},
  utils: { now: Date.now },
}

test('operation export keeps compact primitive unions in the finite v1 dialect', () => {
  const schema = z.object({
    value: z.union([z.string(), z.number()]),
    nullable: z.number().nullable(),
    bounded: z.union([z.string().min(2), z.number().min(3)]),
  })
  const exported = exportOperationSchema(schema)
  expect(exported.properties!.value).toEqual({
    anyOf: [{ type: 'string' }, { type: 'number' }],
  })
  for (const value of [
    { value: 'text', nullable: null, bounded: 'ab' },
    { value: 2, nullable: 3, bounded: 4 },
    { value: false, nullable: null, bounded: 4 },
    { value: 2, nullable: false, bounded: 4 },
    { value: 2, nullable: null, bounded: 'a' },
    { value: 2, nullable: null, bounded: 2 },
  ]) {
    expect(validateOperationValue(exported, value).success).toBe(
      schema.safeParse(value).success
    )
  }
  // Package-authored schemas still cannot add a new type-array dialect.
  expect(
    operationSchema.safeParse({ type: ['string', 'number'] }).success
  ).toBe(false)
})

test('Manifest v1 keeps UTF-16 scope and nested operation text limits after Zod upgrades', () => {
  const manifest = manifestFixture()
  manifest.commands[0]!.permissions = [
    {
      capability: 'fs',
      operations: ['read'],
      scopes: ['😀'.repeat(512)],
    },
  ]
  expect(() => parsePluginManifest(manifest)).not.toThrow()
  manifest.commands[0]!.permissions[0]!.scopes[0] += 'x'
  expect(() => parsePluginManifest(manifest)).toThrow()
  expect(
    operationSchema.safeParse({
      type: 'string',
      description: '😀'.repeat(2048),
    }).success
  ).toBe(true)
  expect(
    operationSchema.safeParse({
      type: 'string',
      description: '😀'.repeat(2048) + 'x',
    }).success
  ).toBe(false)
  for (const length of [64, 65]) {
    const key = '😀'.repeat(length)
    expect(
      operationSchema.safeParse({
        type: 'object',
        properties: { [key]: { type: 'string' } },
        additionalProperties: false,
      }).success
    ).toBe(length === 64)
  }
})

test('operation export never rewrites schema-like default data', () => {
  const payload = { type: ['string', 'number'], nested: { type: 'object' } }
  const schema = z.object({
    payload: z
      .strictObject({
        type: z.array(z.string()),
        nested: z.strictObject({ type: z.string() }),
      })
      .default(payload),
  })
  const exported = exportOperationSchema(schema)
  expect(exported.properties!.payload!.default).toEqual(payload)
  expect(validateOperationValue(exported, {}, true)).toEqual({
    success: true,
    data: { payload },
  })
})

test('manifest execution refuses invalid runtime validators, default-expanded budgets and huge timeouts before run', async () => {
  let calls = 0
  const plugin = {
    meta: { id: 'fixture-plugin', version: '0.1.0' },
    run: () => {
      calls++
      return 'result'
    },
  }
  const manifest = manifestFixture()
  manifest.commands[0]!.runtimeValidation.output = 'required'
  expect(
    await executeManifestCommand(
      manifest,
      'convert',
      { ...plugin, outputSchema: {} } as unknown as ExecutablePlugin,
      { text: 'x' },
      ctx,
      manifestTarget
    )
  ).toMatchObject({ success: false, error: { code: 'NOT_RUNNABLE' } })
  manifest.commands[0]!.runtimeValidation.output = 'schema-only'
  manifest.commands[0]!.inputSchema.properties!.text = {
    type: 'string',
    default: 'x'.repeat(100),
  }
  manifest.commands[0]!.resources.maxInputBytes = 50
  expect(
    await executeManifestCommand(
      manifest,
      'convert',
      plugin,
      {},
      ctx,
      manifestTarget
    )
  ).toMatchObject({ success: false, error: { code: 'INPUT_INVALID' } })
  manifest.commands[0]!.resources.maxInputBytes = 4096
  expect(
    await executeManifestCommand(
      manifest,
      'convert',
      plugin,
      { text: 'x' },
      ctx,
      manifestTarget,
      { timeoutMs: 2147483648 }
    )
  ).toMatchObject({ success: false, error: { code: 'TIMEOUT_INVALID' } })
  expect(calls).toBe(0)
})

test('serializable v1 preserves explicit identity and prototype default', () => {
  const value = manifestFixture()
  expect(
    parsePluginManifest(JSON.parse(JSON.stringify(value)), manifestTarget)
  ).toEqual(value)
  const omitted: Partial<typeof value> = { ...value }
  delete omitted.maturity
  expect(parsePluginManifest(omitted).maturity).toBe('prototype')
  expect(commandIdentity(value, value.commands[0]!)).toBe(
    'flowtools/fixture-plugin/convert'
  )
})

const rejected: Record<
  string,
  (value: ReturnType<typeof manifestFixture>) => unknown
> = {
  id: value => ({ ...value, id: '../other' }),
  publisher: value => ({ ...value, publisher: 'Other.User' }),
  version: value => ({ ...value, version: 'v1.0.0' }),
  range: value => ({ ...value, engines: { host: 'nonsense', sdk: '*' } }),
  format: value => ({ ...value, formatVersion: 2 }),
  critical: value => ({ ...value, certified: true }),
  commandCritical: value => ({
    ...value,
    commands: [{ ...value.commands[0], grants: ['all'] }],
  }),
  duplicateCommand: value => ({
    ...value,
    commands: [...value.commands, ...value.commands],
  }),
  duplicateFile: value => ({
    ...value,
    files: [...value.files, { ...value.files[0], path: 'DIST/COMMANDS.JS' }],
  }),
  hash: value => ({
    ...value,
    files: [{ ...value.files[0], sha256: 'unverified' }],
  }),
  missingEntry: value => ({ ...value, entries: { executor: 'missing.js' } }),
  noExecutor: value => ({ ...value, entries: { ui: 'dist/commands.js' } }),
  coldInteraction: value => ({
    ...value,
    commands: [
      {
        ...value.commands[0],
        supportsColdStart: true,
        interaction: 'required',
      },
    ],
  }),
  unsignedCertification: value => ({
    ...value,
    signature: { status: 'signed', certificate: 'user-claim' },
  }),
  badSchema: value => ({
    ...value,
    commands: [
      {
        ...value.commands[0],
        inputSchema: {
          type: 'object',
          additionalProperties: false,
          $ref: 'https://schema.invalid/',
        },
      },
    ],
  }),
  dependency: value => ({
    ...value,
    dependencies: {
      services: [{ id: 'provider', version: '^1.0.0' }],
      tools: [],
    },
  }),
}
for (const [label, mutate] of Object.entries(rejected))
  test(`manifest rejects ${label} before code loading`, () =>
    expect(() =>
      parsePluginManifest(mutate(manifestFixture()), manifestTarget)
    ).toThrow())

for (const path of [
  '../commands.js',
  '/commands.js',
  'C:/commands.js',
  'dist\\commands.js',
  'dist/%2e%2e/main.js',
  'dist/../main.js',
  'dist/CON.js',
  'dist/main.js:stream',
  'dist//main.js',
  'dist/main.js.',
])
  test(`manifest rejects unsafe entry ${path}`, () => {
    const value = manifestFixture()
    value.entries.executor = path
    value.files[0]!.path = path
    expect(() => parsePluginManifest(value)).toThrow()
  })

test('compatibility uses real semver ranges and exact platform/architecture pairs', () => {
  for (const changed of [
    { hostVersion: '1.0.0' },
    { sdkVersion: '1.0.0' },
    { hostVersion: '0.1.1-beta.1' },
    { platform: 'macos' as const },
    { arch: 'arm64' as const },
  ])
    expect(() =>
      parsePluginManifest(manifestFixture(), { ...manifestTarget, ...changed })
    ).toThrow()
})

test('operation subset rejects unsupported, inconsistent and recursive schemas', () => {
  for (const schema of [
    {},
    { type: 'string', pattern: '.*' },
    { type: 'object', required: ['missing'], additionalProperties: false },
    { type: 'array' },
    { type: 'string', minLength: 3, maxLength: 1 },
    { type: 'integer', default: 1.5 },
    { type: 'boolean', enum: [true, true] },
  ])
    expect(operationSchema.safeParse(schema).success).toBe(false)
  const cycle: Record<string, unknown> = { type: 'array' }
  cycle.items = cycle
  expect(operationSchema.safeParse(cycle).success).toBe(false)
  expect(isJsonValue({ data: undefined })).toBe(false)
  expect(isJsonValue({ data: NaN })).toBe(false)
  expect(isJsonValue(new Date())).toBe(false)
  let reads = 0
  expect(
    isJsonValue({
      get text() {
        reads++
        return 'secret'
      },
    })
  ).toBe(false)
  expect(reads).toBe(0)
})

for (const key of [
  'toString',
  'valueOf',
  'hasOwnProperty',
  '__defineGetter__',
]) {
  test(`closed operation schemas reject inherited property ${key} in values and defaults`, () => {
    const schema = operationSchema.parse({
      type: 'object',
      properties: {},
      additionalProperties: false,
    })
    const value = { [key]: 1 }
    expect(validateOperationValue(schema, value)).toEqual({ success: false })
    expect(validateOperationValue(schema, value, true)).toEqual({
      success: false,
    })
    const withDefault = { ...schema, default: value }
    expect(operationSchema.safeParse(withDefault).success).toBe(false)
    const manifest = manifestFixture()
    manifest.commands[0]!.inputSchema = withDefault
    expect(() => parsePluginManifest(manifest)).toThrow()
  })

  test(`operation schemas accept explicitly declared own property ${key}`, () => {
    const value = { [key]: 1 }
    const schema = operationSchema.parse({
      type: 'object',
      properties: { [key]: { type: 'integer' } },
      required: [key],
      additionalProperties: false,
      default: value,
    })
    expect(validateOperationValue(schema, value)).toEqual({
      success: true,
      data: value,
    })
    expect(validateOperationValue(schema, { [key]: 'wrong' })).toEqual({
      success: false,
    })
    const manifest = manifestFixture()
    manifest.commands[0]!.inputSchema = schema
    expect(parsePluginManifest(manifest).commands[0]!.inputSchema).toEqual(
      schema
    )
  })

  test(`open operation schemas accept undeclared own property ${key}`, () => {
    const value = { [key]: 1 }
    const schema = operationSchema.parse({
      type: 'object',
      properties: {},
      additionalProperties: true,
      default: value,
    })
    expect(validateOperationValue(schema, value)).toEqual({
      success: true,
      data: value,
    })
    const manifest = manifestFixture()
    manifest.commands[0]!.inputSchema = schema
    expect(parsePluginManifest(manifest).commands[0]!.inputSchema).toEqual(
      schema
    )
  })
}

test('operation validation preserves false/negative/array values, defaults and closed objects', () => {
  const schema = exportOperationSchema(
    z.object({
      count: z.number().default(2),
      enabled: z.boolean(),
      items: z.array(z.number()),
    })
  )
  expect(
    validateOperationValue(schema, { enabled: false, items: [-2, 0] }, true)
  ).toEqual({
    success: true,
    data: { count: 2, enabled: false, items: [-2, 0] },
  })
  expect(
    validateOperationValue(schema, { enabled: false, items: [], unknown: 2 })
      .success
  ).toBe(false)
  expect(
    validateOperationValue(schema, { enabled: false, items: ['1'] }).success
  ).toBe(false)
})

test('Zod custom refinement cannot claim complete serialized validation', () => {
  const schema = z.object({
    text: z.string().refine(value => value === 'allowed'),
  })
  expect(() => exportOperationSchema(schema)).toThrow()
  expect(exportOperationSchema(schema, 'required').type).toBe('object')
  expect(() =>
    exportOperationSchema(
      z.object({ text: z.string().transform(value => value.length) })
    )
  ).toThrow()
})

test('manifest execution refuses invalid identity/input and validates actual output', async () => {
  let runs = 0
  const plugin = {
    meta: { id: 'fixture-plugin', version: '0.1.0' },
    run: (_context: ToolContext, input: { text: string }) => {
      runs++
      return input.text
    },
  }
  expect(
    await executeManifestCommand(
      manifestFixture(),
      'convert',
      plugin,
      { text: 2 },
      ctx,
      manifestTarget
    )
  ).toMatchObject({ success: false, error: { code: 'INPUT_INVALID' } })
  expect(
    await executeManifestCommand(
      manifestFixture(),
      'missing',
      plugin,
      { text: 'x' },
      ctx,
      manifestTarget
    )
  ).toMatchObject({ success: false, error: { code: 'NOT_RUNNABLE' } })
  expect(runs).toBe(0)
  expect(
    await executeManifestCommand(
      manifestFixture(),
      'convert',
      plugin,
      { text: 'x' },
      ctx,
      manifestTarget
    )
  ).toMatchObject({ success: true, data: 'x' })
  expect(
    await executeManifestCommand(
      manifestFixture(),
      'convert',
      { ...plugin, run: () => 1 },
      { text: 'x' },
      ctx,
      manifestTarget
    )
  ).toMatchObject({ success: false, error: { code: 'OUTPUT_INVALID' } })
  const manifest = manifestFixture()
  manifest.commands[0]!.runtimeValidation.input = 'required'
  expect(
    await executeManifestCommand(
      manifest,
      'convert',
      plugin,
      { text: 'x' },
      ctx,
      manifestTarget
    )
  ).toMatchObject({ success: false, error: { code: 'NOT_RUNNABLE' } })
  expect(
    await executeManifestCommand(
      manifest,
      'convert',
      {
        ...plugin,
        inputSchema: z.object({
          text: z.string().refine(value => value === 'allowed'),
        }),
      },
      { text: 'x' },
      ctx,
      manifestTarget
    )
  ).toMatchObject({ success: false, error: { code: 'INPUT_INVALID' } })
})

test('output runtime validation and byte budgets fail without false success', async () => {
  const plugin = {
    meta: { id: 'fixture-plugin', version: '0.1.0' },
    run: () => 'wrong',
    outputSchema: z.literal('right'),
  }
  expect(
    await executeManifestCommand(
      manifestFixture(),
      'convert',
      plugin,
      { text: 'x' },
      ctx,
      manifestTarget
    )
  ).toMatchObject({ success: false, error: { code: 'OUTPUT_INVALID' } })
  const manifest = manifestFixture()
  manifest.commands[0]!.resources.maxInputBytes = 1
  expect(
    await executeManifestCommand(
      manifest,
      'convert',
      plugin,
      { text: 'x' },
      ctx,
      manifestTarget
    )
  ).toMatchObject({ success: false, error: { code: 'INPUT_INVALID' } })
})

test('legacy migration requires complete explicit package and scope declarations', () => {
  const { id, name, version, description, maturity, ...declarations } =
    manifestFixture()
  expect(
    migrateLegacyManifest(
      { id, name, version, maturity },
      { ...declarations, description }
    ).maturity
  ).toBe('prototype')
  expect(() =>
    migrateLegacyManifest(
      { id, name, version, run() {} },
      { ...declarations, description }
    )
  ).toThrow()
})
