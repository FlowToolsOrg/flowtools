// The real Node Host sends every original byte sequence to the actual compiled
// Bun runner. Batch in one Host: repeated Bun->child process startup can block
// the Windows test event loop. No retries, substituted handlers or relaxed gate.
import { spawn } from 'node:child_process'
import { readFileSync } from 'node:fs'

/** @type {unknown} */
const input = JSON.parse(readFileSync(0, 'utf8'))
if (!input || typeof input !== 'object') throw new Error('FIXTURE_INVALID')
const config = /** @type {Record<string, unknown>} */ (input)
if (
  typeof config.bun !== 'string' ||
  typeof config.runner !== 'string' ||
  !Array.isArray(config.frames) ||
  config.frames.length > 8 ||
  config.frames.some(frame => typeof frame !== 'string' || frame.length > 8192)
)
  throw new Error('FIXTURE_INVALID')
const frames = /** @type {string[]} */ (config.frames)
/** @type {import('node:child_process').ChildProcessWithoutNullStreams | undefined} */
let active
let timedOut = false
const timeout = setTimeout(() => {
  timedOut = true
  active?.kill()
}, 25_000)
const results = []
try {
  for (const encoded of frames) {
    if (timedOut) break
    const child = spawn(config.bun, [config.runner], {
      stdio: ['pipe', 'pipe', 'pipe'],
    })
    active = child
    let stdout = '',
      stderr = '',
      oversized = false
    child.stdout.on('data', data => {
      stdout += String(data)
      if (stdout.length > 65536) {
        oversized = true
        child.kill()
      }
    })
    child.stderr.on('data', data => {
      stderr += String(data)
      if (stderr.length > 65536) {
        oversized = true
        child.kill()
      }
    })
    /** @type {Promise<number | null>} */
    const closed = new Promise((resolve, reject) => {
      child.once('error', reject)
      child.once('close', resolve)
    })
    child.stdin.end(Buffer.from(encoded, 'base64'))
    const code = await closed
    results.push({ code, stdout, stderr, oversized, pid: child.pid })
    active = undefined
  }
  process.stdout.write(JSON.stringify({ timedOut, results }))
} finally {
  clearTimeout(timeout)
  if (active && active.exitCode === null) {
    active.kill()
    await new Promise(resolve => active?.once('close', resolve))
  }
}
