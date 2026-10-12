import type {
  JsonValue,
  ManifestTarget,
  PluginManifestV1,
} from '@flowtools/sdk/manifest'
import type { FormatConfig } from 'oxfmt'

import { readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { parsePluginManifest } from '@flowtools/sdk/manifest'
import { format } from 'oxfmt'

type Path = (string | number)[]
type ManifestCase = {
  id: string
  manifest?: JsonValue
  rawManifest?: string
  target?: ManifestTarget
  accepted: boolean
  normalized?: PluginManifestV1
}

// Controlled JSON only. No plugin imports, artifact IO, trust pins or grants.
const base: JsonValue = {
  formatVersion: 1,
  publisher: 'fixture-author',
  id: 'fixture-plugin',
  name: '测试工具',
  description: 'Controlled Manifest conformance fixture',
  version: '1.2.3',
  type: 'tool',
  maturity: 'prototype',
  engines: { host: '^0.1.0', sdk: '>=0.0.0 <1.0.0' },
  targets: [{ platform: 'windows', arch: 'x64' }],
  entries: { executor: 'dist/commands.js' },
  files: [{ path: 'dist/commands.js', sha256: '0'.repeat(64), size: 0 }],
  signature: { status: 'unsigned' },
  dependencies: { services: [], tools: [] },
  commands: [
    {
      id: 'convert',
      name: '转换',
      description: 'Controlled command',
      headless: true,
      supportsColdStart: false,
      interaction: 'none',
      effects: [],
      permissions: [],
      resources: {
        timeoutMs: 30_000,
        maxInputBytes: 4096,
        maxOutputBytes: 4096,
      },
      runtimeValidation: { input: 'schema-only', output: 'schema-only' },
      inputSchema: {
        type: 'object',
        properties: {
          text: { type: 'string' },
          count: { type: 'integer', default: 1, minimum: 0 },
        },
        required: ['text'],
        additionalProperties: false,
      },
      outputSchema: { type: 'string' },
    },
  ],
}
const hostTarget: ManifestTarget = {
  hostVersion: '0.1.0',
  sdkVersion: '0.0.0',
  platform: 'windows',
  arch: 'x64',
}
const clone = <T>(value: T): T => structuredClone(value)
function member(value: JsonValue, key: string | number): JsonValue {
  if (Array.isArray(value) && typeof key === 'number') return value[key]!
  if (value !== null && typeof value === 'object' && !Array.isArray(value))
    return value[key]!
  throw new Error('Invalid controlled fixture path')
}
function at(value: JsonValue, path: Path): JsonValue {
  return path.reduce(member, value)
}
function set(value: JsonValue, path: Path, child: JsonValue): void {
  const parent = at(value, path.slice(0, -1))
  if (parent === null || typeof parent !== 'object')
    throw new Error('Invalid controlled fixture parent')
  Object.defineProperty(parent, path.at(-1)!, {
    value: clone(child),
    writable: true,
    configurable: true,
    enumerable: true,
  })
}
function omit(value: JsonValue, path: Path): void {
  const parent = at(value, path.slice(0, -1))
  if (parent === null || typeof parent !== 'object' || Array.isArray(parent))
    throw new Error('Invalid controlled omission')
  delete parent[String(path.at(-1))]
}
const command = at(base, ['commands', 0])
const serviceDependency: JsonValue = {
  publisher: 'fixture-author',
  id: 'provider',
  version: '^1.0.0',
  service: 'transform',
  interfaceVersion: '^2.0.0',
}
const toolDependency: JsonValue = {
  publisher: 'fixture-author',
  id: 'binary-tool',
  version: '~1.0.0',
  target: { platform: 'windows', arch: 'x64' },
  buildFlavor: 'standard',
  digest: '1'.repeat(64),
}
const serviceBase = clone(base)
set(serviceBase, ['entries', 'services'], 'dist/services.js')
set(
  serviceBase,
  ['files'],
  [
    at(base, ['files', 0]),
    { path: 'dist/services.js', sha256: '1'.repeat(64), size: 0 },
  ]
)
set(
  serviceBase,
  ['services'],
  [{ id: 'transform', version: '2.0.0', operations: [command] }]
)
const dependencyBase = clone(base)
set(dependencyBase, ['dependencies', 'services'], [serviceDependency])
set(dependencyBase, ['dependencies', 'tools'], [toolDependency])

const cases: ManifestCase[] = []
function add(
  id: string,
  mutate: (manifest: JsonValue) => void = () => {},
  fixture: JsonValue = base,
  target?: ManifestTarget
): void {
  const manifest = clone(fixture)
  mutate(manifest)
  const serialized = JSON.stringify(manifest)
  const input = {
    id,
    // Large budget vectors need no megabytes of indentation in the corpus.
    ...(serialized.length > 16_384
      ? { rawManifest: serialized }
      : { manifest }),
    ...(target ? { target: clone(target) } : {}),
  }
  try {
    const normalized = parsePluginManifest(manifest, target)
    cases.push({ ...input, accepted: true, normalized })
  } catch {
    cases.push({ ...input, accepted: false })
  }
}
function changed(
  id: string,
  path: Path,
  value: JsonValue,
  fixture: JsonValue = base
): void {
  add(id, manifest => set(manifest, path, value), fixture)
}
function numbered(value: JsonValue, count: number): JsonValue[] {
  return Array.from({ length: count }, (_, index) => {
    const item = clone(value)
    set(item, ['id'], `item-${index}`)
    return item
  })
}

add('valid-base')
add('valid-host-target', () => {}, base, hostTarget)
changed('valid-app-type', ['type'], 'app')
add('valid-default-maturity', manifest => omit(manifest, ['maturity']))
for (const maturity of ['prototype', 'experimental', 'beta', 'production'])
  changed(`valid-maturity-${maturity}`, ['maturity'], maturity)
changed('invalid-maturity-null', ['maturity'], null)
changed('invalid-maturity-stable', ['maturity'], 'stable')
changed('valid-empty-services', ['services'], [])
add('valid-services', () => {}, serviceBase)
add('valid-selected-dependencies', () => {}, dependencyBase)
add('valid-ui-only', manifest => {
  set(manifest, ['commands'], [])
  set(manifest, ['entries'], { ui: 'dist/commands.js' })
})
add(
  'valid-service-only',
  manifest => {
    set(manifest, ['commands'], [])
    omit(manifest, ['entries', 'executor'])
  },
  serviceBase
)
add(
  'valid-legacy-services-entry-without-definitions',
  manifest => omit(manifest, ['services']),
  serviceBase
)

const strictObjects: [string, Path, JsonValue][] = [
  ['manifest', [], base],
  ['engines', ['engines'], base],
  ['target', ['targets', 0], base],
  ['entries', ['entries'], base],
  ['file', ['files', 0], base],
  ['signature', ['signature'], base],
  ['command', ['commands', 0], base],
  ['runtime-validation', ['commands', 0, 'runtimeValidation'], base],
  ['resources', ['commands', 0, 'resources'], base],
  ['dependencies', ['dependencies'], base],
  ['service', ['services', 0], serviceBase],
  ['service-operation', ['services', 0, 'operations', 0], serviceBase],
  ['service-dependency', ['dependencies', 'services', 0], dependencyBase],
  ['tool-dependency', ['dependencies', 'tools', 0], dependencyBase],
  ['tool-target', ['dependencies', 'tools', 0, 'target'], dependencyBase],
]
for (const [name, path, fixture] of strictObjects)
  changed(`invalid-unknown-${name}`, [...path, 'grant'], true, fixture)
const nestedRequired: [string, Path, string[], JsonValue][] = [
  ['engines', ['engines'], ['host', 'sdk'], base],
  ['target', ['targets', 0], ['platform', 'arch'], base],
  ['file', ['files', 0], ['path', 'sha256', 'size'], base],
  ['signature', ['signature'], ['status'], base],
  [
    'runtime-validation',
    ['commands', 0, 'runtimeValidation'],
    ['input', 'output'],
    base,
  ],
  [
    'resources',
    ['commands', 0, 'resources'],
    ['timeoutMs', 'maxInputBytes', 'maxOutputBytes'],
    base,
  ],
  ['dependencies', ['dependencies'], ['services', 'tools'], base],
  ['service', ['services', 0], ['id', 'version', 'operations'], serviceBase],
  [
    'service-reference',
    ['dependencies', 'services', 0],
    ['publisher', 'id', 'version'],
    dependencyBase,
  ],
  [
    'tool-reference',
    ['dependencies', 'tools', 0],
    ['publisher', 'id', 'version'],
    dependencyBase,
  ],
  [
    'tool-target',
    ['dependencies', 'tools', 0, 'target'],
    ['platform', 'arch'],
    dependencyBase,
  ],
]
for (const [name, path, keys, fixture] of nestedRequired)
  for (const key of keys) {
    add(
      `invalid-nested-missing-${name}-${key}`,
      manifest => omit(manifest, [...path, key]),
      fixture
    )
    changed(`invalid-nested-null-${name}-${key}`, [...path, key], null, fixture)
  }
for (const key of [
  'formatVersion',
  'publisher',
  'id',
  'name',
  'description',
  'version',
  'type',
  'engines',
  'targets',
  'entries',
  'files',
  'signature',
  'dependencies',
  'commands',
]) {
  add(`invalid-missing-${key}`, manifest => omit(manifest, [key]))
  changed(`invalid-null-${key}`, [key], null)
}
for (const [key, value] of [
  ['formatVersion', 2],
  ['publisher', 'Other.User'],
  ['id', '../other'],
  ['name', ''],
  ['description', ''],
  ['type', 'plugin'],
] satisfies [string, JsonValue][])
  changed(`invalid-field-${key}`, [key], value)
for (const key of ['publisher', 'id']) {
  changed(`valid-${key}-max-length`, [key], 'a'.repeat(128))
  changed(`invalid-${key}-long`, [key], 'a'.repeat(129))
  changed(`invalid-${key}-double-hyphen`, [key], 'a--b')
}
for (const [key, limit] of [
  ['name', 256],
  ['description', 4096],
] as const) {
  changed(`valid-${key}-utf16-limit`, [key], '😀'.repeat(limit / 2))
  changed(`invalid-${key}-utf16-over`, [key], '😀'.repeat(limit / 2) + 'x')
}
for (const value of ['1.2.3-beta.1', '900719925474099.0.0'])
  changed(`valid-version-${value}`, ['version'], value)
for (const value of [
  'v1.2.3',
  '01.2.3',
  '^1.2.3',
  '1.2.3+build.7',
  '900719925474100.0.0',
  '1.2.3-01',
])
  changed(`invalid-version-${value}`, ['version'], value)
for (const range of [
  '*',
  '~0.1.0',
  '>=0.1.0 <1.0.0',
  '0.1 || 1.x',
  '0.1.0 - 0.2.0',
])
  changed(`valid-engine-range-${range}`, ['engines', 'host'], range)
for (const range of [
  '',
  'garbage',
  '^1.0.0 || garbage',
  '>=01.2.3',
  '^900719925474100.0.0',
])
  changed(`invalid-engine-range-${range}`, ['engines', 'host'], range)
for (const [name, range] of [
  ['nbsp-budget', '\u00a0'.repeat(256)],
  ['bom-budget', '\ufeff'.repeat(256)],
  ['bom-empty', '\ufeff'],
  ['bom-caret', '\ufeff^0.1.0'],
  ['nbsp-over-budget', '\u00a0'.repeat(257)],
  ['bom-over-budget', '\ufeff'.repeat(257)],
  ['nel-empty', '\u0085'],
  ['nel-caret', '\u0085^0.1.0'],
] as const)
  add(
    `engine-ecmascript-whitespace-${name}`,
    manifest => set(manifest, ['engines', 'host'], range),
    base,
    hostTarget
  )
for (const [name, target] of [
  ['host-range', { ...hostTarget, hostVersion: '1.0.0' }],
  ['sdk-range', { ...hostTarget, sdkVersion: '1.0.0' }],
  ['host-prerelease', { ...hostTarget, hostVersion: '0.1.1-beta.1' }],
  ['sdk-prerelease', { ...hostTarget, sdkVersion: '0.0.1-beta.1' }],
  ['platform', { ...hostTarget, platform: 'linux' }],
  ['architecture', { ...hostTarget, arch: 'arm64' }],
  ['host-version', { ...hostTarget, hostVersion: 'garbage' }],
  ['sdk-version', { ...hostTarget, sdkVersion: 'garbage' }],
] satisfies [string, ManifestTarget][])
  add(`invalid-compatibility-${name}`, () => {}, base, target)
add(
  'valid-compatibility-prerelease-explicit',
  manifest => {
    set(manifest, ['engines', 'host'], '>=0.1.0-beta.1 <0.2.0')
  },
  base,
  { ...hostTarget, hostVersion: '0.1.0-beta.2' }
)
add('valid-compatibility-normalized-host-version', () => {}, base, {
  ...hostTarget,
  hostVersion: 'v0.1.0',
})

changed('invalid-targets-empty', ['targets'], [])
changed(
  'invalid-target-duplicate',
  ['targets'],
  [at(base, ['targets', 0]), at(base, ['targets', 0])]
)
changed('invalid-target-platform', ['targets', 0, 'platform'], 'android')
changed('invalid-target-architecture', ['targets', 0, 'arch'], 'x86')
changed(
  'valid-target-web-wasm32',
  ['targets'],
  [{ platform: 'web', arch: 'wasm32' }]
)
changed('invalid-entries-empty', ['entries'], {})
changed('invalid-entries-duplicate', ['entries'], {
  executor: 'dist/commands.js',
  ui: 'dist/commands.js',
})
changed('invalid-entries-missing-file', ['entries', 'executor'], 'missing.js')
changed('invalid-commands-without-executor', ['entries'], {
  ui: 'dist/commands.js',
})
changed(
  'invalid-service-definitions-without-entry',
  ['entries'],
  { executor: 'dist/commands.js' },
  serviceBase
)
changed('invalid-empty-definitions-with-entry', ['services'], [], serviceBase)
changed(
  'invalid-service-entry-null',
  ['entries', 'services'],
  null,
  serviceBase
)
for (const [name, path] of [
  ['parent', '../commands.js'],
  ['absolute', '/commands.js'],
  ['drive', 'C:/commands.js'],
  ['backslash', 'dist\\commands.js'],
  ['encoded', 'dist/%2e%2e/main.js'],
  ['dot', 'dist/../main.js'],
  ['device', 'dist/CON.js'],
  ['ads', 'dist/main.js:stream'],
  ['empty-segment', 'dist//main.js'],
  ['trailing-dot', 'dist/main.js.'],
  ['control', 'dist/\u0001.js'],
  ['query', 'dist/main.js?token'],
])
  add(`invalid-path-${name}`, manifest => {
    set(manifest, ['entries', 'executor'], path!)
    set(manifest, ['files', 0, 'path'], path!)
  })
for (const [name, path] of [
  ['unicode', 'dist/文件.js'],
  ['uppercase', 'Dist/Commands.js'],
  ['long', `${'a'.repeat(241)}.js`],
])
  add(`valid-sdk-path-${name}`, manifest => {
    set(manifest, ['entries', 'executor'], path!)
    set(manifest, ['files', 0, 'path'], path!)
  })
changed('invalid-files-empty', ['files'], [])
changed(
  'invalid-file-duplicate',
  ['files'],
  [at(base, ['files', 0]), at(base, ['files', 0])]
)
changed(
  'invalid-file-case-duplicate',
  ['files'],
  [
    at(base, ['files', 0]),
    { path: 'DIST/COMMANDS.JS', size: 0, sha256: '0'.repeat(64) },
  ]
)
// String lowercasing is contextual: final sigma ignores case-ignorable marks.
for (const [name, upper, lower] of [
  ['sigma-distinct', 'ΟΣ/entry.js', 'οσ/entry.js'],
  ['sigma-duplicate', 'ΟΣ/entry.js', 'ος/entry.js'],
  ['sigma-zwj-before-distinct', 'Ο\u200dΣ/entry.js', 'ο\u200dσ/entry.js'],
  ['sigma-zwj-before-duplicate', 'Ο\u200dΣ/entry.js', 'ο\u200dς/entry.js'],
  ['sigma-zwj-after-distinct', 'ΟΣ\u200d/entry.js', 'οσ\u200d/entry.js'],
  ['sigma-zwj-after-duplicate', 'ΟΣ\u200d/entry.js', 'ος\u200d/entry.js'],
  ['sigma-combining-distinct', 'Ο\u0345Σ/entry.js', 'ο\u0345σ/entry.js'],
  ['sigma-combining-duplicate', 'Ο\u0345Σ/entry.js', 'ο\u0345ς/entry.js'],
] as const)
  add(`file-unicode-fold-${name}`, manifest => {
    set(manifest, ['entries', 'executor'], upper)
    set(
      manifest,
      ['files'],
      [upper, lower].map(path => ({ path, size: 0, sha256: '0'.repeat(64) }))
    )
  })
changed('invalid-file-hash', ['files', 0, 'sha256'], 'unverified')
changed('invalid-file-hash-uppercase', ['files', 0, 'sha256'], 'A'.repeat(64))
for (const [name, size] of [
  ['negative', -1],
  ['fraction', 1.5],
  ['large', 67_108_865],
] as const)
  changed(`invalid-file-size-${name}`, ['files', 0, 'size'], size)
changed('valid-file-size-limit', ['files', 0, 'size'], 67_108_864)
changed(
  'invalid-files-total',
  ['files'],
  [
    { ...(at(base, ['files', 0]) as object), size: 67_108_864 },
    { path: 'other.js', size: 1, sha256: '1'.repeat(64) },
  ]
)
for (const count of [1024, 1025])
  add(`files-count-${count}`, manifest => {
    set(
      manifest,
      ['files'],
      Array.from({ length: count }, (_, index) => ({
        path: index === 0 ? 'dist/commands.js' : `dist/file-${index}.js`,
        size: 0,
        sha256: '0'.repeat(64),
      }))
    )
  })
changed('invalid-signature-claim', ['signature', 'status'], 'signed')

const commandPath: Path = ['commands', 0]
for (const key of [
  'id',
  'name',
  'description',
  'inputSchema',
  'outputSchema',
  'runtimeValidation',
  'headless',
  'supportsColdStart',
  'interaction',
  'effects',
  'permissions',
  'resources',
]) {
  add(`invalid-command-missing-${key}`, manifest =>
    omit(manifest, [...commandPath, key])
  )
  changed(`invalid-command-null-${key}`, [...commandPath, key], null)
}
changed('invalid-command-duplicate', ['commands'], [command, command])
for (const count of [256, 257])
  changed(`commands-count-${count}`, ['commands'], numbered(command, count))
changed('valid-cold-start', [...commandPath, 'supportsColdStart'], true)
add('invalid-cold-start-not-headless', manifest => {
  set(manifest, [...commandPath, 'supportsColdStart'], true)
  set(manifest, [...commandPath, 'headless'], false)
})
add('invalid-cold-start-required-interaction', manifest => {
  set(manifest, [...commandPath, 'supportsColdStart'], true)
  set(manifest, [...commandPath, 'interaction'], 'required')
})
changed('invalid-command-headless-type', [...commandPath, 'headless'], 'true')
changed(
  'invalid-command-interaction',
  [...commandPath, 'interaction'],
  'silent'
)
changed('invalid-command-input-not-object', [...commandPath, 'inputSchema'], {
  type: 'string',
})
for (const key of ['input', 'output']) {
  changed(
    `valid-runtime-validation-required-${key}`,
    [...commandPath, 'runtimeValidation', key],
    'required'
  )
  changed(
    `invalid-runtime-validation-${key}`,
    [...commandPath, 'runtimeValidation', key],
    'trusted'
  )
}
for (const key of ['name', 'description']) {
  const limit = key === 'name' ? 256 : 4096
  changed(
    `valid-command-${key}-utf16-limit`,
    [...commandPath, key],
    '😀'.repeat(limit / 2)
  )
  changed(
    `invalid-command-${key}-utf16-over`,
    [...commandPath, key],
    '😀'.repeat(limit / 2) + 'x'
  )
}
changed(
  'invalid-effects-duplicate',
  [...commandPath, 'effects'],
  ['file-read', 'file-read']
)
changed('invalid-effects-unknown', [...commandPath, 'effects'], ['shell'])
changed(
  'valid-effects-all',
  [...commandPath, 'effects'],
  [
    'file-read',
    'file-create',
    'file-replace',
    'file-delete',
    'network-read',
    'network-send',
    'clipboard-read',
    'clipboard-write',
    'data-read',
    'data-write',
    'notification',
    'tool-execute',
  ]
)
const permission: JsonValue = {
  capability: 'fs',
  operations: ['read'],
  scopes: ['selected'],
}
for (const key of ['capability', 'operations', 'scopes']) {
  add(`invalid-permission-missing-${key}`, manifest => {
    const item = clone(permission)
    omit(item, [key])
    set(manifest, [...commandPath, 'permissions'], [item])
  })
  add(`invalid-permission-null-${key}`, manifest => {
    const item = clone(permission)
    set(item, [key], null)
    set(manifest, [...commandPath, 'permissions'], [item])
  })
}
changed('valid-permission', [...commandPath, 'permissions'], [permission])
changed(
  'invalid-permissions-duplicate-capability',
  [...commandPath, 'permissions'],
  [permission, permission]
)
for (const [name, key, value] of [
  ['unknown-field', 'grant', true],
  ['unknown-capability', 'capability', 'shell'],
  ['empty-operations', 'operations', []],
  ['duplicate-operations', 'operations', ['read', 'read']],
  ['invalid-operation', 'operations', ['../read']],
  ['duplicate-scopes', 'scopes', ['x', 'x']],
  ['empty-scope', 'scopes', ['']],
  ['long-scope', 'scopes', ['x'.repeat(1025)]],
] satisfies [string, string, JsonValue][])
  add(`invalid-permission-${name}`, manifest => {
    const item = clone(permission)
    set(item, [key], value)
    set(manifest, [...commandPath, 'permissions'], [item])
  })
for (const count of [32, 33])
  add(`permission-operations-count-${count}`, manifest => {
    const item = clone(permission)
    set(
      item,
      ['operations'],
      Array.from({ length: count }, (_, n) => `operation-${n}`)
    )
    set(manifest, [...commandPath, 'permissions'], [item])
  })
for (const count of [64, 65])
  add(`permission-scopes-count-${count}`, manifest => {
    const item = clone(permission)
    set(
      item,
      ['scopes'],
      Array.from({ length: count }, (_, n) => `scope-${n}`)
    )
    set(manifest, [...commandPath, 'permissions'], [item])
  })
for (const [key, limit] of [
  ['timeoutMs', 2_147_483_647],
  ['maxInputBytes', 1_048_576],
  ['maxOutputBytes', 16_777_216],
] as const)
  for (const [name, value] of [
    ['minimum', 1],
    ['maximum', limit],
    ['zero', 0],
    ['fraction', 1.5],
    ['overflow', limit + 1],
  ] as const)
    changed(
      `resource-${key}-${name}`,
      [...commandPath, 'resources', key],
      value
    )

const schemas: [string, JsonValue][] = [
  ['valid-boolean', { type: 'boolean', default: false }],
  ['valid-negative', { type: 'number', minimum: -10, maximum: 0, default: -1 }],
  [
    'valid-array',
    { type: 'array', items: { type: 'integer' }, default: [-1, 0] },
  ],
  [
    'valid-anyof',
    { anyOf: [{ type: 'string' }, { type: 'null' }], default: null },
  ],
  [
    'valid-const-object',
    { const: { a: 1, b: false }, default: { b: false, a: 1 } },
  ],
  [
    'valid-string-codepoints',
    { type: 'string', minLength: 1, maxLength: 1, default: '😀' },
  ],
  ['invalid-empty', {}],
  ['invalid-unknown-type', { type: 'date' }],
  ['invalid-pattern', { type: 'string', pattern: '.*' }],
  ['invalid-format', { type: 'string', format: 'email' }],
  ['invalid-ref', { $ref: 'https://schema.invalid/' }],
  ['invalid-object-open-unspecified', { type: 'object' }],
  [
    'invalid-object-required-missing',
    { type: 'object', additionalProperties: false, required: ['missing'] },
  ],
  [
    'invalid-object-required-duplicate',
    {
      type: 'object',
      additionalProperties: false,
      properties: { a: { type: 'string' } },
      required: ['a', 'a'],
    },
  ],
  ['invalid-array-items-missing', { type: 'array' }],
  ['invalid-items-on-string', { type: 'string', items: { type: 'string' } }],
  ['invalid-properties-on-string', { type: 'string', properties: {} }],
  ['invalid-required-on-string', { type: 'string', required: [] }],
  [
    'invalid-additional-on-string',
    { type: 'string', additionalProperties: false },
  ],
  ['invalid-length-on-number', { type: 'number', minLength: 0 }],
  ['invalid-minimum-on-string', { type: 'string', minimum: 0 }],
  ['invalid-count-on-string', { type: 'string', minItems: 0 }],
  ['invalid-length-order', { type: 'string', minLength: 2, maxLength: 1 }],
  [
    'invalid-item-order',
    { type: 'array', items: { type: 'null' }, minItems: 2, maxItems: 1 },
  ],
  ['invalid-number-order', { type: 'number', minimum: 2, maximum: 1 }],
  ['invalid-fraction-count', { type: 'string', minLength: 0.5 }],
  ['invalid-negative-count', { type: 'string', maxLength: -1 }],
  ['invalid-large-count', { type: 'string', maxLength: 1_048_577 }],
  ['invalid-enum-empty', { enum: [] }],
  ['invalid-enum-duplicate', { enum: [false, false] }],
  [
    'invalid-enum-structural-duplicate',
    {
      enum: [
        { a: 1, b: 2 },
        { b: 2, a: 1 },
      ],
    },
  ],
  ['invalid-anyof-empty', { anyOf: [] }],
  ['invalid-anyof-child', { anyOf: [{ type: 'array' }] }],
  ['invalid-default-type', { type: 'integer', default: 1.5 }],
  ['invalid-default-enum', { enum: [0, false], default: true }],
  [
    'invalid-default-exclusive',
    { type: 'number', exclusiveMinimum: 1, default: 1 },
  ],
  [
    'invalid-default-closed',
    { type: 'object', additionalProperties: false, default: { a: 1 } },
  ],
  [
    'invalid-default-needs-child-default',
    {
      type: 'object',
      additionalProperties: false,
      properties: { a: { type: 'string', default: 'x' } },
      required: ['a'],
      default: {},
    },
  ],
]
for (const [name, schema] of schemas)
  changed(`schema-${name}`, [...commandPath, 'outputSchema'], schema)
for (const key of [
  'toString',
  'valueOf',
  'hasOwnProperty',
  '__defineGetter__',
]) {
  changed(
    `schema-invalid-default-inherited-property-${key}`,
    [...commandPath, 'outputSchema'],
    {
      type: 'object',
      properties: {},
      additionalProperties: false,
      default: { [key]: 1 },
    }
  )
  changed(
    `schema-valid-default-explicit-own-property-${key}`,
    [...commandPath, 'outputSchema'],
    {
      type: 'object',
      properties: { [key]: { type: 'integer' } },
      required: [key],
      additionalProperties: false,
      default: { [key]: 1 },
    }
  )
  changed(
    `schema-valid-default-additional-own-property-${key}`,
    [...commandPath, 'outputSchema'],
    {
      type: 'object',
      properties: {},
      additionalProperties: true,
      default: { [key]: 1 },
    }
  )
}
for (const key of ['__proto__', 'constructor', 'prototype'])
  add(`invalid-json-dangerous-key-${key}`, manifest =>
    set(manifest, [...commandPath, 'outputSchema', key], true)
  )
for (const count of [16, 17])
  changed(`schema-anyof-count-${count}`, [...commandPath, 'outputSchema'], {
    anyOf: Array.from({ length: count }, (_, index) => ({ const: index })),
  })
for (const count of [256, 257])
  changed(`schema-enum-count-${count}`, [...commandPath, 'outputSchema'], {
    enum: Array.from({ length: count }, (_, index) => index),
  })
for (const count of [128, 129])
  changed(
    `schema-property-name-count-${count}`,
    [...commandPath, 'outputSchema'],
    {
      type: 'object',
      additionalProperties: false,
      properties: { ['a'.repeat(count)]: { type: 'string' } },
    }
  )

// JSON tree limits are SDK value constraints, separate from signed raw bytes.
// The scalar leaves are at Manifest depth 4 + count in these cases.
for (const count of [28, 29]) {
  let nested: JsonValue = 0
  for (let index = 0; index < count; index++) nested = [nested]
  changed(`json-depth-${count + 4}`, [...commandPath, 'outputSchema'], {
    const: nested,
  })
}
changed('invalid-schema-json-size', [...commandPath, 'outputSchema'], {
  const: 'x'.repeat(65_536),
})
changed(
  'invalid-manifest-json-size',
  ['commands'],
  numbered(command, 256).map(item => {
    set(item, ['description'], 'x'.repeat(4096))
    set(item, ['outputSchema'], {
      type: 'string',
      description: 'x'.repeat(4096),
    })
    return item
  })
)
changed(
  'invalid-manifest-json-nodes',
  ['commands'],
  numbered(command, 128).map(item => {
    set(item, ['outputSchema'], {
      const: Array.from({ length: 1000 }, () => 0),
    })
    return item
  })
)

for (const [name, path, value] of [
  ['id', ['id'], 'bad/id'],
  ['version', ['version'], '^1.0.0'],
  ['operations-empty', ['operations'], []],
  ['operations-null', ['operations'], null],
  ['headless', ['operations', 0, 'headless'], false],
  ['interaction', ['operations', 0, 'interaction'], 'optional'],
] satisfies [string, Path, JsonValue][])
  changed(
    `invalid-service-${name}`,
    ['services', 0, ...path],
    value,
    serviceBase
  )
changed(
  'valid-service-prerelease',
  ['services', 0, 'version'],
  '2.0.0-beta.1',
  serviceBase
)
changed(
  'invalid-service-duplicate',
  ['services'],
  [at(serviceBase, ['services', 0]), at(serviceBase, ['services', 0])],
  serviceBase
)
changed(
  'invalid-service-operation-duplicate',
  ['services', 0, 'operations'],
  [command, command],
  serviceBase
)
for (const count of [64, 65]) {
  changed(
    `services-count-${count}`,
    ['services'],
    numbered(at(serviceBase, ['services', 0]), count),
    serviceBase
  )
  changed(
    `service-operations-count-${count}`,
    ['services', 0, 'operations'],
    numbered(command, count),
    serviceBase
  )
}
for (const className of ['services', 'tools'] as const) {
  const item = className === 'services' ? serviceDependency : toolDependency
  for (const count of [128, 129])
    changed(
      `dependencies-${className}-count-${count}`,
      ['dependencies', className],
      numbered(item, count),
      dependencyBase
    )
  changed(
    `invalid-dependencies-${className}-duplicate`,
    ['dependencies', className],
    [item, item],
    dependencyBase
  )
  changed(
    `invalid-dependencies-${className}-range`,
    ['dependencies', className, 0, 'version'],
    'garbage',
    dependencyBase
  )
  changed(
    `invalid-dependencies-${className}-publisher`,
    ['dependencies', className, 0, 'publisher'],
    '*',
    dependencyBase
  )
}
for (const key of ['service', 'interfaceVersion']) {
  add(
    `invalid-service-selector-missing-${key}`,
    manifest => omit(manifest, ['dependencies', 'services', 0, key]),
    dependencyBase
  )
  changed(
    `invalid-service-selector-null-${key}`,
    ['dependencies', 'services', 0, key],
    null,
    dependencyBase
  )
}
add(
  'valid-legacy-service-reference',
  manifest => {
    omit(manifest, ['dependencies', 'services', 0, 'service'])
    omit(manifest, ['dependencies', 'services', 0, 'interfaceVersion'])
  },
  dependencyBase
)
add(
  'invalid-mixed-service-selector',
  manifest => {
    const legacy = clone(serviceDependency)
    omit(legacy, ['service'])
    omit(legacy, ['interfaceVersion'])
    set(manifest, ['dependencies', 'services'], [serviceDependency, legacy])
  },
  dependencyBase
)
add(
  'valid-multiple-services-one-provider',
  manifest => {
    const other = clone(serviceDependency)
    set(other, ['service'], 'other')
    set(manifest, ['dependencies', 'services'], [serviceDependency, other])
  },
  dependencyBase
)
for (const key of ['target', 'buildFlavor', 'digest']) {
  add(
    `invalid-tool-selector-missing-${key}`,
    manifest => omit(manifest, ['dependencies', 'tools', 0, key]),
    dependencyBase
  )
  changed(
    `invalid-tool-selector-null-${key}`,
    ['dependencies', 'tools', 0, key],
    null,
    dependencyBase
  )
}
changed(
  'invalid-tool-selector-flavor',
  ['dependencies', 'tools', 0, 'buildFlavor'],
  '../executable',
  dependencyBase
)
changed(
  'invalid-tool-selector-digest',
  ['dependencies', 'tools', 0, 'digest'],
  'unverified',
  dependencyBase
)
add(
  'valid-legacy-tool-reference',
  manifest => {
    for (const key of ['target', 'buildFlavor', 'digest'])
      omit(manifest, ['dependencies', 'tools', 0, key])
  },
  dependencyBase
)
add(
  'invalid-mixed-tool-selector',
  manifest => {
    const legacy = clone(toolDependency)
    for (const key of ['target', 'buildFlavor', 'digest']) omit(legacy, [key])
    set(manifest, ['dependencies', 'tools'], [toolDependency, legacy])
  },
  dependencyBase
)
add(
  'valid-multiple-tool-targets',
  manifest => {
    const other = clone(toolDependency)
    set(other, ['target', 'arch'], 'arm64')
    set(manifest, ['dependencies', 'tools'], [toolDependency, other])
  },
  dependencyBase
)

// Preserve numeric spelling for Rust byte-to-value parity. Duplicate raw keys,
// UTF-8/inventory/signature constraints remain separate package-profile tests.
const rawBase = JSON.stringify(base)
for (const [id, rawManifest] of [
  [
    'raw-format-integral-float',
    rawBase.replace('"formatVersion":1', '"formatVersion":1.0'),
  ],
  ['raw-size-integral-float', rawBase.replace('"size":0', '"size":0.0')],
  [
    'raw-timeout-integral-exponent',
    rawBase.replace('"timeoutMs":30000', '"timeoutMs":3e4'),
  ],
  [
    'raw-schema-number-rounding',
    rawBase.replace(
      '"outputSchema":{"type":"string"}',
      '"outputSchema":{"type":"number","default":9007199254740993}'
    ),
  ],
  [
    'raw-schema-positive-exponent-roundtrip',
    rawBase.replace(
      '"outputSchema":{"type":"string"}',
      '"outputSchema":{"type":"number","default":1.2345678901234567e+100}'
    ),
  ],
  [
    'raw-schema-negative-exponent-roundtrip',
    rawBase.replace(
      '"outputSchema":{"type":"string"}',
      '"outputSchema":{"type":"number","default":-1.2345678901234567e+100}'
    ),
  ],
  [
    'raw-schema-numeric-equivalent-literals',
    rawBase.replace(
      '"outputSchema":{"type":"string"}',
      '"outputSchema":{"const":1.2345678901234566e100,"default":1.2345678901234567e100}'
    ),
  ],
  [
    'raw-schema-numeric-distinct-literals',
    rawBase.replace(
      '"outputSchema":{"type":"string"}',
      '"outputSchema":{"const":1.2345678901234567e100,"default":1.2345678901234568e100}'
    ),
  ],
  [
    'raw-schema-negative-zero',
    rawBase.replace(
      '"outputSchema":{"type":"string"}',
      '"outputSchema":{"type":"number","default":-0}'
    ),
  ],
  [
    'raw-schema-numeric-enum-duplicate',
    rawBase.replace(
      '"outputSchema":{"type":"string"}',
      '"outputSchema":{"enum":[1,1.0]}'
    ),
  ],
] as const) {
  try {
    const normalized = parsePluginManifest(JSON.parse(rawManifest))
    cases.push({ id, rawManifest, accepted: true, normalized })
  } catch {
    cases.push({ id, rawManifest, accepted: false })
  }
}

const ids = new Set(cases.map(item => item.id))
if (!cases.length || ids.size !== cases.length)
  throw new Error('Manifest conformance cases must be nonempty and distinct')
const path = fileURLToPath(
  new URL('./manifest-conformance.json', import.meta.url)
)
const config = JSON.parse(
  readFileSync(new URL('../../.oxfmtrc.json', import.meta.url), 'utf8')
) as FormatConfig
const formatted = await format(
  path,
  JSON.stringify({ formatVersion: 1, cases }),
  config
)
if (formatted.errors.length)
  throw new Error('Cannot format SDK Manifest conformance corpus')
const output = formatted.code.replaceAll('\r\n', '\n')
const args = process.argv.slice(2)
if (args.length > 1 || args.some(arg => !['--check', '--write'].includes(arg)))
  throw new Error('Use --check (default) or explicit --write')
if (args[0] === '--write') writeFileSync(path, output)
else if (readFileSync(path, 'utf8').replaceAll('\r\n', '\n') !== output)
  throw new Error(
    'SDK Manifest conformance drift; inspect the contract changes'
  )
process.stdout.write(
  `SDK Manifest conformance: ${cases.length} cases, ${cases.filter(item => item.accepted).length} accepted; ${args[0] === '--write' ? 'written' : 'unchanged'}.\n`
)
