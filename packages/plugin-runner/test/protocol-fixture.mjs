// The real Node Host sends the original rejection bytes to the actual compiled
// Bun runner. It creates fresh accepted-pin deadlines at actual submission.
// Batch in one Host: Bun->child startup can block the Windows test event loop.
import { spawn } from 'node:child_process'
import { readFileSync } from 'node:fs'

/** @type {unknown} */
const input = JSON.parse(readFileSync(0, 'utf8'))
if (!input || typeof input !== 'object') throw new Error('FIXTURE_INVALID')
const config = /** @type {Record<string, unknown>} */ (input)
if (
  typeof config.bun !== 'string' ||
  typeof config.runner !== 'string' ||
  typeof config.submissionDeadline !== 'boolean' ||
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
    const submittedAt = Date.now()
    let bytes = Buffer.from(encoded, 'base64')
    /** @type {number | undefined} */
    let deadlineAt
    if (config.submissionDeadline) {
      if (bytes.length < 5 || bytes.readUInt32LE(0) !== bytes.length - 4)
        throw new Error('FIXTURE_INVALID')
      /** @type {unknown} */
      const packet = JSON.parse(bytes.subarray(4).toString('utf8'))
      if (
        !packet ||
        typeof packet !== 'object' ||
        !('kind' in packet) ||
        packet.kind !== 'start' ||
        !('payload' in packet) ||
        !packet.payload ||
        typeof packet.payload !== 'object' ||
        !('deadline' in packet.payload) ||
        packet.payload.deadline !== 0
      )
        throw new Error('FIXTURE_INVALID')
      deadlineAt = submittedAt + 10_000
      packet.payload.deadline = deadlineAt
      const body = Buffer.from(JSON.stringify(packet))
      const header = Buffer.alloc(4)
      header.writeUInt32LE(body.length)
      bytes = Buffer.concat([header, body])
    }
    child.stdin.end(bytes)
    const code = await closed
    results.push({
      code,
      stdout,
      stderr,
      oversized,
      pid: child.pid,
      submittedAt,
      finishedAt: Date.now(),
      deadlineAt,
    })
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
