import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  copyFileSync,
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  writeFileSync,
} from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'

// Build-time inputs only. Runtime clients never accept an executable path.
const root = resolve(import.meta.dirname, '../..')
if (process.platform !== 'win32' || process.arch !== 'x64')
  throw new Error('Standalone bundle supports Windows x64 only')
const destination = resolve(
  root,
  'execution-validation/standalone',
  crypto.randomUUID()
)
mkdirSync(destination, { recursive: true })
const target = resolve(process.env.CARGO_TARGET_DIR ?? join(root, 'target'))
const regular = (path: string) => {
  const stat = lstatSync(path)
  if (
    !stat.isFile() ||
    stat.isSymbolicLink() ||
    realpathSync(path).toLowerCase() !== path.toLowerCase()
  )
    throw new Error('Invalid build artifact')
}
const copy = (source: string, name: string) => {
  regular(source)
  const path = join(destination, name)
  mkdirSync(dirname(path), { recursive: true })
  copyFileSync(source, path)
}
const node = spawnSync('node', ['-p', 'process.execPath'], { encoding: 'utf8' })
if (node.status !== 0) throw new Error('Build requires installed Node')
if (
  spawnSync(
    node.stdout.trim(),
    ['-p', 'process.platform + ":" + process.arch'],
    { encoding: 'utf8' }
  ).stdout.trim() !== 'win32:x64'
)
  throw new Error('Node build target mismatch')
copy(resolve(node.stdout.trim()), 'native/node.exe')
copy(process.execPath, 'native/bun.exe')
copy(
  join(root, 'packages/plugin-runner/dist/runner.js'),
  'packages/plugin-runner/dist/runner.js'
)
cpSync(join(root, 'plugins/dist'), join(destination, 'plugins/dist'), {
  recursive: true,
  dereference: false,
})
copy(
  join(root, 'plugins/.generated/builtin-manifests.json'),
  'plugins/.generated/builtin-manifests.json'
)
mkdirSync(join(destination, 'node_modules/@flowtools/sdk'), { recursive: true })
writeFileSync(
  join(destination, 'node_modules/@flowtools/sdk/package.json'),
  JSON.stringify({
    type: 'module',
    exports: { './result': './result.js', './manifest': './manifest.js' },
  })
)
for (const name of ['result', 'manifest']) {
  const build = await Bun.build({
    entrypoints: [join(root, `packages/sdk/src/${name}/index.ts`)],
    target: 'bun',
    outdir: join(destination, 'node_modules/@flowtools/sdk'),
    naming: `${name}.js`,
    external: ['zod'],
  })
  if (!build.success)
    throw new Error(
      `SDK vendor build failed: ${build.logs.map(log => log.message).join('\n')}`
    )
}
const zod = dirname(
  Bun.resolveSync('zod/package.json', join(root, 'packages/cli'))
)
cpSync(zod, join(destination, 'node_modules/zod'), {
  recursive: true,
  dereference: false,
})
const notices = join(destination, 'notices')
mkdirSync(notices)
copy(join(root, 'apps/runtime/notices/node.txt'), 'notices/node.txt')
copy(join(root, 'apps/runtime/notices/bun.txt'), 'notices/bun.txt')
copy(join(root, 'LICENSE'), 'notices/flowtools.txt')
const license = spawnSync(node.stdout.trim(), ['-p', 'process.release.name'], {
  encoding: 'utf8',
})
if (license.status !== 0 || license.stdout.trim() !== 'node')
  throw new Error('Invalid Node build')
for (const dependency of ['commander', 'zod', 'semver', 'json-schema-faker']) {
  const dir = dirname(
    Bun.resolveSync(
      `${dependency}/package.json`,
      join(root, dependency === 'semver' ? 'packages/sdk' : 'packages/cli')
    )
  )
  const file = readdirSync(dir).find(name => /^licen[sc]e(?:\.|$)/i.test(name))
  if (!file) throw new Error(`Missing license: ${dependency}`)
  copy(join(dir, file), `notices/${dependency}.txt`)
}
const nodeVersion = spawnSync(node.stdout.trim(), ['--version'], {
  encoding: 'utf8',
}).stdout.trim()
writeFileSync(
  join(notices, 'runtime.txt'),
  `Prototype Windows x64 CLI-only bundle. Node ${nodeVersion}, Bun ${Bun.version}.\nNode notice snapshot: https://github.com/nodejs/node/blob/v24.16.0/LICENSE\nBun notices: https://github.com/oven-sh/bun/blob/bun-v1.3.14/LICENSE.md\nThis bundle is local T1 integrity evidence, not a signed release or third-party sandbox.\n`
)
const artifact = (name: string) => {
  const path = join(destination, name)
  regular(path)
  return {
    path: name.replaceAll('\\', '/'),
    sha256: createHash('sha256').update(readFileSync(path)).digest('hex'),
  }
}
const files: string[] = []
function walk(dir: string) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isSymbolicLink()) throw new Error('Redirected bundle input')
    if (entry.isDirectory()) walk(path)
    else files.push(relative(destination, path).replaceAll('\\', '/'))
  }
}
walk(destination)
const buildDir = join(destination, '.build')
mkdirSync(buildDir)
writeFileSync(
  join(buildDir, 'runner.json'),
  JSON.stringify({
    bun: artifact('native/bun.exe'),
    runner: artifact('packages/plugin-runner/dist/runner.js'),
    files: files
      .filter(name => !name.startsWith('native/node'))
      .sort()
      .map(artifact),
  })
)
const cargo = (args: string[]) => {
  const result = spawnSync(
    'cargo',
    [
      ...args,
      '--target-dir',
      join(target, 'standalone'),
      '--target',
      'x86_64-pc-windows-msvc',
      '--locked',
      '--manifest-path',
      join(root, 'apps/runtime/Cargo.toml'),
    ],
    {
      env: {
        ...process.env,
        FLOWTOOLS_BUNDLE_BUILD_DIR: buildDir,
        RUSTFLAGS: '-C target-feature=+crt-static',
        CARGO_ENCODED_RUSTFLAGS: undefined,
      },
      encoding: 'utf8',
      maxBuffer: 4194304,
    }
  )
  if (result.status !== 0)
    throw new Error(`Native bundle build failed: ${result.stderr}`)
}
cargo(['build', '--bin', 'flowtools-runtime', '--features', 'standalone'])
copy(
  join(target, 'standalone/x86_64-pc-windows-msvc/debug/flowtools-runtime.exe'),
  'native/flowtools-runtime.exe'
)
const runtime = artifact('native/flowtools-runtime.exe')
const built = await Bun.build({
  entrypoints: [join(root, 'packages/cli/src/cli.ts')],
  target: 'node',
  outdir: join(destination, 'packages/cli/dist'),
  naming: 'cli.mjs',
  define: {
    __FLOWTOOLS_BUNDLE_RUNTIME_LOCK__: JSON.stringify(runtime),
    __FLOWTOOLS_NATIVE_BUILD_PATH__: 'null',
  },
})
if (!built.success)
  throw new Error(
    `CLI build failed: ${built.logs.map(log => log.message).join('\n')}`
  )
writeFileSync(
  join(buildDir, 'launcher.json'),
  JSON.stringify({
    node: artifact('native/node.exe'),
    cli: artifact('packages/cli/dist/cli.mjs'),
  })
)
cargo(['build', '--bin', 'flowtools', '--features', 'bundle-launcher'])
copy(
  join(target, 'standalone/x86_64-pc-windows-msvc/debug/flowtools.exe'),
  'flowtools.exe'
)
// Every pinned file is listed for review; .build contains no credentials.
writeFileSync(
  join(destination, 'bundle.json'),
  JSON.stringify(
    {
      formatVersion: 1,
      platform: 'windows-x64',
      maturity: 'prototype',
      runtime,
      launcher: artifact('flowtools.exe'),
    },
    null,
    2
  )
)
if (!existsSync(join(destination, 'flowtools.exe')))
  throw new Error('Missing launcher')
process.stdout.write(destination + '\n')
