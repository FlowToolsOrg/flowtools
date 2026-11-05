import type { JsonValue } from '@flowtools/sdk/manifest'
import type { FormatConfig } from 'oxfmt'

import { readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { parseVersionedPluginManifest } from '@flowtools/sdk/manifest'
import { format } from 'oxfmt'

// Independent v2 corpus. The original 405-case v1 bytes are never rewritten.
const original = JSON.parse(
  readFileSync(new URL('./manifest-conformance.json', import.meta.url), 'utf8')
) as { cases: { manifest: Record<string, JsonValue> }[] }
const base = structuredClone(original.cases[0].manifest) as Record<
  string,
  JsonValue
>
base.formatVersion = 2
base.capabilityProfile = 'flowtools-native-v2'
const commands = base.commands as Record<string, JsonValue>[]
commands[0]!.capabilityProfile = 'flowtools-native-v2'
const cases: {
  id: string
  manifest: JsonValue
  accepted: boolean
  normalized?: JsonValue
}[] = []
function add(
  id: string,
  change: (value: Record<string, JsonValue>) => void = () => {}
) {
  const manifest = structuredClone(base)
  change(manifest)
  try {
    cases.push({
      id,
      manifest,
      accepted: true,
      normalized: parseVersionedPluginManifest(
        manifest
      ) as unknown as JsonValue,
    })
  } catch {
    cases.push({ id, manifest, accepted: false })
  }
}
add('v2-pure')
add('v2-missing-profile', value => {
  delete value.capabilityProfile
})
add('v2-unknown-profile', value => {
  value.capabilityProfile = 'other'
})
add('v2-unknown-version', value => {
  value.formatVersion = 3
})
add('v2-services', value => {
  const operation = structuredClone(
    (value.commands as Record<string, JsonValue>[])[0]!
  )
  value.entries = { executor: 'dist/commands.js', services: 'dist/services.js' }
  ;(value.files as JsonValue[]).push({
    path: 'dist/services.js',
    sha256: '1'.repeat(64),
    size: 0,
  })
  value.services = [
    { id: 'service', version: '1.0.0', operations: [operation] },
  ]
})
for (const [operation, method] of [
  ['request', 'POST'],
  ['send', 'GET'],
])
  add(`network-${operation}-method-confusion`, value => {
    const command = (value.commands as Record<string, JsonValue>[])[0]!
    command.effects = [
      operation === 'request' ? 'network-read' : 'network-send',
    ]
    command.permissions = [
      {
        capability: 'network',
        operations: [operation!],
        scopes: [
          {
            kind: 'endpoint',
            slot: 'api',
            origin: 'https://example.invalid:443',
            methods: [method!],
            pathPrefix: [],
          },
        ],
      },
    ]
  })
const definitions: readonly [string, string, string, JsonValue][] = [
  ['fs', 'read', 'file-read', { kind: 'resource', slot: 'source' }],
  ['fs', 'stat', 'file-read', { kind: 'resource', slot: 'source' }],
  ['fs', 'list', 'file-read', { kind: 'resource', slot: 'directory' }],
  ['fs', 'create', 'file-create', { kind: 'resource', slot: 'output' }],
  [
    'opener',
    'open-url',
    'opener-open-url',
    {
      kind: 'url',
      origin: 'https://example.invalid:443',
      pathPrefix: ['docs'],
    },
  ],
  [
    'opener',
    'open-resource',
    'opener-open-resource',
    { kind: 'resource', slot: 'source' },
  ],
  [
    'opener',
    'reveal-resource',
    'opener-reveal-resource',
    { kind: 'resource', slot: 'source' },
  ],
  [
    'secrets',
    'use',
    'secret-use',
    {
      kind: 'credential',
      slot: 'token',
      endpoint: 'api',
      method: 'POST',
      header: 'authorization',
    },
  ],
  [
    'secrets',
    'describe',
    'secret-use',
    {
      kind: 'credential',
      slot: 'token',
      endpoint: 'api',
      method: 'GET',
      header: 'x-api-key',
    },
  ],
  [
    'network',
    'request',
    'network-read',
    {
      kind: 'endpoint',
      slot: 'api',
      origin: 'https://example.invalid:443',
      methods: ['GET', 'HEAD'],
      pathPrefix: [],
    },
  ],
  [
    'network',
    'send',
    'network-send',
    {
      kind: 'endpoint',
      slot: 'api',
      origin: 'https://example.invalid:443',
      methods: ['POST'],
      pathPrefix: ['v1'],
    },
  ],
  ['storage', 'get', 'data-read', { kind: 'data', keyPrefix: 'todos' }],
  ['storage', 'set', 'data-write', { kind: 'data', keyPrefix: 'todos' }],
  ['clipboard', 'read', 'clipboard-read', { kind: 'clipboard' }],
  ['notification', 'notify', 'notification', { kind: 'notification' }],
  [
    'tool',
    'execute',
    'tool-execute',
    { kind: 'tool', lock: 'fixed-lock', action: 'probe' },
  ],
]
for (const [capability, operation, effect, scope] of definitions) {
  const configure = (value: Record<string, JsonValue>) => {
    const command = (value.commands as Record<string, JsonValue>[])[0]!
    command.effects = [effect]
    command.permissions = [
      { capability, operations: [operation], scopes: [scope] },
    ]
    return command
  }
  add(`${capability}-${operation}-valid`, configure)
  add(`${capability}-${operation}-unknown-operation`, value => {
    const command = configure(value)
    ;(command.permissions as Record<string, JsonValue>[])[0]!.operations = [
      'invoke',
    ]
  })
  add(`${capability}-${operation}-legacy-scope`, value => {
    const command = configure(value)
    ;(command.permissions as Record<string, JsonValue>[])[0]!.scopes = [
      'handle:source',
    ]
  })
  add(`${capability}-${operation}-duplicate-scope`, value => {
    const command = configure(value)
    ;(command.permissions as Record<string, JsonValue>[])[0]!.scopes = [
      scope,
      scope,
    ]
  })
  add(`${capability}-${operation}-unknown-scope-field`, value => {
    const command = configure(value)
    ;(command.permissions as Record<string, JsonValue>[])[0]!.scopes = [
      { ...(scope as Record<string, JsonValue>), grant: true },
    ]
  })
  add(`${capability}-${operation}-missing-effect`, value => {
    configure(value).effects = []
  })
}
for (const origin of [
  'https://localhost:443',
  'https://127.0.0.1:443',
  'https://EXAMPLE.invalid:443',
  'https://example.invalid:0443',
  'https://example.invalid:65536',
  'https://a..invalid:443',
  'https://example.invalid:443/path',
]) {
  add(`invalid-origin-${origin}`, value => {
    const command = (value.commands as Record<string, JsonValue>[])[0]!
    command.effects = ['opener-open-url']
    command.permissions = [
      {
        capability: 'opener',
        operations: ['open-url'],
        scopes: [{ kind: 'url', origin, pathPrefix: [] }],
      },
    ]
  })
}
const path = new URL('./manifest-v2-conformance.json', import.meta.url)
if (
  !cases.length ||
  new Set(cases.map(value => value.id)).size !== cases.length
)
  throw new Error('Native v2 corpus must be nonempty and distinct')
const config = JSON.parse(
  readFileSync(new URL('../../.oxfmtrc.json', import.meta.url), 'utf8')
) as FormatConfig
const formatted = await format(
  fileURLToPath(path),
  JSON.stringify({ formatVersion: 2, cases }),
  config
)
if (formatted.errors.length) throw new Error('Cannot format native v2 corpus')
const output = formatted.code.replaceAll('\r\n', '\n')
const args = process.argv.slice(2)
if (args.length > 1 || args.some(arg => !['--write', '--check'].includes(arg)))
  throw new Error('Use --check or explicit --write')
if (args[0] === '--write') writeFileSync(path, output)
else if (readFileSync(path, 'utf8') !== output)
  throw new Error('Native v2 corpus drift; inspect changes')
process.stdout.write(
  `Native v2 conformance: ${cases.length} cases; ${args[0] === '--write' ? 'written' : 'unchanged'}\n`
)
