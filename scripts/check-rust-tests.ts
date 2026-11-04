/// <reference types="bun-types" />
import { resolve } from 'node:path'

const manifest = resolve(process.argv[2] ?? 'Cargo.toml')
const expected = process.argv[3]
if (expected !== undefined && !/^[1-9]\d{0,5}$/.test(expected)) {
  throw new Error('Invalid expected Rust test count')
}
const child = Bun.spawn(
  ['cargo', 'test', '--locked', '--manifest-path', manifest, '--', '--list'],
  { stdout: 'pipe', stderr: 'inherit' }
)
const output = await new Response(child.stdout).text()
const code = await child.exited
if (code !== 0) process.exit(code)
const count = output
  .split(/\r?\n/)
  .filter(line => /^.+: test$/.test(line)).length
if (count === 0) throw new Error('Rust test inventory is empty')
if (expected !== undefined && count !== Number(expected)) {
  throw new Error(
    `Rust test inventory mismatch: expected ${expected}, collected ${count}`
  )
}
process.stdout.write(`Verified ${count} Rust tests.\n`)
