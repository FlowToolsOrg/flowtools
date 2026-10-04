import { resolve } from 'node:path'

// Each actual compilation owns its Bun file cache. Reusing Bun.build after
// importing SDK/React in the test process fails repeated Windows file reads.
// This is compiler lifecycle management, not isolation for plugin execution.
const [output, environment] = process.argv.slice(2)
if (!output || !environment)
  throw new Error('Mode build arguments are required')
const built = await Bun.build({
  entrypoints: [
    resolve(import.meta.dir, '../src/runtime/development-html-launch.ts'),
    resolve(
      import.meta.dir,
      '../src/runtime/development-html-plugin-bridge.ts'
    ),
  ],
  outdir: output,
  target: 'bun',
  // Bundle declared SDK/Zod workspaces, without a direct consumer dependency.
  packages: 'bundle',
  define: { 'import.meta.env': environment },
})
if (!built.success) {
  for (const log of built.logs) process.stderr.write(`${log.message}\n`)
  process.exit(1)
}
