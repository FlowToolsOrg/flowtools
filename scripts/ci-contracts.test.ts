/// <reference types="bun-types" />

import { expect, test } from 'bun:test'
import { spawnSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
  unlinkSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

interface WorkflowStep {
  uses?: string
  run?: string
  if?: string
  env?: Record<string, string>
  with?: { path?: string; 'persist-credentials'?: boolean }
  'continue-on-error'?: boolean
}

interface Workflow {
  on: Record<string, unknown>
  permissions: Record<string, string>
  jobs: {
    quality: {
      name: string
      'runs-on': string
      'timeout-minutes': number
      defaults: { run: { shell: string } }
      env: Record<string, string>
      steps: WorkflowStep[]
      'continue-on-error'?: boolean
    }
  }
}

const workflow = Bun.YAML.parse(
  await Bun.file(
    new URL('../.github/workflows/windows-quality.yml', import.meta.url)
  ).text()
) as Workflow

test('native integrity hashing is optimized without weakening debug checks', () => {
  // The mandatory Node companion verifies that Cargo actually resolves this
  // workspace. Keep full TOML semantics and the exact profile assertion here.
  const manifest = Bun.TOML.parse(
    readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8')
  ) as { profile: Record<string, unknown> }
  expect(manifest.profile).toEqual({
    dev: { package: { sha2: { 'opt-level': 3 } } },
  })
})

test('Desktop requires the native Node companion and strictly checks its JavaScript', () => {
  const desktop = JSON.parse(
    readFileSync(
      new URL('../apps/desktop/package.json', import.meta.url),
      'utf8'
    )
  ) as { scripts: Record<string, string> }
  expect(desktop.scripts.test).toBe(
    'bun test test ../../scripts && node --test ../../scripts/ci-native-contracts.node.mjs && bun run test/rust-test-inventory.ts && cargo test --locked --features codegen --lib --test bindings --manifest-path src-tauri/Cargo.toml'
  )
  expect(desktop.scripts['check-types']).toBe(
    'tsc --noEmit && tsc --noEmit -p tsconfig.test.json && tsc --noEmit -p ../../scripts/tsconfig.node.json'
  )
  expect(desktop.scripts.lint).toContain(
    '&& bun run --cwd ../.. lint:ci-native'
  )
  const root = JSON.parse(
    readFileSync(new URL('../package.json', import.meta.url), 'utf8')
  ) as { scripts: Record<string, string> }
  expect(root.scripts['lint:ci-native']).toBe(
    'oxlint --type-aware --type-check --tsconfig scripts/tsconfig.node.json scripts/ci-native-contracts.node.mjs'
  )
  const config = JSON.parse(
    readFileSync(new URL('./tsconfig.node.json', import.meta.url), 'utf8')
  ) as { compilerOptions: Record<string, unknown>; include: string[] }
  expect(config.compilerOptions).toMatchObject({
    strict: true,
    allowJs: true,
    checkJs: true,
    noEmit: true,
    types: ['node'],
  })
  expect(config.include).toEqual(['ci-native-contracts.node.mjs'])
})

test('root and workspace lint accept defined CSS classes and reject unknown ones', () => {
  const root = fileURLToPath(new URL('../', import.meta.url))
  const consumer = join(root, 'apps', 'ui-test')
  const fixture = join(consumer, 'src', `lint-probe-${randomUUID()}.tsx`)
  try {
    for (const cwd of [root, consumer]) {
      writeFileSync(
        fixture,
        'export const probe = <div className="appearance-lab" />\n'
      )
      const allowed = spawnSync(process.execPath, ['x', 'oxlint', fixture], {
        cwd,
        encoding: 'utf8',
      })
      expect(allowed.status, allowed.stdout + allowed.stderr).toBe(0)
      writeFileSync(
        fixture,
        'export const probe = <div className="appearance-lab-unknown" />\n'
      )
      const denied = spawnSync(process.execPath, ['x', 'oxlint', fixture], {
        cwd,
        encoding: 'utf8',
      })
      expect(denied.status, denied.stdout + denied.stderr).toBe(1)
      expect(denied.stdout + denied.stderr).toContain('no-unknown-classes')
    }
  } finally {
    unlinkSync(fixture)
  }
}, 30_000)

function psLiteral(value: string): string {
  return `'${value.replaceAll("'", "''")}'`
}

const helperPath = psLiteral(
  fileURLToPath(new URL('./ci-gates.ps1', import.meta.url))
)

test('Core isolates full native cases without filtering tests or changing job deadlines', () => {
  const core = JSON.parse(
    readFileSync(
      new URL('../packages/runtime-core/package.json', import.meta.url),
      'utf8'
    )
  ) as { scripts: { test: string } }
  expect(core.scripts.test.split(' && ')).toEqual([
    'bun run manifest-conformance.ts --check',
    'bun run generate-catalog.ts',
    'bun run ../../scripts/check-rust-tests.ts Cargo.toml',
    'cargo test --locked --manifest-path Cargo.toml -- --test-threads=1',
  ])
  const scheduling = readFileSync(
    new URL(
      '../packages/runtime-core/src/runtime_scheduling.rs',
      import.meta.url
    ),
    'utf8'
  )
  expect(scheduling).toContain('timeout_ms: 1_000,')
  expect(scheduling).toContain(
    'fn notification_needs_current_policy_and_durable_claim_before_os_effect()'
  )
  expect(scheduling).toContain(
    'fn scheduled_acceptance_uses_private_inputs_and_survives_client_disconnect_without_replay()'
  )
})

function runPowerShell(command: string, cwd?: string) {
  return spawnSync(
    'pwsh',
    ['-NoProfile', '-NonInteractive', '-Command', command],
    {
      cwd,
      encoding: 'utf8',
    }
  )
}

test('uses unprivileged PR events and immutable action references', () => {
  expect('pull_request' in workflow.on).toBe(true)
  expect('workflow_dispatch' in workflow.on).toBe(true)
  expect('pull_request_target' in workflow.on).toBe(false)
  expect(workflow.permissions).toEqual({ contents: 'read' })
  expect(workflow.jobs.quality['continue-on-error']).toBeUndefined()
  for (const step of workflow.jobs.quality.steps) {
    expect(step['continue-on-error']).toBeUndefined()
    if (step.uses) {
      expect(step.uses).toMatch(/^[\w-]+\/[\w-]+@[a-f0-9]{40}$/)
    }
  }
  expect(workflow.jobs.quality.steps[0]?.with?.['persist-credentials']).toBe(
    false
  )
})

test('uses runner-dependent Bun cache paths only in step contexts', () => {
  const job = workflow.jobs.quality
  expect(job.env.BUN_INSTALL_CACHE_DIR).toBeUndefined()
  for (const value of Object.values(job.env)) {
    expect(value).not.toMatch(/\$\{\{[^}]*\b(runner|env|steps|job)\./)
  }
  const cachePath = '${{ runner.temp }}/flowtools-bun-cache'
  expect(job.steps.some(step => step.with?.path === cachePath)).toBe(true)
  const gates = job.steps.find(step => step.run === './scripts/check-ci.ps1')
  expect(gates?.env?.BUN_INSTALL_CACHE_DIR).toBe(cachePath)
})

test('runs bounded Windows gates without caching JS build products', async () => {
  const turbo = (await Bun.file(
    new URL('../turbo.json', import.meta.url)
  ).json()) as {
    globalPassThroughEnv: string[]
    tasks: Record<
      'test' | 'build' | 'lint' | 'check-types',
      { passThroughEnv: string[]; outputs: string[] }
    >
  }
  expect(turbo.globalPassThroughEnv).toEqual(['PATHEXT'])
  expect(turbo.tasks.test.passThroughEnv).toEqual(['CARGO_TARGET_DIR'])
  expect(turbo.tasks.build.passThroughEnv).toEqual(['CARGO_TARGET_DIR'])
  expect(turbo.tasks.lint.passThroughEnv).toEqual(['CARGO_TARGET_DIR'])
  expect(turbo.tasks['check-types'].passThroughEnv).toEqual([
    'CARGO_TARGET_DIR',
  ])
  expect(turbo.tasks.build.outputs).toEqual(['dist/**/*', '.generated/**/*'])
  const job = workflow.jobs.quality
  expect(job.name).toBe('Windows quality gates')
  expect(job['runs-on']).toBe('windows-2022')
  expect(job['timeout-minutes']).toBeGreaterThan(0)
  expect(job.defaults.run.shell).toBe('pwsh')
  expect(job.steps.some(step => step.run === './scripts/check-ci.ps1')).toBe(
    true
  )
  expect(job.steps.at(-1)?.if).toBe('always()')
  for (const step of job.steps.filter(item => item.with?.path)) {
    expect(step.with?.path).not.toMatch(/node_modules|\.turbo|\bdist\b/)
  }
  const rawWorkflow = await Bun.file(
    new URL('../.github/workflows/windows-quality.yml', import.meta.url)
  ).text()
  expect(rawWorkflow).toContain(
    "hashFiles('Cargo.lock', 'apps/desktop/src-tauri/Cargo.lock')"
  )
  const rawTurbo = (await Bun.file(
    new URL('../turbo.json', import.meta.url)
  ).json()) as { tasks: Record<string, { cache?: boolean }> }
  expect(rawTurbo['tasks']['@flowtools/runtime#build'].cache).toBe(false)
  expect(rawTurbo['tasks']['@flowtools/runtime-core#build'].cache).toBe(false)
  const entry = await Bun.file(
    new URL('./check-ci.ps1', import.meta.url)
  ).text()
  expect(entry.indexOf("'build:packages'")).toBeLessThan(
    entry.indexOf("'lint'")
  )
  expect(entry.indexOf("'generate:hosts'")).toBeLessThan(
    entry.indexOf("'lint'")
  )
  expect(entry.indexOf("'test'")).toBeLessThan(
    entry.indexOf("'build', '--force'")
  )
  expect(entry).toContain("'install', '--frozen-lockfile'")
  expect(entry).toContain("'run', 'docs:check'")
  expect(entry).toContain("'run', 'verify:plugin-catalog'")
  expect(entry).toContain("'run', 'verify:runtime-contracts'")
  expect(entry.indexOf("'generate:hosts'")).toBeLessThan(
    entry.indexOf("'verify:runtime-contracts'")
  )
  expect(entry.indexOf("'verify:runtime-contracts'")).toBeLessThan(
    entry.indexOf("'lint'")
  )
  expect(entry.indexOf("'verify:plugin-catalog'")).toBeLessThan(
    entry.indexOf("'lint'")
  )
  expect(entry.indexOf("'docs:check'")).toBeLessThan(entry.indexOf("'lint'"))
  expect(entry).toContain("'check', '--locked'")
})

test('clean bootstrap builds the declared Runtime client used by the native harness', async () => {
  const root = (await Bun.file(
    new URL('../package.json', import.meta.url)
  ).json()) as { scripts: Record<string, string> }
  const consumer = (await Bun.file(
    new URL('../apps/ui-test/package.json', import.meta.url)
  ).json()) as { devDependencies: Record<string, string> }
  const harness = await Bun.file(
    new URL(
      '../apps/ui-test/scripts/validate-runtime-hosts.ts',
      import.meta.url
    )
  ).text()
  expect(root.scripts['build:packages']).toContain(
    '--filter=@flowtools/runtime-client'
  )
  expect(consumer.devDependencies['@flowtools/runtime-client']).toBe(
    'workspace:*'
  )
  expect(harness).toContain("from '@flowtools/runtime-client'")
  expect(harness).toContain("from '@flowtools/runtime-client/node'")
  expect(harness).not.toContain('runtime-client/dist')
})

test('uncached native builds retain their actual Turbo prerequisite graph', () => {
  const rootPackage = JSON.parse(
    readFileSync(new URL('../package.json', import.meta.url), 'utf8')
  ) as { scripts: Record<string, string> }
  const plan = spawnSync(
    process.execPath,
    [
      'x',
      '--no-install',
      ...rootPackage.scripts['build:packages']!.split(' '),
      '--dry=json',
    ],
    {
      cwd: fileURLToPath(new URL('../', import.meta.url)),
      encoding: 'utf8',
      maxBuffer: 4 * 1024 * 1024,
    }
  )
  if (plan.status !== 0) {
    throw new Error(`Turbo build plan failed: ${plan.stderr}`)
  }
  const tasks = (
    JSON.parse(plan.stdout) as {
      tasks: {
        taskId: string
        dependencies: string[]
        resolvedTaskDefinition: {
          cache: boolean
          passThroughEnv: string[] | null
        }
      }[]
    }
  ).tasks
  const dependencies = (taskId: string) =>
    tasks.find(task => task.taskId === taskId)?.dependencies
  for (const taskId of [
    '@flowtools/runtime#build',
    '@flowtools/runtime-core#build',
  ]) {
    const task = tasks.find(task => task.taskId === taskId)
    expect(task?.resolvedTaskDefinition.cache).toBe(false)
    expect(task?.resolvedTaskDefinition.passThroughEnv).toEqual([
      'CARGO_TARGET_DIR',
    ])
  }
  expect(dependencies('@flowtools/runtime#build')).toEqual(
    expect.arrayContaining([
      '@flowtools/plugin-runner#build',
      '@flowtools/runtime-core#build',
      '@flowtools/runtime-client#build',
    ])
  )
  expect(dependencies('@flowtools/runtime-core#build')).toEqual(
    expect.arrayContaining(['@flowtools/plugins#build', '@flowtools/sdk#build'])
  )
  expect(dependencies('@flowtools/cli#build')).toEqual(
    expect.arrayContaining(['@flowtools/runtime-client#build'])
  )
  expect(
    tasks.find(task => task.taskId === '@flowtools/cli#build')
      ?.resolvedTaskDefinition.passThroughEnv
  ).toEqual(['CARGO_TARGET_DIR'])
  expect(dependencies('@flowtools/runtime-client#build')).not.toContain(
    '@flowtools/runtime#build'
  )
}, 30_000)

test('Runtime codegen stays clean after Windows CRLF checkout and rejects content drift', () => {
  const fixture = mkdtempSync(join(tmpdir(), 'flowtools-ci-generated-'))
  if (dirname(resolve(fixture)) !== resolve(tmpdir())) {
    throw new Error('Fixture cleanup target escaped temporary directory')
  }
  const attributes = readFileSync(
    new URL('../.gitattributes', import.meta.url),
    'utf8'
  )
  const files = ['bindings.ts', 'wire-schema.json', 'wire-fixtures.json'].map(
    name => `packages/runtime-client/src/${name}`
  )
  const runGit = (args: string[]) => {
    const result = spawnSync('git', args, { cwd: fixture, encoding: 'utf8' })
    if (result.status !== 0) {
      throw new Error(`Git fixture failed: ${result.stderr}`)
    }
  }
  try {
    runGit(['init', '--quiet'])
    runGit(['config', 'core.autocrlf', 'true'])
    writeFileSync(join(fixture, '.gitattributes'), attributes)
    for (const file of files) {
      mkdirSync(dirname(join(fixture, file)), { recursive: true })
      writeFileSync(join(fixture, file), 'generated\nfixture\n')
    }
    runGit(['add', '.'])
    runGit([
      '-c',
      'user.name=CI fixture',
      '-c',
      'user.email=ci@fixture.invalid',
      'commit',
      '--quiet',
      '-m',
      'fixture',
    ])
    runGit(['checkout-index', '--force', '--all'])
    for (const file of files) {
      expect(readFileSync(join(fixture, file), 'utf8')).toBe(
        'generated\nfixture\n'
      )
      // The Rust generator writes LF regardless of the checkout platform.
      writeFileSync(join(fixture, file), 'generated\nfixture\n')
    }
    const clean = runPowerShell(
      `. ${helperPath}; Assert-CleanWorktree`,
      fixture
    )
    expect(clean.status).toBe(0)
    writeFileSync(join(fixture, files[0]!), 'changed\nfixture\n')
    const dirty = runPowerShell(
      `. ${helperPath}; Assert-CleanWorktree`,
      fixture
    )
    expect(dirty.status).toBe(1)
    expect(dirty.stdout).toContain(files[0]!)
  } finally {
    rmSync(fixture, { recursive: true, force: true })
  }
}, 30_000)

test('Manifest conformance gate refuses drift without writes and requires explicit regeneration', () => {
  const root = fileURLToPath(new URL('../', import.meta.url))
  const fixtureParent = resolve(root, 'packages/runtime-core/.generated')
  const fixture = resolve(
    fixtureParent,
    `g5-manifest-conformance-${randomUUID()}`
  )
  if (dirname(fixture) !== fixtureParent) {
    throw new Error('Fixture cleanup target escaped generated directory')
  }
  const runtime = join(fixture, 'packages', 'runtime-core')
  const source = join(root, 'packages', 'runtime-core')
  const generator = join(runtime, 'manifest-conformance.ts')
  const corpus = join(runtime, 'manifest-conformance.json')
  const dependencies = join(runtime, 'node_modules')
  let linkedDependencies = false
  const expected = readFileSync(join(source, 'manifest-conformance.json'))
  const manifest = JSON.parse(
    readFileSync(join(source, 'package.json'), 'utf8')
  ) as { scripts: Record<string, string> }
  for (const task of ['generate', 'verify:bindings', 'test']) {
    expect(manifest.scripts[task]).toMatch(
      /^bun run manifest-conformance\.ts --check && /
    )
  }
  const run = (args: string[] = []) =>
    spawnSync(process.execPath, [generator, ...args], {
      cwd: runtime,
      encoding: 'utf8',
      env: {
        PATH: process.env.PATH,
        SystemRoot: process.env.SystemRoot,
        TEMP: process.env.TEMP,
        TMP: process.env.TMP,
      },
    })
  try {
    mkdirSync(runtime, { recursive: true })
    symlinkSync(join(source, 'node_modules'), dependencies, 'junction')
    linkedDependencies = true
    writeFileSync(
      generator,
      readFileSync(join(source, 'manifest-conformance.ts'))
    )
    writeFileSync(corpus, expected)
    writeFileSync(
      join(fixture, '.oxfmtrc.json'),
      readFileSync(join(root, '.oxfmtrc.json'))
    )
    for (const args of [[], ['--check']]) {
      const clean = run(args)
      expect(clean.error).toBeUndefined()
      expect(clean.status, clean.stdout + clean.stderr).toBe(0)
      expect(clean.stdout).toContain('unchanged')
      expect(readFileSync(corpus).equals(expected)).toBe(true)
    }
    const changed = JSON.parse(expected.toString('utf8')) as {
      cases: { normalized?: { name: string } }[]
    }
    const first = changed.cases[0]?.normalized
    expect(first).toBeDefined()
    first!.name = 'CI controlled conformance drift'
    const drift = Buffer.from(JSON.stringify(changed))
    writeFileSync(corpus, drift)
    const denied = run(['--check'])
    expect(denied.error).toBeUndefined()
    expect(denied.status, denied.stdout + denied.stderr).not.toBe(0)
    expect(denied.stderr).toContain('SDK Manifest conformance drift')
    expect(readFileSync(corpus).equals(drift)).toBe(true)
    const regenerated = run(['--write'])
    expect(regenerated.error).toBeUndefined()
    expect(regenerated.status, regenerated.stdout + regenerated.stderr).toBe(0)
    expect(regenerated.stdout).toContain('written')
    expect(readFileSync(corpus, 'utf8')).toBe(
      expected.toString('utf8').replaceAll('\r\n', '\n')
    )
  } finally {
    if (linkedDependencies) unlinkSync(dependencies)
    rmSync(fixture, { recursive: true, force: true })
  }
}, 30_000)

test('browser gate bounds file concurrency without replacing or narrowing the runner', async () => {
  const config = await Bun.file(
    new URL('../apps/ui-test/vitest.browser.config.ts', import.meta.url)
  ).text()
  const app = (await Bun.file(
    new URL('../apps/ui-test/package.json', import.meta.url)
  ).json()) as {
    scripts: Record<string, string>
    devDependencies: Record<string, string>
  }
  expect(config).toMatch(/fileParallelism:\s*false/)
  expect(config).toContain('provider: playwright()')
  expect(config).toContain("instances: [{ browser: 'chromium' }]")
  expect(config).not.toMatch(/(?:exclude|retry|passWithNoTests)\s*:/)
  expect(app.scripts.test).toBe('vitest run --config vitest.browser.config.ts')
  expect(app.devDependencies.playwright).toBe('1.59.1')
})

test('browser API uses the actual OS-selected IPv4 loopback port with strict binding', () => {
  const config = new URL(
    '../apps/ui-test/vitest.browser.config.ts',
    import.meta.url
  )
  const probe = spawnSync(
    process.execPath,
    [
      '-e',
      `import assert from 'node:assert/strict'
import { Server, createServer } from 'node:net'
const selected = []
const original = Server.prototype.listen
Server.prototype.listen = function (...args) {
  if (args[0] === 0 && args[1] === '127.0.0.1') {
    this.once('listening', () => selected.push(this.address().port))
  }
  return original.apply(this, args)
}
const exported = (await import(${JSON.stringify(config.href)})).default
const config = await exported({ command: 'serve', mode: 'test' })
const api = config.test.browser.api
assert.equal(selected.length, 1)
assert.equal(api.port, selected[0])
assert.ok(api.port > 0)
assert.equal(api.host, '127.0.0.1')
assert.equal(api.strictPort, true)
const rebound = createServer()
await new Promise((resolve, reject) => {
  rebound.once('error', reject)
  rebound.listen({ port: api.port, host: api.host, exclusive: true }, resolve)
})
await new Promise((resolve, reject) => rebound.close(error => error ? reject(error) : resolve()))
process.stdout.write('OS-selected browser API port verified\\n')`,
    ],
    {
      cwd: fileURLToPath(new URL('../apps/ui-test', import.meta.url)),
      encoding: 'utf8',
    }
  )
  expect(probe.error).toBeUndefined()
  expect(probe.status, probe.stdout + probe.stderr).toBe(0)
  expect(probe.stdout.trim()).toBe('OS-selected browser API port verified')
}, 30_000)

test('PowerShell propagates native failure before subsequent gates run', () => {
  for (const host of ['bun', 'node']) {
    const success = runPowerShell(
      `. ${helperPath}; Invoke-QualityCommand 'fixture' ${host} @('-e', 'process.exit(0)')`
    )
    expect(success.error).toBeUndefined()
    if (success.status !== 0) {
      throw new Error(`PowerShell success probe failed: ${success.stderr}`)
    }
    expect(success.status).toBe(0)
    const failure = runPowerShell(
      `. ${helperPath}; Invoke-QualityCommand 'fixture' ${host} @('-e', 'process.exit(17)'); Write-Output 'UNEXPECTED_CONTINUATION'`
    )
    expect(failure.error).toBeUndefined()
    expect(failure.status).toBe(1)
    expect(failure.stderr).toContain('exit code 17')
    expect(failure.stdout).not.toContain('UNEXPECTED_CONTINUATION')
  }
}, 30_000)

test('production artifact refusal is a fatal post-build gate before the clean-worktree check', async () => {
  const entry = await Bun.file(
    new URL('./check-ci.ps1', import.meta.url)
  ).text()
  const gate = entry.indexOf("'verify:production-entrypoints'")
  expect(entry).toContain(
    "Invoke-QualityCommand 'Production external-entrypoint artifacts' bun @("
  )
  expect(gate).toBeGreaterThan(entry.indexOf("'build', '--force'"))
  expect(gate).toBeLessThan(entry.lastIndexOf('Assert-CleanWorktree'))
  const root = (await Bun.file(
    new URL('../package.json', import.meta.url)
  ).json()) as {
    scripts: Record<string, string>
  }
  expect(root.scripts['verify:production-entrypoints']).toBe(
    'bun run scripts/verify-production-entrypoints.ts'
  )
})

test('worktree guard accepts clean repositories and rejects untracked drift', () => {
  const fixture = mkdtempSync(join(tmpdir(), 'flowtools-ci-contract-'))
  try {
    const init = spawnSync('git', ['init', '--quiet'], { cwd: fixture })
    expect(init.status).toBe(0)
    const clean = runPowerShell(
      `. ${helperPath}; Assert-CleanWorktree`,
      fixture
    )
    if (clean.status !== 0) {
      throw new Error(`Clean fixture probe failed: ${clean.stderr}`)
    }
    expect(clean.status).toBe(0)
    writeFileSync(join(fixture, 'generated.txt'), 'fixture')
    const dirty = runPowerShell(
      `. ${helperPath}; Assert-CleanWorktree`,
      fixture
    )
    expect(dirty.status).toBe(1)
    expect(dirty.stdout).toContain('generated.txt')
    expect(dirty.stderr).toContain('tracked or untracked repository files')
  } finally {
    rmSync(fixture, { recursive: true, force: true })
  }
}, 30_000)
