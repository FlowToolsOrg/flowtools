// Only the test Host changes. Keep the real Cargo process and original 5s bound;
// Bun child startup can block the Windows event loop before a test can proceed.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { realpathSync } from 'node:fs'
import { after, test } from 'node:test'
import { fileURLToPath } from 'node:url'

let completed = 0
after(() =>
  assert.equal(completed, 1, 'exactly one native contract must complete')
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
    assert.ok(
      performance.now() - started < 5000,
      'original complete test bound'
    )
    completed++
  }
)
