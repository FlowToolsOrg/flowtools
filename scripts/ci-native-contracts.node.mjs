// Only the test Host changes. Keep real Cargo/Bun processes and original bounds;
// Bun child startup can block the Windows event loop before a test can proceed.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { realpathSync, writeFileSync, unlinkSync } from 'node:fs'
import { join } from 'node:path'
import { after, test } from 'node:test'
import { fileURLToPath } from 'node:url'

let completed = 0
after(() =>
  assert.equal(completed, 2, 'exactly two native contracts must complete')
)

test(
  'root and workspace lint accept defined CSS classes and reject unknown ones',
  { timeout: 30_000 },
  () => {
    const started = performance.now()
    /** @type {unknown} */
    const root = fileURLToPath(new URL('../', import.meta.url))
    assert.ok(typeof root === 'string')
    /** @type {unknown} */
    const consumer = join(root, 'apps', 'ui-test')
    assert.ok(typeof consumer === 'string')
    /** @type {unknown} */
    const fixture = join(consumer, 'src', `lint-probe-${randomUUID()}.tsx`)
    assert.ok(typeof fixture === 'string')
    /**
     * @param {string} cwd
     * @returns {{status:number,stdout:string,stderr:string}}
     */
    const runLint = cwd => {
      const remaining = Math.floor(30_000 - (performance.now() - started))
      assert.ok(remaining > 0, 'original complete CSS test bound')
      /** @type {unknown} */
      const result = spawnSync('bun', ['x', 'oxlint', fixture], {
        cwd,
        encoding: 'utf8',
        timeout: remaining,
        maxBuffer: 1_048_576,
      })
      assert.ok(result && typeof result === 'object')
      assert.ok(!('error' in result) || result.error === undefined)
      assert.ok(
        'pid' in result && typeof result.pid === 'number' && result.pid > 0,
        'actual Bun lint child'
      )
      assert.ok('signal' in result && result.signal === null)
      assert.ok('status' in result && typeof result.status === 'number')
      assert.ok('stdout' in result && typeof result.stdout === 'string')
      assert.ok('stderr' in result && typeof result.stderr === 'string')
      return {
        status: Number(result.status),
        stdout: String(result.stdout),
        stderr: String(result.stderr),
      }
    }
    try {
      for (const cwd of [root, consumer]) {
        writeFileSync(
          fixture,
          'export const probe = <div className="appearance-lab" />\n'
        )
        const allowed = runLint(cwd)
        assert.equal(allowed.status, 0, allowed.stdout + allowed.stderr)
        writeFileSync(
          fixture,
          'export const probe = <div className="appearance-lab-unknown" />\n'
        )
        const denied = runLint(cwd)
        assert.equal(denied.status, 1, denied.stdout + denied.stderr)
        assert.ok(
          (denied.stdout + denied.stderr).includes('no-unknown-classes')
        )
      }
      assert.ok(
        performance.now() - started < 30_000,
        'original complete CSS test bound'
      )
      completed++
    } finally {
      unlinkSync(fixture)
    }
  }
)

test(
  'actual Cargo resolves the workspace whose complete profile the Bun suite checks',
  { timeout: 5000 },
  () => {
    const started = performance.now()
    /** @type {import('node:child_process').SpawnSyncOptionsWithStringEncoding} */
    const options = {
      encoding: 'utf8',
      timeout: 5000,
      maxBuffer: 4 * 1024 * 1024,
    }
    /** @type {unknown} */
    const metadata = spawnSync(
      'cargo',
      [
        'metadata',
        '--locked',
        '--offline',
        '--no-deps',
        '--format-version',
        '1',
        '--manifest-path',
        fileURLToPath(new URL('../apps/runtime/Cargo.toml', import.meta.url)),
      ],
      options
    )
    assert.ok(metadata && typeof metadata === 'object')
    assert.ok('pid' in metadata && typeof metadata.pid === 'number')
    assert.ok('status' in metadata)
    assert.ok('stdout' in metadata && typeof metadata.stdout === 'string')
    assert.ok('stderr' in metadata && typeof metadata.stderr === 'string')
    assert.ok(!('error' in metadata) || metadata.error === undefined)
    assert.ok(metadata.pid > 0)
    assert.equal(metadata.status, 0, metadata.stdout + metadata.stderr)
    /** @type {unknown} */
    const workspace = JSON.parse(metadata.stdout)
    assert.ok(
      workspace &&
        typeof workspace === 'object' &&
        'workspace_root' in workspace
    )
    assert.equal(typeof workspace.workspace_root, 'string')
    assert.ok(typeof workspace.workspace_root === 'string')
    assert.equal(
      realpathSync(workspace.workspace_root),
      realpathSync(fileURLToPath(new URL('../', import.meta.url)))
    )
    assert.ok('packages' in workspace && Array.isArray(workspace.packages))
    assert.ok(
      'workspace_default_members' in workspace &&
        Array.isArray(workspace.workspace_default_members)
    )
    /** @type {unknown} */
    const probe = workspace.packages.find(
      /** @param {unknown} value */
      value =>
        value &&
        typeof value === 'object' &&
        'name' in value &&
        value.name === 'flowtools-isolation-probe'
    )
    assert.ok(probe && typeof probe === 'object' && 'id' in probe)
    assert.ok('publish' in probe && Array.isArray(probe.publish))
    assert.deepEqual(probe.publish, [])
    assert.ok('targets' in probe && Array.isArray(probe.targets))
    assert.ok(
      probe.targets.some(
        /** @param {unknown} value */
        value =>
          value &&
          typeof value === 'object' &&
          'kind' in value &&
          Array.isArray(value.kind) &&
          value.kind.includes('bin')
      )
    )
    // A member --manifest-path selects that member, while the root's default
    // set must exclude the build-only probe too. Verify both real selections.
    assert.equal(workspace.workspace_default_members.length, 1)
    assert.ok(!workspace.workspace_default_members.includes(probe.id))
    /** @type {unknown} */
    const rootMetadata = spawnSync(
      'cargo',
      [
        'metadata',
        '--locked',
        '--offline',
        '--no-deps',
        '--format-version',
        '1',
        '--manifest-path',
        fileURLToPath(new URL('../Cargo.toml', import.meta.url)),
      ],
      options
    )
    assert.ok(rootMetadata && typeof rootMetadata === 'object')
    assert.ok(
      'pid' in rootMetadata &&
        typeof rootMetadata.pid === 'number' &&
        rootMetadata.pid > 0
    )
    assert.ok(!('error' in rootMetadata) || rootMetadata.error === undefined)
    assert.ok('status' in rootMetadata)
    assert.ok(
      'stdout' in rootMetadata && typeof rootMetadata.stdout === 'string'
    )
    assert.ok(
      'stderr' in rootMetadata && typeof rootMetadata.stderr === 'string'
    )
    assert.equal(
      rootMetadata.status,
      0,
      rootMetadata.stdout + rootMetadata.stderr
    )
    /** @type {unknown} */
    const rootSelection = JSON.parse(rootMetadata.stdout)
    assert.ok(
      rootSelection &&
        typeof rootSelection === 'object' &&
        'workspace_default_members' in rootSelection &&
        Array.isArray(rootSelection.workspace_default_members)
    )
    assert.equal(rootSelection.workspace_default_members.length, 2)
    assert.ok(!rootSelection.workspace_default_members.includes(probe.id))
    /** @type {unknown} */
    const mediaMetadata = spawnSync(
      'cargo',
      [
        'metadata',
        '--locked',
        '--offline',
        '--no-deps',
        '--format-version',
        '1',
        '--manifest-path',
        fileURLToPath(
          new URL(
            '../apps/runtime/test/fixtures/media-candidate/Cargo.toml',
            import.meta.url
          )
        ),
      ],
      options
    )
    assert.ok(mediaMetadata && typeof mediaMetadata === 'object')
    assert.ok(
      'pid' in mediaMetadata &&
        typeof mediaMetadata.pid === 'number' &&
        mediaMetadata.pid > 0
    )
    assert.ok(!('error' in mediaMetadata) || mediaMetadata.error === undefined)
    assert.ok('status' in mediaMetadata && mediaMetadata.status === 0)
    assert.ok(
      'stdout' in mediaMetadata && typeof mediaMetadata.stdout === 'string'
    )
    /** @type {unknown} */
    const mediaWorkspace = JSON.parse(mediaMetadata.stdout)
    assert.ok(mediaWorkspace && typeof mediaWorkspace === 'object')
    assert.ok(
      'workspace_root' in mediaWorkspace &&
        typeof mediaWorkspace.workspace_root === 'string'
    )
    assert.equal(
      realpathSync(mediaWorkspace.workspace_root),
      realpathSync(
        fileURLToPath(
          new URL(
            '../apps/runtime/test/fixtures/media-candidate/',
            import.meta.url
          )
        )
      )
    )
    assert.ok(
      'packages' in mediaWorkspace && Array.isArray(mediaWorkspace.packages)
    )
    assert.equal(mediaWorkspace.packages.length, 1)
    /** @type {unknown} */
    const mediaPackage = mediaWorkspace.packages[0]
    assert.ok(mediaPackage && typeof mediaPackage === 'object')
    assert.ok(
      'id' in mediaPackage &&
        'name' in mediaPackage &&
        mediaPackage.name === 'flowtools-media-candidate-validation'
    )
    assert.ok('publish' in mediaPackage && Array.isArray(mediaPackage.publish))
    assert.deepEqual(mediaPackage.publish, [])
    assert.ok('targets' in mediaPackage && Array.isArray(mediaPackage.targets))
    assert.ok(
      !mediaPackage.targets.some(
        /** @param {unknown} target */
        target =>
          target &&
          typeof target === 'object' &&
          'kind' in target &&
          Array.isArray(target.kind) &&
          target.kind.includes('bin')
      )
    )
    assert.ok(
      'workspace_default_members' in mediaWorkspace &&
        Array.isArray(mediaWorkspace.workspace_default_members)
    )
    assert.deepEqual(mediaWorkspace.workspace_default_members, [
      mediaPackage.id,
    ])
    assert.ok(
      !rootSelection.workspace_default_members.includes(mediaPackage.id)
    )
    assert.ok(
      performance.now() - started < 5000,
      'original complete test bound'
    )
    completed++
  }
)
