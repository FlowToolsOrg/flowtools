import { resolve } from 'node:path'

import {
  artifactDigests,
  assertProductionArtifacts,
  readProductionArtifacts,
} from './production-artifacts'

const repository = resolve(import.meta.dir, '..')
const hosts = ['apps/web-vite', 'apps/desktop'] as const
const probes = {
  VITE_HTML_PLUGIN_ROOT: 'X:/flowtools-release-gate-checkout-canary',
  VITE_PLUGIN_PREVIEW_URL: 'https://flowtools-release-gate-preview.invalid/',
  // Synthetic probes override any real key in the child; never print/persist it.
  TAURI_PRIVATE_KEY: 'flowtools-release-gate-private-key-canary',
  TAURI_SIGNING_PRIVATE_KEY: 'flowtools-release-gate-signing-key-canary',
}
const canaries = Object.values(probes)

export async function verifyProductionEntrypoints(): Promise<void> {
  for (const host of hosts) {
    const directory = resolve(repository, host, 'dist')
    const baseline = readProductionArtifacts(directory)
    assertProductionArtifacts(baseline, canaries)
    const expected = JSON.stringify(artifactDigests(baseline))
    // Explicit child-only overrides, never a global env mutation or Turbo loose mode.
    // Vite must omit unsafe modules even with opt-in=1 in a production build.
    const build = Bun.spawn([process.execPath, 'run', '--cwd', host, 'build'], {
      cwd: repository,
      env: {
        ...process.env,
        ...probes,
        VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW: '1',
      },
      stdout: 'inherit',
      stderr: 'inherit',
    })
    if ((await build.exited) !== 0)
      throw new Error('Production opt-in build failed')
    const rebuilt = readProductionArtifacts(directory)
    assertProductionArtifacts(rebuilt, canaries)
    if (JSON.stringify(artifactDigests(rebuilt)) !== expected)
      throw new Error('Production opt-in changed actual artifacts')
    process.stdout.write(
      `${host}: ${rebuilt.length} production artifacts identical; known unsafe code, certification switches and canaries absent.\n`
    )
  }
}

if (import.meta.main) {
  if (process.argv.length !== 2)
    throw new Error('Production gate takes no arguments')
  await verifyProductionEntrypoints()
  process.stdout.write(
    'Production entrypoint gate passed; not signing, isolation or security certification.\n'
  )
}
