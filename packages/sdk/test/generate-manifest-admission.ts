import { writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { parsePluginManifest } from '../src/manifest'

import { manifestFixture, manifestTarget } from './fixtures/manifest-v1'

const base = manifestFixture()
const command = base.commands[0]!
const service = { id: 'text-service', version: '1.0.0', operations: [command] }
const reference = { publisher: 'flowtools', id: 'provider', version: '^1.0.0' }
const selected = {
  ...reference,
  service: 'text-service',
  interfaceVersion: '^1.0.0',
}
const tool = {
  ...reference,
  target: manifestTargetToPlatform(),
  buildFlavor: 'standard',
  digest: 'a'.repeat(64),
}
function manifestTargetToPlatform() {
  return { platform: 'windows', arch: 'x64' }
}
type Patch = { path: string; value?: unknown; remove?: true }
const cases: {
  name: string
  accept: boolean
  patches: Patch[]
  target?: Partial<typeof manifestTarget>
}[] = []
function add(name: string, accept: boolean, path: string, value: unknown) {
  cases.push({ name, accept, patches: [{ path, value }] })
}
add('valid', true, '/name', 'Fixture')
cases.push({
  name: 'maturity-default',
  accept: true,
  patches: [{ path: '/maturity', remove: true }],
})
for (const [path, value] of [
  ['/formatVersion', 2],
  ['/publisher', '../publisher'],
  ['/unknown', true],
  ['/version', 'v1.0.0'],
  ['/version', '1.0.0+build'],
  ['/engines/host', 'bad'],
  ['/engines/sdk', '900719925474100.0.0'],
  ['/targets', [...base.targets, ...base.targets]],
  ['/entries', {}],
  ['/entries', { ui: base.entries.executor }],
  ['/entries', { executor: 'missing.js' }],
  ['/entries', { ui: base.entries.executor, executor: base.entries.executor }],
  ['/files', [...base.files, ...base.files]],
  ['/files/0/size', 67_108_865],
  ['/files/0/sha256', 'unsigned'],
  ['/signature', { status: 'signed' }],
  ['/commands', [command, command]],
  ['/commands/0/unknown', true],
  ['/commands/0/inputSchema', { type: 'string' }],
  ['/commands/0/effects', ['data-read', 'data-read']],
  ['/commands/0/resources/timeoutMs', 0],
  ['/commands/0/runtimeValidation/input', 'ignored'],
  ['/commands/0', { ...command, supportsColdStart: true, headless: false }],
  [
    '/commands/0',
    { ...command, supportsColdStart: true, interaction: 'required' },
  ],
  ['/commands/0/name', '😀'.repeat(129)],
] as const)
  add(`reject-${cases.length}-${path}`, false, path, value)
add('file-budget-total', false, '/files', [
  { ...base.files[0], size: 40_000_000 },
  { ...base.files[0], path: 'other.js', size: 40_000_000 },
])
for (const permissions of [
  [{ capability: 'db', operations: ['read', 'read'], scopes: [] }],
  [{ capability: 'db', operations: ['read'], scopes: ['x', 'x'] }],
  [
    { capability: 'db', operations: ['read'], scopes: [] },
    { capability: 'db', operations: ['write'], scopes: [] },
  ],
])
  add(
    `permissions-${cases.length}`,
    false,
    '/commands/0/permissions',
    permissions
  )
for (const path of [
  '../main.js',
  '/main.js',
  'C:/main.js',
  'dist\\main.js',
  'dist/CON.js',
  'dist/main.js:ads',
  'dist//main.js',
  'dist/main.js.',
]) {
  cases.push({
    name: `path-${path}`,
    accept: false,
    patches: [
      { path: '/entries/executor', value: path },
      { path: '/files/0/path', value: path },
    ],
  })
}
const servicePatches: Patch[] = [
  { path: '/entries/services', value: 'services.js' },
  {
    path: '/files',
    value: [
      ...base.files,
      { path: 'services.js', sha256: '1'.repeat(64), size: 0 },
    ],
  },
]
for (const [name, accept, definitions] of [
  ['service-valid', true, [service]],
  ['service-empty', false, []],
  ['service-duplicate', false, [service, service]],
  [
    'service-operation-duplicate',
    false,
    [{ ...service, operations: [command, command] }],
  ],
  [
    'service-ui',
    false,
    [{ ...service, operations: [{ ...command, headless: false }] }],
  ],
  [
    'service-interaction',
    false,
    [{ ...service, operations: [{ ...command, interaction: 'optional' }] }],
  ],
  ['service-version', false, [{ ...service, version: '01.0.0' }]],
] as const)
  cases.push({
    name,
    accept,
    patches: [...servicePatches, { path: '/services', value: definitions }],
  })
add('services-without-entry', false, '/services', [service])
add('services-empty-without-entry', true, '/services', [])
for (const [accept, refs] of [
  [true, [reference]],
  [true, [selected]],
  [false, [selected, selected]],
  [false, [{ ...reference, service: 'text-service' }]],
  [false, [{ ...reference, interfaceVersion: '*' }]],
  [false, [reference, selected]],
  [false, [{ ...selected, interfaceVersion: 'bad' }]],
] as const)
  add(
    `service-reference-${cases.length}`,
    accept,
    '/dependencies/services',
    refs
  )
for (const [accept, refs] of [
  [true, [reference]],
  [true, [tool]],
  [false, [tool, tool]],
  [false, [{ ...reference, target: tool.target }]],
  [false, [{ ...reference, buildFlavor: 'standard', digest: tool.digest }]],
  [false, [reference, tool]],
  [false, [{ ...tool, digest: 'bad' }]],
] as const)
  add(`tool-reference-${cases.length}`, accept, '/dependencies/tools', refs)
for (const schema of [
  {},
  { type: 'string', pattern: '.*' },
  { $ref: 'https://schema.invalid' },
  { type: 'object', additionalProperties: false, required: ['missing'] },
  {
    type: 'object',
    properties: { x: { type: 'string' } },
    required: ['x', 'x'],
    additionalProperties: true,
  },
  { type: 'object' },
  { type: 'array' },
  { type: 'string', items: { type: 'string' } },
  { type: 'string', minLength: 3, maxLength: 1 },
  { type: 'boolean', minimum: 0 },
  { type: 'integer', default: 1.5 },
  { type: 'boolean', enum: [true, true] },
  {
    type: 'object',
    additionalProperties: false,
    properties: { nested: { type: 'array' } },
  },
  { type: 'array', items: { type: 'string' }, default: [1] },
  { type: 'string', enum: ['a'], default: 'b' },
  { type: 'string', anyOf: [{ type: 'string', minLength: 2 }], default: 'a' },
  {
    type: 'object',
    additionalProperties: false,
    properties: { x: { type: 'string' } },
    default: { x: 1 },
  },
])
  add(
    `operation-reject-${cases.length}`,
    false,
    '/commands/0/outputSchema',
    schema
  )
for (const schema of [
  { type: 'array', items: { type: 'number' }, default: [-1, 2] },
  {
    type: 'object',
    additionalProperties: false,
    properties: { x: { type: 'string' } },
    default: { x: 'a' },
  },
  { anyOf: [{ type: 'null' }, { type: 'string' }], default: null },
  { enum: [1, 2], default: 1 },
  { const: { x: 1 }, default: { x: 1 } },
  { type: 'string', minLength: 1, maxLength: 1, default: '😀' },
])
  add(
    `operation-valid-${cases.length}`,
    true,
    '/commands/0/outputSchema',
    schema
  )
for (const target of [
  { hostVersion: '1.0.0' },
  { sdkVersion: '1.0.0' },
  { hostVersion: '0.1.1-beta.1' },
  { platform: 'macos' as const },
  { arch: 'arm64' as const },
])
  cases.push({
    name: `target-${cases.length}`,
    accept: false,
    patches: [],
    target,
  })
for (const item of cases) {
  const value = structuredClone(base) as unknown as Record<string, unknown>
  for (const patch of item.patches) {
    const keys = patch.path.slice(1).split('/')
    let parent = value
    for (const key of keys.slice(0, -1))
      parent = parent[key] as Record<string, unknown>
    const key = keys.at(-1)!
    if (patch.remove) delete parent[key]
    else parent[key] = patch.value
  }
  let accepted = false
  try {
    parsePluginManifest(value, { ...manifestTarget, ...item.target })
    accepted = true
  } catch {
    /* Rejection is the expected fixture result. */
  }
  if (accepted !== item.accept)
    throw new Error(`SDK fixture mismatch: ${item.name}`)
}
writeFileSync(
  resolve(import.meta.dirname, 'fixtures/manifest-admission-v1.json'),
  JSON.stringify(
    { formatVersion: 1, base, target: manifestTarget, cases },
    null,
    2
  ) + '\n'
)
