import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { resolve, join } from 'node:path'

const root = resolve(import.meta.dirname, '../../..')
const cli = join(root, 'packages/cli/dist/cli.mjs')
const profile = process.argv[2]
assert.ok(profile)

/** @param {string[]} args */
async function invoke(args) {
  const child = spawn(process.execPath, [cli, ...args, '--profile', profile], {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  let stdout = '',
    stderr = ''
  child.stdout.on('data', bytes => {
    stdout += bytes
  })
  child.stderr.on('data', bytes => {
    stderr += bytes
  })
  /** @type {Promise<number | null>} */
  const exited = new Promise((resolve, reject) => {
    child.once('error', reject)
    // Close follows both stdio EOFs, so snapshots include the complete output.
    child.once(
      'close',
      /** @param {number | null} code */ code => resolve(code)
    )
  })
  const code = await exited
  assert.equal(code, 0)
  assert.equal(stderr, '')
  return stdout
}

await invoke(['runtime', 'start'])
const permissions = await invoke(['permissions', 'list'])
const jobs = await invoke(['jobs', 'list'])
const args = [
  'dependencies',
  'plan',
  'plugin-base64-encoder',
  '--format',
  'json',
]
const first = await invoke(args)
const second = await invoke(args)
assert.equal(second, first)
/** @type {{ success: boolean, data: import('@flowtools/runtime-client').DependencyPlan }} */
const plan =
  /** @type {{ success: boolean, data: import('@flowtools/runtime-client').DependencyPlan }} */ (
    /** @type {unknown} */ (JSON.parse(first))
  )
assert.equal(plan.success, true)
assert.equal(plan.data.formatVersion, 1)
assert.equal(plan.data.mode, 'plan-only')
assert.deepEqual(plan.data.lock.roots, [
  { publisher: 'flowtools', id: 'plugin-base64-encoder' },
])
assert.match(plan.data.lock.digest, /^[a-f0-9]{64}$/)
assert.equal(plan.data.lock.packages.length, 1)
assert.equal(await invoke(['permissions', 'list']), permissions)
assert.equal(await invoke(['jobs', 'list']), jobs)

const text = await invoke(['dependencies', 'plan', 'plugin-base64-encoder'])
assert.ok(text.includes(plan.data.lock.digest))
assert.ok(text.includes('plan-only'))
assert.ok(text.includes('does not install, activate or grant'))
/** @type {{ success: boolean, data: import('@flowtools/runtime-client').ProviderChangePlan }} */
const unload =
  /** @type {{ success: boolean, data: import('@flowtools/runtime-client').ProviderChangePlan }} */ (
    /** @type {unknown} */ (
      JSON.parse(
        await invoke([
          'dependencies',
          'unload-plan',
          'plugin-base64-encoder',
          '--format',
          'json',
        ])
      )
    )
  )
assert.equal(unload.success, true)
assert.equal(unload.data.mode, 'plan-only')
assert.equal(unload.data.provider.id, 'plugin-base64-encoder')
assert.equal(unload.data.replacement, null)
assert.deepEqual(unload.data.consumers, [])
assert.equal(await invoke(['jobs', 'list']), jobs)
assert.equal(await invoke(['permissions', 'list']), permissions)
process.stdout.write('Dependency plan CLI fixture passed\n')
