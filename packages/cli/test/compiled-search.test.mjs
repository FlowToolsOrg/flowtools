// A real Node test Host starts the actual Bun CLI. Bun->child startup can block
// the Windows event loop for ~10s; keep both original 5s test bounds here.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
} from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { after, afterEach, test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { parsePluginManifest } from '@flowtools/sdk/manifest'

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const parent = join(packageRoot, 'node_modules/.tmp')
/** @type {string[]} */
const fixtures = []
let completed = 0

function fixture() {
  mkdirSync(parent, { recursive: true })
  const root = mkdtempSync(join(parent, 'search-node-'))
  fixtures.push(root)
  const dist = join(root, 'packages/cli/dist')
  cpSync(join(packageRoot, 'dist'), dist, { recursive: true })
  const plugins = join(root, 'plugins')
  mkdirSync(join(plugins, 'dist'), { recursive: true })
  cpSync(
    join(packageRoot, '../../plugins/.generated'),
    join(plugins, '.generated'),
    {
      recursive: true,
    }
  )
  /** @type {unknown} */
  const catalog = JSON.parse(
    readFileSync(join(plugins, '.generated/builtin-manifests.json'), 'utf8')
  )
  assert.ok(
    catalog &&
      typeof catalog === 'object' &&
      'formatVersion' in catalog &&
      catalog.formatVersion === 1 &&
      'plugins' in catalog &&
      Array.isArray(catalog.plugins)
  )
  for (const entry of catalog.plugins) {
    const manifest = parsePluginManifest(entry)
    copyFileSync(
      join(packageRoot, 'test-fixtures/broken-compiled.txt'),
      join(plugins, 'dist', `${manifest.id}.commands.js`)
    )
  }
  const source = join(plugins, 'plugin-base64-encoder')
  mkdirSync(source)
  copyFileSync(
    join(packageRoot, 'test-fixtures/source-canary.txt'),
    join(source, 'index.ts')
  )
  return join(dist, 'cli.mjs')
}

/** @param {string} cli @param {string[]} args @param {AbortSignal} signal */
async function invoke(cli, args, signal) {
  // PATH is the same pinned Bun installed by the ordinary workspace gate.
  const child = spawn('bun', [cli, ...args], {
    stdio: ['ignore', 'pipe', 'pipe'],
    windowsHide: true,
    signal,
  })
  let stdout = '',
    stderr = '',
    bytes = 0,
    oversized = false
  /** @type {Error | undefined} */
  let failure
  /** @param {Buffer} chunk @param {'stdout' | 'stderr'} stream */
  const collect = (chunk, stream) => {
    bytes += chunk.length
    if (bytes > 65_536) {
      oversized = true
      child.kill()
      return
    }
    if (stream === 'stdout') stdout += chunk.toString()
    else stderr += chunk.toString()
  }
  child.stdout.on('data', chunk => collect(chunk, 'stdout'))
  child.stderr.on('data', chunk => collect(chunk, 'stderr'))
  child.on('error', error => {
    failure = error
  })
  /** @type {number | null} */
  const code = await new Promise(resolve => child.once('close', resolve))
  if (failure) throw failure
  assert.equal(oversized, false)
  assert.ok(child.pid && child.pid > 0)
  assert.equal((stdout + stderr).includes('UNTRUSTED_SOURCE_EXECUTED'), false)
  return { code, stdout }
}

afterEach(() => {
  for (const root of fixtures.splice(0)) {
    const canonical = realpathSync(root)
    assert.equal(canonical, root)
    assert.equal(dirname(canonical), realpathSync(parent))
    assert.ok(canonical.startsWith(join(realpathSync(parent), 'search-node-')))
    rmSync(canonical, { recursive: true })
  }
})
after(() =>
  assert.equal(completed, 2, 'Both actual compiled search cases must run')
)

void test(
  'compiled search handles pinyin without importing executors',
  { timeout: 5000 },
  async t => {
    const output = await invoke(fixture(), ['search', 'bdtx'], t.signal)
    assert.equal(output.code, 0)
    /** @type {unknown} */
    const result = JSON.parse(output.stdout)
    assert.ok(
      result &&
        typeof result === 'object' &&
        'success' in result &&
        'data' in result
    )
    assert.equal(result.success, true)
    assert.deepEqual(result.data, [
      { id: 'plugin-local-reminder:run', score: 300, reasons: ['initials'] },
    ])
    completed++
  }
)
void test(
  'compiled search rejects unbounded regex without importing executors',
  { timeout: 5000 },
  async t => {
    const output = await invoke(
      fixture(),
      ['search', 'a{999999999}', '--mode', 'regex'],
      t.signal
    )
    assert.equal(output.code, 1)
    /** @type {unknown} */
    const result = JSON.parse(output.stdout)
    assert.ok(
      result &&
        typeof result === 'object' &&
        'success' in result &&
        'error' in result &&
        result.error &&
        typeof result.error === 'object' &&
        'code' in result.error
    )
    assert.equal(result.success, false)
    assert.equal(result.error.code, 'INPUT_INVALID')
    completed++
  }
)
