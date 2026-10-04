import { afterEach, expect, test } from 'bun:test'
import { createHash } from 'node:crypto'
import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  realpathSync,
  rmSync,
  symlinkSync,
  readFileSync,
  writeFileSync,
} from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

import { builtInCLIManifests } from './builtin-manifests'

const packageRoot = resolve(import.meta.dir, '..')
const fixtureParent = join(packageRoot, 'node_modules/.tmp')
const fixtures: string[] = []
const builtinId = 'plugin-base64-encoder'
const sourceCanary = 'UNTRUSTED_SOURCE_EXECUTED'

function fixture(withBuiltins = false) {
  mkdirSync(fixtureParent, { recursive: true })
  const root = mkdtempSync(join(fixtureParent, 'compiled-inventory-'))
  fixtures.push(root)
  const dist = join(root, 'packages/cli/dist')
  cpSync(join(packageRoot, 'dist'), dist, { recursive: true })
  const plugins = join(root, 'plugins')
  mkdirSync(plugins)
  cpSync(
    join(packageRoot, '../../plugins/.generated'),
    join(plugins, '.generated'),
    { recursive: true }
  )
  if (withBuiltins) {
    mkdirSync(join(plugins, 'dist'))
    for (const info of builtInCLIManifests)
      copyFileSync(
        join(packageRoot, 'test-fixtures/broken-compiled.txt'),
        join(plugins, 'dist', `${info.id}.commands.js`)
      )
  }
  return {
    root,
    plugins,
    cli: join(dist, 'cli.mjs'),
    api: join(dist, 'index.mjs'),
  }
}

function addSource(plugins: string, id = builtinId) {
  const target = join(plugins, id)
  mkdirSync(target, { recursive: true })
  copyFileSync(
    join(
      packageRoot,
      'test-fixtures',
      id === 'plugin-untrusted-extra'
        ? 'extra-source-canary.txt'
        : 'source-canary.txt'
    ),
    join(target, 'index.ts')
  )
}

async function invoke(entry: string, args: string[], evaluate = false) {
  const child = Bun.spawn(
    [process.execPath, ...(evaluate ? ['--eval', entry] : [entry]), ...args],
    { stdout: 'pipe', stderr: 'pipe' }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ])
  return { stdout, stderr, code }
}

afterEach(() => {
  for (const root of fixtures.splice(0)) {
    // Delete only this test's immediate, canonical child, never the workspace.
    const canonical = realpathSync(root)
    expect(dirname(canonical)).toBe(realpathSync(fixtureParent))
    expect(canonical).toBe(root)
    rmSync(canonical, { recursive: true })
  }
})

for (const command of ['run', 'info', 'help', 'list']) {
  test(`compiled CLI ${command} fails when only a built-in source exists`, async () => {
    const sample = fixture()
    addSource(sample.plugins)
    const args =
      command === 'list'
        ? ['list', '--format', 'json']
        : command === 'help'
          ? ['run', builtinId, '--help']
          : [command, builtinId, '--format', 'json']
    const output = await invoke(sample.cli, args)
    expect(output.stdout + output.stderr).not.toContain(sourceCanary)
    expect(output.code).toBe(1)
    if (command === 'help') expect(output.stderr).toContain('[LOAD_FAILED]')
    else
      expect(JSON.parse(output.stdout)).toMatchObject({
        success: false,
        error: { code: 'LOAD_FAILED' },
      })
  }, 30_000)
}

test('compiled CLI never falls back to source after a compiled import fails', async () => {
  const sample = fixture()
  addSource(sample.plugins)
  mkdirSync(join(sample.plugins, 'dist'))
  copyFileSync(
    join(packageRoot, 'test-fixtures/broken-compiled.txt'),
    join(sample.plugins, 'dist', `${builtinId}.commands.js`)
  )
  const output = await invoke(sample.cli, [
    'run',
    builtinId,
    '--format',
    'json',
  ])
  expect(output.stdout + output.stderr).not.toContain(sourceCanary)
  expect(output.code).toBe(1)
  expect(JSON.parse(output.stdout)).toMatchObject({
    error: { code: 'LOAD_FAILED' },
  })
}, 30_000)

test('extra source and compiled entries cannot change the compiled inventory', async () => {
  const sample = fixture(true)
  addSource(sample.plugins, 'plugin-untrusted-extra')
  copyFileSync(
    join(packageRoot, 'test-fixtures/extra-source-canary.txt'),
    join(sample.plugins, 'dist/plugin-untrusted-extra.js')
  )
  const output = await invoke(sample.cli, ['list', '--format', 'json'])
  expect(output.code).toBe(0)
  const plugins = JSON.parse(output.stdout) as { id: string }[]
  expect(plugins).toHaveLength(12)
  expect(plugins.map(plugin => plugin.id)).not.toContain(
    'plugin-untrusted-extra'
  )
  const rejected = await invoke(sample.cli, [
    'run',
    'plugin-untrusted-extra',
    '--format',
    'json',
  ])
  expect(rejected.stdout + rejected.stderr).not.toContain(sourceCanary)
  expect(rejected.code).toBe(1)
  expect(JSON.parse(rejected.stdout)).toMatchObject({
    error: { code: 'PLUGIN_NOT_FOUND' },
  })
}, 30_000)

test('public compiled loader rejects unknown and path IDs before importing', async () => {
  const sample = fixture()
  addSource(sample.plugins, 'outside')
  addSource(sample.plugins, 'plugin-untrusted-extra')
  const ids = [
    '../plugins/outside',
    './outside',
    'outside',
    'plugin-untrusted-extra',
    `${builtinId}.js`,
    'PLUGIN-BASE64-ENCODER',
    'D:\\plugins\\outside',
    'file:///plugins/outside',
    '__proto__',
  ]
  const code = `import { loadPlugin } from ${JSON.stringify(pathToFileURL(sample.api).href)};
    for (const id of ${JSON.stringify(ids)}) {
      if (await loadPlugin(id) !== null) throw Error('Unknown inventory identity accepted');
    }`
  const output = await invoke(code, [], true)
  expect(output.stdout + output.stderr).not.toContain(sourceCanary)
  expect(output.code).toBe(0)
}, 30_000)

test('compiled directory junction cannot redirect imports outside the fixed artifact root', async () => {
  const sample = fixture()
  addSource(sample.plugins)
  const redirected = join(sample.root, 'outside-dist')
  mkdirSync(redirected)
  copyFileSync(
    join(packageRoot, 'test-fixtures/source-canary.txt'),
    join(redirected, `${builtinId}.commands.js`)
  )
  symlinkSync(redirected, join(sample.plugins, 'dist'), 'junction')
  const output = await invoke(sample.cli, [
    'run',
    builtinId,
    '--format',
    'json',
  ])
  expect(output.stdout + output.stderr).not.toContain(sourceCanary)
  expect(output.code).toBe(1)
  expect(JSON.parse(output.stdout)).toMatchObject({
    error: { code: 'LOAD_FAILED' },
  })
}, 30_000)

test('compiled CLI parses malformed JSON before any built-in import', async () => {
  const sample = fixture()
  addSource(sample.plugins)
  const output = await invoke(sample.cli, [
    'run',
    builtinId,
    '--input',
    '{secret-token',
    '--format',
    'json',
  ])
  expect(output.stdout + output.stderr).not.toContain(sourceCanary)
  expect(output.stdout + output.stderr).not.toContain('secret-token')
  expect(output.code).toBe(1)
  expect(JSON.parse(output.stdout)).toMatchObject({
    error: { code: 'INPUT_INVALID' },
  })
}, 30_000)

for (const mismatch of [
  'id',
  'version',
  'maturity',
  'type',
  'run',
  'schema',
  'output',
  'command',
]) {
  test(`compiled CLI rejects a built-in artifact with mismatched ${mismatch}`, async () => {
    const sample = fixture()
    cpSync(
      join(packageRoot, '../../plugins/dist'),
      join(sample.plugins, 'dist'),
      { recursive: true }
    )
    copyFileSync(
      join(packageRoot, 'test-fixtures/metadata-mismatch.txt'),
      join(sample.plugins, 'dist', `${builtinId}.commands.js`)
    )
    // Keep file integrity valid so each assertion really reaches the module contract.
    const bytes = readFileSync(
      join(sample.plugins, 'dist', `${builtinId}.commands.js`)
    )
    const catalogPath = join(
      sample.plugins,
      '.generated/builtin-manifests.json'
    )
    const catalog = JSON.parse(readFileSync(catalogPath, 'utf8')) as {
      plugins: { files: { path: string; size: number; sha256: string }[] }[]
    }
    for (const manifest of catalog.plugins) {
      const file = manifest.files.find(
        item => item.path === `${builtinId}.commands.js`
      )!
      file.size = bytes.length
      file.sha256 = createHash('sha256').update(bytes).digest('hex')
    }
    writeFileSync(catalogPath, JSON.stringify(catalog))
    const output = await invoke(sample.cli, [
      'run',
      builtinId,
      '--format',
      'json',
      mismatch,
    ])
    expect(output.code).toBe(1)
    expect(JSON.parse(output.stdout)).toMatchObject({
      success: false,
      error: { code: 'LOAD_FAILED' },
    })
    expect(output.stdout + output.stderr).not.toContain(
      'Mismatched compiled entry must not run'
    )
  }, 30_000)
}

for (const corruption of ['schema', 'digest', 'entry']) {
  test(`compiled CLI refuses ${corruption} before importing code`, async () => {
    const sample = fixture()
    cpSync(
      join(packageRoot, '../../plugins/dist'),
      join(sample.plugins, 'dist'),
      { recursive: true }
    )
    const entry = join(sample.plugins, 'dist', `${builtinId}.commands.js`)
    writeFileSync(entry, `throw new Error('${sourceCanary}')`)
    const path = join(sample.plugins, '.generated/builtin-manifests.json')
    const catalog = JSON.parse(readFileSync(path, 'utf8'))
    const manifest = catalog.plugins.find(
      (item: { id: string }) => item.id === builtinId
    )
    if (corruption === 'schema')
      manifest.commands[0].inputSchema.unknownCritical = true
    if (corruption === 'entry') manifest.entries.executor = '../outside.js'
    writeFileSync(path, JSON.stringify(catalog))
    const output = await invoke(sample.cli, [
      'run',
      builtinId,
      '--format',
      'json',
    ])
    expect(output.code).toBe(1)
    expect(JSON.parse(output.stdout)).toMatchObject({
      error: { code: 'LOAD_FAILED' },
    })
    expect(output.stdout + output.stderr).not.toContain(sourceCanary)
  }, 30_000)
}
