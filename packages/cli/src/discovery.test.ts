import type { PluginManifestV1 } from '@flowtools/sdk/manifest'

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

async function invoke(
  entry: string,
  args: string[],
  evaluate = false,
  environment: Record<string, string> = {}
) {
  const child = Bun.spawn(
    [process.execPath, ...(evaluate ? ['--eval', entry] : [entry]), ...args],
    { stdout: 'pipe', stderr: 'pipe', env: { ...process.env, ...environment } }
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
    '--text',
    'hello',
    '--format',
    'json',
  ])
  expect(output.stdout + output.stderr).not.toContain(sourceCanary)
  expect(output.code).toBe(1)
  expect(JSON.parse(output.stdout)).toMatchObject({
    error: { code: 'LOAD_FAILED' },
  })
}, 30_000)

test('compiled discovery/info/help use serialized data without importing an executor', async () => {
  const sample = fixture(true)
  for (const args of [
    ['commands', '--format', 'json'],
    ['describe', builtinId, 'run', '--format', 'json'],
    ['info', builtinId, '--format', 'json'],
    ['run', builtinId, '--help'],
  ]) {
    const output = await invoke(sample.cli, args)
    expect(output.code).toBe(0)
    expect(output.stdout + output.stderr).not.toContain(sourceCanary)
    if (args[0] === 'describe')
      expect(JSON.parse(output.stdout)).toMatchObject({
        formatVersion: 1,
        identity: `flowtools/${builtinId}/run`,
        command: {
          inputSchema: { properties: { text: { type: 'string' } } },
        },
        authorization: 'declarations-only',
      })
    if (args[0] === 'commands')
      expect(JSON.parse(output.stdout).commands).toHaveLength(12)
  }
}, 30_000)

for (const flags of [
  ['--text', 'hello', '--typo', 'secret'],
  ['--text', 'hello', 'unexpected'],
  ['--text', 'hello', '--text', 'duplicate'],
  [],
  ['--input', '{"text":"hello","undeclared":true}'],
  ['--input', '{"text":"hello"}', '--text', 'ignored'],
  ['--batch-input', '[]'],
  ['--batch-input', '[{}]'],
  ['--batch-input', '[{"text":"hello"},{"text":3}]'],
  ['--batch-input', '{private-secret'],
  ['--timeout', 'NaN', '--text', 'hello'],
]) {
  test(`compiled CLI rejects invalid preparation without code import ${JSON.stringify(flags)}`, async () => {
    const sample = fixture(true)
    const output = await invoke(sample.cli, [
      'run',
      builtinId,
      ...flags,
      '--format',
      'json',
    ])
    expect(output.code).toBe(1)
    expect(JSON.parse(output.stdout)).toMatchObject({
      success: false,
      error: {
        code: flags[0] === '--timeout' ? 'TIMEOUT_INVALID' : 'INPUT_INVALID',
      },
    })
    expect(output.stdout + output.stderr).not.toContain('private-secret')
    expect(output.stdout + output.stderr).not.toContain(sourceCanary)
  }, 30_000)
}

test('compiled CLI refuses an oversized default before importing code', async () => {
  const sample = fixture(true)
  const path = join(sample.plugins, '.generated/builtin-manifests.json')
  const catalog = JSON.parse(readFileSync(path, 'utf8')) as {
    plugins: PluginManifestV1[]
  }
  const command = catalog.plugins.find(item => item.id === builtinId)!
    .commands[0]!
  command.resources.maxInputBytes = 32
  command.inputSchema.properties!.text!.default = 'x'.repeat(64)
  writeFileSync(path, JSON.stringify(catalog))
  const output = await invoke(sample.cli, [
    'run',
    builtinId,
    '--input',
    '{}',
    '--format',
    'json',
  ])
  expect(output.code).toBe(1)
  expect(JSON.parse(output.stdout)).toMatchObject({
    success: false,
    error: { code: 'INPUT_INVALID' },
  })
  expect(output.stdout + output.stderr).not.toContain(sourceCanary)
}, 30_000)

test('compiled SDK adapter carries false, negative and array flags into runtime validation and real Base64 run', async () => {
  const sample = fixture()
  const dist = join(sample.plugins, 'dist')
  cpSync(join(packageRoot, '../../plugins/dist'), dist, { recursive: true })
  copyFileSync(
    join(dist, `${builtinId}.commands.js`),
    join(dist, 'base64-original.js')
  )
  copyFileSync(
    join(packageRoot, 'test-fixtures/command-options.txt'),
    join(dist, `${builtinId}.commands.js`)
  )
  const path = join(sample.plugins, '.generated/builtin-manifests.json')
  const catalog = JSON.parse(readFileSync(path, 'utf8')) as {
    plugins: PluginManifestV1[]
  }
  const command = catalog.plugins.find(item => item.id === builtinId)!
    .commands[0]!
  Object.assign(command.inputSchema.properties!, {
    active: { type: 'boolean', default: true },
    offset: { type: 'number', default: 0 },
    items: { type: 'array', items: { type: 'string' }, default: [] },
  })
  for (const manifest of catalog.plugins) {
    for (const name of [`${builtinId}.commands.js`, 'base64-original.js']) {
      const bytes = readFileSync(join(dist, name))
      const file = {
        path: name,
        size: bytes.length,
        sha256: createHash('sha256').update(bytes).digest('hex'),
      }
      const index = manifest.files.findIndex(item => item.path === name)
      if (index === -1) manifest.files.push(file)
      else manifest.files[index] = file
    }
  }
  writeFileSync(path, JSON.stringify(catalog))
  const executeFixture = async (args: string[]) => {
    const source = `import {loadPlugin, getBuiltinCommandManifest, createPluginRunner, createCLIToolContext, parseCommandFlags} from ${JSON.stringify(pathToFileURL(sample.api).href)};
      const args = ${JSON.stringify(args)};
      const command = getBuiltinCommandManifest('${builtinId}').commands[0];
      const batch = args[0] === '--batch-input';
      const inputs = batch ? JSON.parse(args[1]) : [parseCommandFlags(command,args.slice(0,args.indexOf('--format')))];
      const run = createPluginRunner({loadPlugin,createContext: id => createCLIToolContext(id,{pluginType:'app',permissions:[]})});
      const results = [];
      for(const input of inputs) results.push(await run('${builtinId}',input));
      const success = results.every(result => result.success);
      console.log(JSON.stringify(batch ? {formatVersion:1,type:'batch',success,results} : results[0]));
      process.exitCode = success ? 0 : 1;`
    return invoke(source, [], true)
  }
  const output = await executeFixture([
    '--text',
    'hello',
    '--active',
    'false',
    '--offset',
    '-2',
    '--items',
    'a',
    '--items',
    'b',
    '--format',
    'json',
  ])
  expect(output.code).toBe(0)
  expect(JSON.parse(output.stdout)).toMatchObject({
    success: true,
    data: { value: { result: 'aGVsbG8=' } },
  })
  const input = { text: 'hello', active: false, offset: -2, items: ['a', 'b'] }
  const batch = await executeFixture([
    '--batch-input',
    JSON.stringify([
      input,
      { ...input, active: true },
      { ...input, text: 'world' },
    ]),
  ])
  expect(batch.code).toBe(1)
  expect(JSON.parse(batch.stdout)).toMatchObject({
    formatVersion: 1,
    type: 'batch',
    success: false,
    results: [
      { success: true, data: { value: { result: 'aGVsbG8=' } } },
      { success: false, error: { code: 'EXECUTION_FAILED' } },
      { success: true, data: { value: { result: 'd29ybGQ=' } } },
    ],
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
    const output = await invoke(
      sample.cli,
      ['run', builtinId, '--format', 'json', '--text', 'hello'],
      false,
      { FLOWTOOLS_CONTRACT_MISMATCH: mismatch }
    )
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
    const catalog = JSON.parse(readFileSync(path, 'utf8')) as {
      plugins: PluginManifestV1[]
    }
    const manifest = catalog.plugins.find(
      (item: { id: string }) => item.id === builtinId
    )!
    if (corruption === 'schema')
      Object.assign(manifest.commands[0]!.inputSchema, {
        unknownCritical: true,
      })
    if (corruption === 'entry') manifest.entries.executor = '../outside.js'
    writeFileSync(path, JSON.stringify(catalog))
    const output = await invoke(sample.cli, [
      'run',
      builtinId,
      '--format',
      'json',
      '--text',
      'hello',
    ])
    expect(output.code).toBe(1)
    expect(JSON.parse(output.stdout)).toMatchObject({
      error: { code: 'LOAD_FAILED' },
    })
    expect(output.stdout + output.stderr).not.toContain(sourceCanary)
  }, 30_000)
}
