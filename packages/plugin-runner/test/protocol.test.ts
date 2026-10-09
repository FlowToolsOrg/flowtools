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
  const body = Buffer.from(JSON.stringify(value))
  const header = Buffer.alloc(4)
  header.writeUInt32LE(body.length)
  return Buffer.concat([header, body])
}

test('actual compiled runner refuses truncated, oversized and forged Host frames', async () => {
  const oversized = Buffer.alloc(4)
  oversized.writeUInt32LE(1048577)
  for (const input of [
    Buffer.from([5, 0, 0, 0, 123]),
    oversized,
    frame({ kind: 'start', payload: {}, grant: 'injected' }),
    frame({
      kind: 'service',
      target: { id: 'untrusted' },
      input: { private: 'canary' },
    }),
  ]) {
    const child = Bun.spawn(
      [process.execPath, resolve(import.meta.dirname, '../dist/runner.js')],
      { stdin: 'pipe', stdout: 'pipe', stderr: 'pipe' }
    )
    await child.stdin.write(input)
    await child.stdin.end()
    const [code, stdout, stderr] = await Promise.all([
      child.exited,
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
    ])
    expect(code).toBe(1)
    expect(stdout).toBe('')
    expect(stderr).toContain('RUNNER_FAILED')
    expect(stderr).not.toContain('canary')
  }
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
    for (const packageDigest of [acceptedDigest, currentDigest]) {
      const child = Bun.spawn([process.execPath, runner], {
        stdin: 'pipe',
        stdout: 'pipe',
        stderr: 'pipe',
      })
      await child.stdin.write(
        frame({
          kind: 'start',
          payload: {
            pluginId: id,
            commandId: 'svc:fixture:run',
            data: null,
            input: { text: 'hello' },
            packageDigest,
            deadline: Date.now() + 10_000,
            serviceTarget: {
              publisher: 'flowtools',
              id,
              service: 'fixture',
              operation: 'run',
            },
          },
        })
      )
      await child.stdin.end()
      const [code, stdout, stderr] = await Promise.all([
        child.exited,
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
      ])
      // The matching control imports the marker, then refuses missing handlers.
      expect(code).toBe(1)
      expect(stdout).toBe('')
      expect(stderr).toContain('RUNNER_FAILED')
      if (packageDigest === acceptedDigest) expect(stderr).not.toContain(marker)
      else expect(stderr).toContain(marker)
    }
  } finally {
    const canonical = realpathSync(directory)
    if (
      canonical !== directory ||
      dirname(canonical) !== parent ||
      !basename(canonical).startsWith('flowtools-service-pin-')
    )
      throw new Error('Unexpected disposable fixture directory')
    rmSync(canonical, { recursive: true, force: true })
  }
}, 30_000)
