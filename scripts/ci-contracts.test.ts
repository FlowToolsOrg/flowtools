/// <reference types="bun-types" />

import { expect, test } from 'bun:test'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
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

function psLiteral(value: string): string {
  return `'${value.replaceAll("'", "''")}'`
}

const helperPath = psLiteral(
  fileURLToPath(new URL('./ci-gates.ps1', import.meta.url))
)

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

test('PowerShell propagates native failure before subsequent gates run', () => {
  const success = runPowerShell(
    `. ${helperPath}; Invoke-QualityCommand 'fixture' bun @('-e', 'process.exit(0)')`
  )
  expect(success.error).toBeUndefined()
  if (success.status !== 0) {
    throw new Error(`PowerShell success probe failed: ${success.stderr}`)
  }
  expect(success.status).toBe(0)
  const failure = runPowerShell(
    `. ${helperPath}; Invoke-QualityCommand 'fixture' bun @('-e', 'process.exit(17)'); Write-Output 'UNEXPECTED_CONTINUATION'`
  )
  expect(failure.error).toBeUndefined()
  expect(failure.status).toBe(1)
  expect(failure.stderr).toContain('exit code 17')
  expect(failure.stdout).not.toContain('UNEXPECTED_CONTINUATION')
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
