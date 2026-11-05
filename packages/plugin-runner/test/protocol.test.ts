import { expect, test } from 'bun:test'
import { createHash } from 'node:crypto'
import {
  cpSync,
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'

import { RUNNER_PROTOCOL_VERSION } from '@flowtools/runtime-client'
import {
  parsePluginManifest,
  type PluginManifestV1,
} from '@flowtools/sdk/manifest'
import {
  manifestPackageDigest,
  verifyManifestPackage,
} from '@flowtools/sdk/manifest/package'

import { cliManifestTarget } from '../../cli/src/discovery'

const frame = (value: unknown) => {
  const body = Buffer.from(
    JSON.stringify({ version: RUNNER_PROTOCOL_VERSION, ...(value as object) })
  )
  const header = Buffer.alloc(4)
  header.writeUInt32LE(body.length)
  return Buffer.concat([header, body])
}

function runFrames(
  runner: string,
  frames: Buffer[],
  submissionDeadline = false
) {
  const host = Bun.spawnSync(
    ['node', resolve(import.meta.dirname, 'protocol-fixture.mjs')],
    {
      stdin: Buffer.from(
        JSON.stringify({
          bun: process.execPath,
          runner,
          frames: frames.map(bytes => bytes.toString('base64')),
          submissionDeadline,
        })
      ),
      stdout: 'pipe',
      stderr: 'pipe',
    }
  )
  expect(host.exitCode).toBe(0)
  expect(host.stderr.toString()).toBe('')
  const report = JSON.parse(host.stdout.toString()) as {
    timedOut: boolean
    results: {
      code: number
      stdout: string
      stderr: string
      oversized: boolean
      pid: number
      submittedAt: number
      finishedAt: number
      deadlineAt?: number
    }[]
  }
  expect(report.timedOut).toBe(false)
  expect(report.results).toHaveLength(frames.length)
  for (const result of report.results) {
    expect(result.oversized).toBe(false)
    expect(result.pid).toBeGreaterThan(0)
    if (submissionDeadline) {
      expect(result.deadlineAt).toBe(result.submittedAt + 10_000)
      expect(result.finishedAt).toBeLessThanOrEqual(result.deadlineAt!)
    }
  }
  return report.results
}

function removeServiceFixture(directory: string, parent: string) {
  const canonical = realpathSync(directory)
  if (
    canonical !== directory ||
    dirname(canonical) !== parent ||
    !basename(canonical).startsWith('flowtools-service-pin-')
  )
    throw new Error('Unexpected disposable fixture directory')
  rmSync(canonical, { recursive: true, force: true })
}

test('actual compiled runner refuses truncated, oversized and forged Host frames', async () => {
  const oversized = Buffer.alloc(4)
  oversized.writeUInt32LE(1048577)
  const inputs = [
    Buffer.from([5, 0, 0, 0, 123]),
    oversized,
    frame({ kind: 'start', payload: {}, grant: 'injected' }),
    frame({ kind: 'start', payload: {}, version: undefined }),
    frame({ kind: 'start', payload: {}, version: 1 }),
    frame({ kind: 'start', payload: {}, version: 65536 }),
    frame({
      kind: 'service',
      target: { id: 'untrusted' },
      input: { private: 'canary' },
    }),
  ]
  for (const { code, stdout, stderr } of runFrames(
    resolve(import.meta.dirname, '../dist/runner.js'),
    inputs
  )) {
    expect(code).toBe(1)
    expect(stdout).toBe('')
    expect(stderr).toContain('RUNNER_FAILED')
    expect(stderr).not.toContain('canary')
  }
}, 30_000)

test('actual compiled v2 runner executes the real pure builtin and refuses stale pins', async () => {
  const host = Bun.spawnSync(
    ['node', resolve(import.meta.dirname, 'manifest-v2-fixture.mjs')],
    {
      stdin: Buffer.from(
        JSON.stringify({ bun: process.execPath, target: cliManifestTarget })
      ),
      stdout: 'pipe',
      stderr: 'pipe',
    }
  )
  expect(host.exitCode).toBe(0)
  expect(host.stderr.toString()).toBe('')
  type Observation = {
    code: number
    stdout: string
    stdoutBase64: string
    stderr: string
    oversized: boolean
    pid: number
    submittedAt: number
    finishedAt: number
    deadlineAt: number
  }
  const report = JSON.parse(host.stdout.toString()) as {
    timedOut: boolean
    discovery: Observation
    results: Observation[]
    marker: string
  }
  expect(report.timedOut).toBe(false)
  expect(report.results).toHaveLength(3)
  for (const result of [report.discovery, ...report.results]) {
    expect(result.oversized).toBe(false)
    expect(result.pid).toBeGreaterThan(0)
  }
  for (const result of report.results) {
    expect(result.deadlineAt).toBe(result.submittedAt + 10_000)
    expect(result.finishedAt).toBeLessThanOrEqual(result.deadlineAt)
  }
  expect(report.discovery.code).toBe(0)
  expect(report.discovery.stderr).toBe('')
  expect(JSON.parse(report.discovery.stdout)).toMatchObject({
    formatVersion: 2,
    command: { capabilityProfile: 'flowtools-native-v2' },
    authorization: 'declarations-only',
  })
  const [stale, pure, denied] = report.results
  expect(stale!.code).toBe(1)
  expect(stale!.stdout).toBe('')
  expect(pure!.code).toBe(0)
  expect(pure!.stderr).toBe('')
  const output = Buffer.from(pure!.stdoutBase64, 'base64')
  expect(output.readUInt32LE(0)).toBe(output.length - 4)
  const execution = JSON.parse(output.subarray(4).toString('utf8')) as {
    version: number
    kind: string
    execution: { success: boolean; data: unknown }
  }
  expect(execution.version).toBe(RUNNER_PROTOCOL_VERSION)
  expect(execution.kind).toBe('result')
  expect(execution.execution.success).toBe(true)
  expect(JSON.stringify(execution.execution.data)).toContain('aGVsbG8=')
  expect(denied!.stderr).not.toContain(report.marker)
  const rejected = Buffer.from(denied!.stdoutBase64, 'base64')
  expect(rejected.readUInt32LE(0)).toBe(rejected.length - 4)
  expect(JSON.parse(rejected.subarray(4).toString('utf8'))).toMatchObject({
    execution: { success: false, error: { code: 'LOAD_FAILED' } },
  })
}, 30_000)

test('actual compiled runner verifies the Host package pin before importing service code', async () => {
  const parent = realpathSync(tmpdir())
  const directory = mkdtempSync(join(parent, 'flowtools-service-pin-'))
  try {
    const source = resolve(import.meta.dirname, '../../../plugins')
    const plugins = join(directory, 'plugins')
    const dist = join(plugins, 'dist')
    const generated = join(plugins, '.generated')
    const runner = join(directory, 'packages/plugin-runner/dist/runner.js')
    mkdirSync(dirname(runner), { recursive: true })
    mkdirSync(generated, { recursive: true })
    copyFileSync(resolve(import.meta.dirname, '../dist/runner.js'), runner)
    cpSync(join(source, 'dist'), dist, { recursive: true })
    const catalog = JSON.parse(
      readFileSync(join(source, '.generated/builtin-manifests.json'), 'utf8')
    ) as { formatVersion: 1; plugins: PluginManifestV1[] }
    const id = 'plugin-base64-encoder'
    const manifest = catalog.plugins.find(item => item.id === id)!
    const acceptedDigest = manifestPackageDigest(manifest)
    const entry = `${id}.services.js`
    const marker = 'COMPILED_SERVICE_IMPORT_SIDE_EFFECT'
    const bytes = Buffer.from(`console.error('${marker}'); export default {}`)
    manifest.entries.services = entry
    manifest.services = [
      {
        id: 'fixture',
        version: '1.0.0',
        operations: [structuredClone(manifest.commands[0]!)],
      },
    ]
    manifest.files.push({
      path: entry,
      size: bytes.length,
      sha256: createHash('sha256').update(bytes).digest('hex'),
    })
    writeFileSync(join(dist, entry), bytes)
    writeFileSync(
      join(generated, 'builtin-manifests.json'),
      JSON.stringify(catalog)
    )
    // The rejection must be the old Host pin, not a malformed package fixture.
    parsePluginManifest(manifest, cliManifestTarget)
    verifyManifestPackage(dist, manifest, cliManifestTarget)
    const currentDigest = manifestPackageDigest(manifest)
    expect(currentDigest).not.toBe(acceptedDigest)
    const digests = [acceptedDigest, currentDigest]
    const results = runFrames(
      runner,
      digests.map(packageDigest =>
        frame({
          kind: 'start',
          payload: {
            pluginId: id,
            commandId: 'svc:fixture:run',
            data: null,
            input: { text: 'hello' },
            packageDigest,
            deadline: 0, // The actual Node Host sets the unchanged 10s run window.
            serviceTarget: {
              publisher: 'flowtools',
              id,
              service: 'fixture',
              operation: 'run',
            },
          },
        })
      ),
      true
    )
    for (const [index, { code, stdout, stderr }] of results.entries()) {
      // The matching control imports the marker, then refuses missing handlers.
      expect(code).toBe(1)
      expect(stdout).toBe('')
      expect(stderr).toContain('RUNNER_FAILED')
      if (digests[index] === acceptedDigest)
        expect(stderr).not.toContain(marker)
      else expect(stderr).toContain(marker)
    }
  } finally {
    removeServiceFixture(directory, parent)
  }
}, 30_000)
