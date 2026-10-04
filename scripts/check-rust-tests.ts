/// <reference types="bun-types" />
import { resolve } from 'node:path'

const manifest = resolve(process.argv[2] ?? 'Cargo.toml')
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
process.stdout.write(`Verified ${count} Rust tests.\n`)
