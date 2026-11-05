// One Node Host owns the whole actual compiled v2 scenario. Bun child startup
// must not consume a fresh Host startup for each phase on Windows.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { basename, dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { RUNNER_PROTOCOL_VERSION } from '@flowtools/runtime-client'
import { parseVersionedPluginManifest } from '@flowtools/sdk/manifest'
import {
  manifestPackageDigest,
  verifyManifestPackage,
} from '@flowtools/sdk/manifest/package'

/** @type {unknown} */
const input = JSON.parse(readFileSync(0, 'utf8'))
if (!input || typeof input !== 'object') throw new Error('FIXTURE_INVALID')
const config = /** @type {Record<string, unknown>} */ (input)
if (typeof config.bun !== 'string' || !config.target)
  throw new Error('FIXTURE_INVALID')
const bun = config.bun
const target =
  /** @type {import('@flowtools/sdk/manifest').ManifestTarget} */ (
    config.target
  )
const here = dirname(fileURLToPath(import.meta.url))
const fixtureParent = resolve(here, '../../cli/node_modules/.tmp')
mkdirSync(fixtureParent, { recursive: true })
const parent = realpathSync(fixtureParent)
const directory = mkdtempSync(join(parent, 'flowtools-service-pin-'))
function removeFixture() {
  const canonical = realpathSync(directory)
  if (
    canonical !== directory ||
    dirname(canonical) !== parent ||
    !basename(canonical).startsWith('flowtools-service-pin-')
  )
    throw new Error('Unexpected disposable fixture directory')
  rmSync(canonical, { recursive: true, force: true })
}
/** @type {import('node:child_process').ChildProcessWithoutNullStreams | undefined} */
let active
let timedOut = false
const timeout = setTimeout(() => {
  timedOut = true
  active?.kill()
}, 25_000)

/**
 * @param {string[]} args
 * @param {Record<string, unknown>} [payload]
 */
async function run(args, payload) {
  if (timedOut) throw new Error('FIXTURE_TIMEOUT')
  const child = spawn(bun, args, { stdio: ['pipe', 'pipe', 'pipe'] })
  active = child
  /** @type {Buffer[]} */
  const output = []
  let stdoutBytes = 0,
    stderr = '',
    oversized = false
  child.stdout.on('data', bytes => {
    stdoutBytes += bytes.length
    if (stdoutBytes <= 65536) output.push(Buffer.from(bytes))
    else {
      oversized = true
      child.kill()
    }
  })
  child.stderr.on('data', bytes => {
    stderr += String(bytes)
    if (stderr.length > 65536) {
      oversized = true
      child.kill()
    }
  })
  /** @type {Promise<number | null>} */
  const closed = new Promise((resolve, reject) => {
    child.once('error', reject)
    child.once('close', resolve)
  })
  const submittedAt = Date.now()
  const deadlineAt = payload ? submittedAt + 10_000 : undefined
  if (payload) {
    const body = Buffer.from(
      JSON.stringify({
        version: RUNNER_PROTOCOL_VERSION,
        kind: 'start',
        payload: { ...payload, deadline: deadlineAt },
      })
    )
    const header = Buffer.alloc(4)
    header.writeUInt32LE(body.length)
    child.stdin.end(Buffer.concat([header, body]))
  } else child.stdin.end()
  const code = await closed
  active = undefined
  const bytes = Buffer.concat(output)
  assert.equal(timedOut, false)
  assert.equal(oversized, false)
  return {
    code,
    stdout: bytes.toString('utf8'),
    stdoutBase64: bytes.toString('base64'),
    stderr,
    oversized,
    pid: child.pid,
    submittedAt,
    deadlineAt,
    finishedAt: Date.now(),
  }
}

try {
  const source = resolve(here, '../../../plugins')
  const dist = join(directory, 'plugins/dist')
  const generated = join(directory, 'plugins/.generated')
  const runner = join(directory, 'packages/plugin-runner/dist/runner.js')
  mkdirSync(dirname(runner), { recursive: true })
  mkdirSync(generated, { recursive: true })
  copyFileSync(resolve(here, '../dist/runner.js'), runner)
  cpSync(join(source, 'dist'), dist, { recursive: true })
  const catalog =
    /** @type {{formatVersion: 1, plugins: import('@flowtools/sdk/manifest').PluginManifest[]}} */ (
      JSON.parse(
        readFileSync(join(source, '.generated/builtin-manifests.json'), 'utf8')
      )
    )
  const index = catalog.plugins.findIndex(
    item => item.id === 'plugin-base64-encoder'
  )
  const original = catalog.plugins[index]
  assert.ok(original)
  const oldDigest = manifestPackageDigest(original)
  const manifest = parseVersionedPluginManifest(
    {
      ...original,
      formatVersion: 2,
      capabilityProfile: 'flowtools-native-v2',
      commands: original.commands.map(command => ({
        ...command,
        capabilityProfile: 'flowtools-native-v2',
        effects: [],
        permissions: [],
      })),
    },
    target
  )
  const wrapper = join(directory, 'v2-wrapper.ts')
  writeFileSync(
    wrapper,
    `import original from ${JSON.stringify(join(source, 'dist/plugin-base64-encoder.commands.js'))};export default {...original,command:${JSON.stringify(manifest.commands[0])}}`
  )
  const entry = 'plugin-base64-encoder.commands.js'
  const compiled = await run([
    'build',
    wrapper,
    '--target',
    'bun',
    '--outfile',
    join(dist, entry),
  ])
  assert.equal(compiled.code, 0)
  const bytes = readFileSync(join(dist, entry))
  const file = manifest.files.find(file => file.path === entry)
  assert.ok(file)
  file.size = bytes.length
  file.sha256 = createHash('sha256').update(bytes).digest('hex')
  catalog.plugins[index] = manifest
  writeFileSync(
    join(generated, 'builtin-manifests.json'),
    JSON.stringify(catalog)
  )
  verifyManifestPackage(dist, manifest, target)
  // Use the actual production CLI and all its chunks, without rebuilding it.
  const cliDist = join(directory, 'packages/cli/dist')
  cpSync(resolve(here, '../../cli/dist'), cliDist, { recursive: true })
  const discovery = await run([
    join(cliDist, 'cli.mjs'),
    'describe',
    manifest.id,
    'run',
    '--format',
    'json',
  ])
  const digest = manifestPackageDigest(manifest)
  const payload = {
    pluginId: manifest.id,
    commandId: 'run',
    input: { text: 'hello', mode: 'encode' },
    data: null,
  }
  const stale = await run([runner], { ...payload, packageDigest: oldDigest })
  const pure = await run([runner], { ...payload, packageDigest: digest })
  const marker = 'V2_NATIVE_IMPORT_MUST_NOT_RUN'
  const marked = Buffer.concat([
    Buffer.from(`console.error('${marker}');`),
    bytes,
  ])
  writeFileSync(join(dist, entry), marked)
  const unavailable = parseVersionedPluginManifest(
    {
      ...manifest,
      commands: manifest.commands.map(command => ({
        ...command,
        effects: ['file-read'],
        permissions: [
          {
            capability: 'fs',
            operations: ['read'],
            scopes: [{ kind: 'resource', slot: 'source' }],
          },
        ],
      })),
    },
    target
  )
  const markedFile = unavailable.files.find(file => file.path === entry)
  assert.ok(markedFile)
  markedFile.size = marked.length
  markedFile.sha256 = createHash('sha256').update(marked).digest('hex')
  catalog.plugins[index] = unavailable
  writeFileSync(
    join(generated, 'builtin-manifests.json'),
    JSON.stringify(catalog)
  )
  verifyManifestPackage(dist, unavailable, target)
  const denied = await run([runner], {
    ...payload,
    packageDigest: manifestPackageDigest(unavailable),
  })
  process.stdout.write(
    JSON.stringify({
      timedOut,
      discovery,
      results: [stale, pure, denied],
      marker,
    })
  )
} finally {
  clearTimeout(timeout)
  if (active && active.exitCode === null) {
    active.kill()
    await new Promise(resolve => active?.once('close', resolve))
  }
  removeFixture()
}
