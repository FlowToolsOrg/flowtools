import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { join, resolve } from 'node:path'

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
    // Await process and both pipe EOFs before the next actual CLI command.
    child.once(
      'close',
      /** @param {number | null} code */ code => resolve(code)
    )
  })
  const code = await exited
  assert.equal(code, 0, `CLI stage: ${args[0]}`)
  assert.equal(stderr, '', `CLI stderr stage: ${args[0]}`)
  return stdout
}

const output =
  /** @type {{success: boolean, pluginId: string, pluginVersion: string, data: {type: string, value: {result: string}}}} */ (
    /** @type {unknown} */ (
      JSON.parse(
        await invoke([
          'run',
          'plugin-base64-encoder',
          '--text',
          'hello',
          '--format',
          'json',
        ])
      )
    )
  )
assert.equal(output.success, true)
assert.equal(output.pluginId, 'plugin-base64-encoder')
assert.equal(output.pluginVersion, '0.1.0')
assert.equal(output.data.type, 'json')
assert.equal(output.data.value.result, 'aGVsbG8=')

const jobs = await invoke(['jobs', 'list'])
const rows =
  /** @type {{data: {runId: string, pluginId: string}[]}} */ (
    /** @type {unknown} */ (JSON.parse(jobs))
  ).data
const run = rows.find(job => job.pluginId === 'plugin-base64-encoder')
assert.ok(run)
const calls =
  /** @type {{success: boolean, data: unknown[]}} */ (
    /** @type {unknown} */ (
      JSON.parse(
        await invoke(['dependencies', 'calls', run.runId, '--format', 'json'])
      )
    )
  )
assert.equal(calls.success, true)
assert.deepEqual(calls.data, [])
assert.equal(await invoke(['jobs', 'list']), jobs)
process.stdout.write('Run envelope CLI fixture passed\n')
