import { fileURLToPath } from 'node:url'

export function countRustTests(output: string): number {
  return output.split(/\r?\n/).filter(line => /^.+: test$/.test(line)).length
}

export function assertRustTestsPresent(output: string): number {
  const count = countRustTests(output)
  if (count === 0) {
    throw new Error('Desktop Rust test inventory is empty')
  }
  return count
}

if (import.meta.main) {
  const manifest = fileURLToPath(
    new URL('../src-tauri/Cargo.toml', import.meta.url)
  )
  const listing = Bun.spawn(
    [
      'cargo',
      'test',
      '--locked',
      '--lib',
      '--manifest-path',
      manifest,
      '--',
      '--list',
    ],
    { stdout: 'pipe', stderr: 'inherit' }
  )
  const output = await new Response(listing.stdout).text()
  const exitCode = await listing.exited
  if (exitCode !== 0) {
    process.exit(exitCode)
  }
  process.stdout.write(
    `Verified ${assertRustTestsPresent(output)} Rust tests.\n`
  )
}
