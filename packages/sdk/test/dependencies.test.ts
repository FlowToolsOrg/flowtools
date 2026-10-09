import type { ServiceDefinition } from '../src/dependencies'

import { expect, test } from 'bun:test'
import { readFile } from 'node:fs/promises'

import { satisfies } from 'semver'
import { z } from 'zod'

import {
  dependencyRangeSchema,
  dependencyVersionSchema,
  parseDependencyDeclarations,
  parseServiceDefinitions,
  serviceDefinitionSchema,
  serviceDependencySchema,
  toolDependencySchema,
} from '../src/dependencies'
import { parsePluginManifest } from '../src/manifest'

import {
  serviceDefinitionFixture,
  serviceDependencyFixture,
  serviceManifestFixture,
  toolDependencyFixture,
} from './fixtures/dependencies-v1'
import { manifestFixture, manifestTarget } from './fixtures/manifest-v1'

test('numeric and canonical declarations match the shared Host supported domain', async () => {
  const vectors = z
    .array(
      z.strictObject({
        value: z.string(),
        canonicalValid: z.boolean(),
        rangeValid: z.boolean(),
      })
    )
    .min(1)
    .parse(
      JSON.parse(
        await readFile(
          new URL(
            '../../runtime-core/src/dependencies/npm-semver-declaration-fixtures.json',
            import.meta.url
          ),
          'utf8'
        )
      )
    )
  for (const vector of vectors) {
    expect(dependencyVersionSchema.safeParse(vector.value).success).toBe(
      vector.canonicalValid
    )
    expect(dependencyRangeSchema.safeParse(vector.value).success).toBe(
      vector.rangeValid
    )
    const service = { ...serviceDefinitionFixture(), version: vector.value }
    if (vector.canonicalValid)
      expect(parseServiceDefinitions([service])).toEqual([service])
    else expect(() => parseServiceDefinitions([service])).toThrow()
    const declarations = [
      {
        services: [{ ...serviceDependencyFixture(), version: vector.value }],
        tools: [],
      },
      {
        services: [
          { ...serviceDependencyFixture(), interfaceVersion: vector.value },
        ],
        tools: [],
      },
      {
        services: [],
        tools: [{ ...toolDependencyFixture(), version: vector.value }],
      },
    ]
    for (const declaration of declarations)
      if (vector.rangeValid)
        expect(parseDependencyDeclarations(declaration)).toEqual(declaration)
      else expect(() => parseDependencyDeclarations(declaration)).toThrow()
  }
})

test('dependency ranges match the shared Rust npm semver golden vectors', async () => {
  const vectors = z
    .array(
      z.strictObject({
        range: z.string(),
        version: z.string(),
        matches: z.boolean(),
      })
    )
    .length(47)
    .parse(
      JSON.parse(
        await readFile(
          new URL(
            '../../runtime-core/src/dependencies/npm-semver-fixtures.json',
            import.meta.url
          ),
          'utf8'
        )
      )
    )
  for (const vector of vectors) {
    expect(satisfies(vector.version, vector.range)).toBe(vector.matches)
    const declarations = {
      services: [
        {
          ...serviceDependencyFixture(),
          version: vector.range,
          interfaceVersion: vector.range,
        },
      ],
      tools: [{ ...toolDependencyFixture(), version: vector.range }],
    }
    expect(parseDependencyDeclarations(declarations)).toEqual(declarations)
  }
  for (const invalidRange of ['', 'not-a-range', '^1.2.3 || garbage'])
    expect(() =>
      parseDependencyDeclarations({
        services: [
          { ...serviceDependencyFixture(), interfaceVersion: invalidRange },
        ],
        tools: [],
      })
    ).toThrow()
  for (const invalidVersion of ['v1.2.3', '01.2.3', '^1.2.3', '1.2.3+build.7'])
    expect(() =>
      parseServiceDefinitions([
        { ...serviceDefinitionFixture(), version: invalidVersion },
      ])
    ).toThrow()
  expect(
    parseServiceDefinitions([
      { ...serviceDefinitionFixture(), version: '1.2.3-beta.1' },
    ])[0]?.version
  ).toBe('1.2.3-beta.1')
})

test('G4 preserves normalized legacy Manifest bytes and does not add service defaults', () => {
  const manifest = parsePluginManifest(manifestFixture())
  manifest.dependencies = {
    services: [{ publisher: 'flowtools', id: 'provider', version: '^1.0.0' }],
    tools: [{ publisher: 'flowtools', id: 'tool', version: '~2.0.0' }],
  }
  const before = JSON.stringify(manifest)
  const parsed = parsePluginManifest(manifest, manifestTarget)
  expect(JSON.stringify(parsed)).toBe(before)
  expect(parsed).not.toHaveProperty('services')
  expect(parsed.dependencies.services[0]).not.toHaveProperty('service')
  expect(parsed.dependencies.tools[0]).not.toHaveProperty('target')
})

test('service operations reuse command schemas, effects and budgets without identity claims', () => {
  const manifest = serviceManifestFixture()
  expect(parsePluginManifest(manifest, manifestTarget)).toEqual(manifest)
  const service = parseServiceDefinitions(manifest.services)[0]!
  expect(service.operations[0]).toEqual(manifestFixture().commands[0])
  expect(
    serviceDefinitionSchema.safeParse({
      ...service,
      publisher: 'spoofed',
    }).success
  ).toBe(false)
})

test('service entry alignment is enforced for explicit definitions', () => {
  const missing = serviceManifestFixture()
  delete missing.entries.services
  expect(() => parsePluginManifest(missing)).toThrow()
  const empty = serviceManifestFixture()
  empty.services = []
  expect(() => parsePluginManifest(empty)).toThrow()
  const undeclaredFile = serviceManifestFixture()
  undeclaredFile.files = undeclaredFile.files.slice(0, 1)
  expect(() => parsePluginManifest(undeclaredFile)).toThrow()
  const legacy = serviceManifestFixture()
  delete legacy.services
  expect(parsePluginManifest(legacy)).not.toHaveProperty('services')
  expect(
    parsePluginManifest({ ...manifestFixture(), services: [] }).services
  ).toEqual([])
})

const invalidDefinitions: Record<
  string,
  (service: ServiceDefinition) => unknown
> = {
  'interface version is canonical': value => ({ ...value, version: 'v1.2.0' }),
  'interface version is not a range': value => ({
    ...value,
    version: '^1.2.0',
  }),
  'nonempty operation inventory': value => ({ ...value, operations: [] }),
  'unique operation ID': value => ({
    ...value,
    operations: [...value.operations, ...value.operations],
  }),
  'operation budget': value => ({
    ...value,
    operations: [
      {
        ...value.operations[0],
        resources: { timeoutMs: 0, maxInputBytes: 1, maxOutputBytes: 1 },
      },
    ],
  }),
  'operation schema': value => ({
    ...value,
    operations: [
      {
        ...value.operations[0],
        inputSchema: {
          type: 'object',
          additionalProperties: false,
          $ref: 'https://schema.invalid/',
        },
      },
    ],
  }),
  'headless operation': value => ({
    ...value,
    operations: [{ ...value.operations[0], headless: false }],
  }),
  'no interaction': value => ({
    ...value,
    operations: [{ ...value.operations[0], interaction: 'optional' }],
  }),
  'unknown critical field': value => ({ ...value, grants: ['all'] }),
}
for (const [label, mutate] of Object.entries(invalidDefinitions))
  test(`service definitions reject ${label}`, () => {
    expect(() =>
      parseServiceDefinitions([mutate(serviceDefinitionFixture())])
    ).toThrow()
  })

test('services and operations have bounded distinct inventories', () => {
  const operation = manifestFixture().commands[0]!
  const service = serviceDefinitionFixture()
  const maximum = Array.from({ length: 64 }, (_, index) => ({
    ...service,
    id: `service-${index}`,
  }))
  expect(parseServiceDefinitions(maximum)).toHaveLength(64)
  expect(() =>
    parseServiceDefinitions([...maximum, { ...service, id: 'extra' }])
  ).toThrow()
  expect(() => parseServiceDefinitions([service, service])).toThrow()
  const operations = Array.from({ length: 64 }, (_, index) => ({
    ...operation,
    id: `operation-${index}`,
  }))
  expect(
    parseServiceDefinitions([{ ...service, operations }])[0]?.operations
  ).toHaveLength(64)
  expect(() =>
    parseServiceDefinitions([
      {
        ...service,
        operations: [...operations, { ...operation, id: 'extra' }],
      },
    ])
  ).toThrow()
})

test('service selectors are explicit, paired and bound to fixed publishers', () => {
  const selected = serviceDependencyFixture()
  expect(serviceDependencySchema.parse(selected)).toEqual(selected)
  const { service, interfaceVersion, ...legacy } = selected
  expect(serviceDependencySchema.parse(legacy)).toEqual(legacy)
  for (const value of [
    { ...legacy, service },
    { ...legacy, interfaceVersion },
    { ...selected, publisher: '*' },
    { ...selected, service: '../escape' },
    { ...selected, interfaceVersion: 'unresolved' },
    { ...selected, caller: 'spoofed' },
    { ...selected, providerPath: 'C:/untrusted' },
  ])
    expect(serviceDependencySchema.safeParse(value).success).toBe(false)
})

test('one provider may expose multiple services; duplicate selectors and mixed legacy claims fail', () => {
  const selected = serviceDependencyFixture()
  const second = {
    ...selected,
    service: 'other-service',
    version: '~1.1.0',
    interfaceVersion: '^2.0.0',
  }
  expect(
    parseDependencyDeclarations({ services: [selected, second], tools: [] })
      .services
  ).toHaveLength(2)
  expect(() =>
    parseDependencyDeclarations({
      services: [selected, { ...selected, version: '~1.1.0' }],
      tools: [],
    })
  ).toThrow()
  const legacy = {
    publisher: selected.publisher,
    id: selected.id,
    version: selected.version,
  }
  expect(() =>
    parseDependencyDeclarations({ services: [selected, legacy], tools: [] })
  ).toThrow()
  expect(() =>
    parseDependencyDeclarations({ services: [legacy, selected], tools: [] })
  ).toThrow()
  expect(() =>
    parseDependencyDeclarations({ services: [legacy, legacy], tools: [] })
  ).toThrow()
})

test('tool selectors require exact artifact digest, target and build flavor together', () => {
  const selected = toolDependencyFixture()
  expect(toolDependencySchema.parse(selected)).toEqual(selected)
  const legacy = {
    publisher: selected.publisher,
    id: selected.id,
    version: selected.version,
  }
  expect(toolDependencySchema.parse(legacy)).toEqual(legacy)
  for (const value of [
    { ...legacy, target: selected.target },
    { ...legacy, buildFlavor: selected.buildFlavor },
    { ...legacy, digest: selected.digest },
    { ...legacy, target: selected.target, buildFlavor: selected.buildFlavor },
    { ...legacy, target: selected.target, digest: selected.digest },
    { ...legacy, buildFlavor: selected.buildFlavor, digest: selected.digest },
    { ...selected, target: { platform: 'windows', arch: 'unknown' } },
    { ...selected, target: { ...selected.target, executable: 'untrusted' } },
    { ...selected, digest: 'unverified' },
    { ...selected, digest: 'A'.repeat(64) },
    { ...selected, buildFlavor: '../custom' },
    { ...selected, buildFlavor: '' },
    { ...selected, argv: ['--unsafe'] },
  ])
    expect(toolDependencySchema.safeParse(value).success).toBe(false)
})

test('distinct tool targets and flavors coexist; same-consumer artifact ambiguity fails', () => {
  const selected = toolDependencyFixture()
  const otherTarget = {
    ...selected,
    target: { platform: 'windows', arch: 'arm64' },
    digest: '2'.repeat(64),
  }
  const otherFlavor = {
    ...selected,
    buildFlavor: 'minimal',
    digest: '3'.repeat(64),
  }
  expect(
    parseDependencyDeclarations({
      services: [],
      tools: [selected, otherTarget, otherFlavor],
    }).tools
  ).toHaveLength(3)
  for (const other of [
    selected,
    { ...selected, digest: '2'.repeat(64) },
    { ...selected, version: '^3.0.0' },
  ])
    expect(() =>
      parseDependencyDeclarations({ services: [], tools: [selected, other] })
    ).toThrow()
  const legacy = {
    publisher: selected.publisher,
    id: selected.id,
    version: selected.version,
  }
  expect(() =>
    parseDependencyDeclarations({ services: [], tools: [legacy, selected] })
  ).toThrow()
  expect(() =>
    parseDependencyDeclarations({ services: [], tools: [selected, legacy] })
  ).toThrow()
})

test('dependency inventories and bounded parsers reject oversized or executable data', () => {
  const selected = serviceDependencyFixture()
  const references = Array.from({ length: 128 }, (_, index) => ({
    ...selected,
    id: `provider-${index}`,
  }))
  expect(
    parseDependencyDeclarations({ services: references, tools: [] }).services
  ).toHaveLength(128)
  expect(() =>
    parseDependencyDeclarations({
      services: [...references, { ...selected, id: 'extra' }],
      tools: [],
    })
  ).toThrow()
  const tools = Array.from({ length: 128 }, (_, index) => ({
    ...toolDependencyFixture(),
    id: `tool-${index}`,
  }))
  expect(
    parseDependencyDeclarations({ services: [], tools }).tools
  ).toHaveLength(128)
  expect(() =>
    parseDependencyDeclarations({
      services: [],
      tools: [...tools, { ...toolDependencyFixture(), id: 'extra' }],
    })
  ).toThrow()
  let getterCalls = 0
  const accessor = {
    get services() {
      getterCalls++
      return []
    },
    tools: [],
  }
  expect(() => parseDependencyDeclarations(accessor)).toThrow('bounded JSON')
  expect(getterCalls).toBe(0)
  const cyclic: Record<string, unknown> = {}
  cyclic.cycle = cyclic
  expect(() => parseServiceDefinitions(cyclic)).toThrow('bounded JSON')
  expect(() =>
    parseDependencyDeclarations({ services: [], tools: [], script: () => {} })
  ).toThrow('bounded JSON')
  expect(() =>
    parseDependencyDeclarations({
      services: [],
      tools: [],
      padding: 'x'.repeat(1_048_576),
    })
  ).toThrow('bounded JSON')
  expect(() =>
    parseDependencyDeclarations({ services: [], tools: [], certified: true })
  ).toThrow()
})
