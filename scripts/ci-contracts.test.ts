/// <reference types="bun-types" />

import { expect, test } from 'bun:test'
import { spawnSync } from 'node:child_process'
import { createHash, randomUUID } from 'node:crypto'
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
  unlinkSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

interface WorkflowStep {
  uses?: string
  run?: string
  if?: string
  env?: Record<string, string>
  with?: {
    path?: string
    'persist-credentials'?: boolean
    'fetch-depth'?: number
  }
  'continue-on-error'?: boolean
}

interface Workflow {
  on: Record<string, unknown>
  permissions: Record<string, string>
  jobs: {
    quality: {
      name: string
      'runs-on': string
      'timeout-minutes': number
      defaults: { run: { shell: string } }
      env: Record<string, string>
      steps: WorkflowStep[]
      'continue-on-error'?: boolean
    }
  }
}

const workflow = Bun.YAML.parse(
  await Bun.file(
    new URL('../.github/workflows/windows-quality.yml', import.meta.url)
  ).text()
) as Workflow

test('native integrity hashing is optimized without weakening debug checks', () => {
  // The mandatory Node companion verifies that Cargo actually resolves this
  // workspace. Keep full TOML semantics and the exact profile assertion here.
  const manifest = Bun.TOML.parse(
    readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8')
  ) as { profile: Record<string, unknown> }
  expect(manifest.profile).toEqual({
    dev: {
      package: {
        sha2: { 'opt-level': 3 },
        'ed25519-dalek': {
          'opt-level': 3,
          'debug-assertions': true,
          'overflow-checks': true,
        },
        'curve25519-dalek': {
          'opt-level': 3,
          'debug-assertions': true,
          'overflow-checks': true,
        },
      },
    },
  })
  for (const relative of [
    '../apps/desktop/src-tauri/Cargo.toml',
    '../apps/runtime/test/fixtures/managed-media-candidate/Cargo.toml',
  ]) {
    const other = Bun.TOML.parse(
      readFileSync(new URL(relative, import.meta.url), 'utf8')
    ) as { profile: Record<string, unknown> }
    const packageOverrides = {
      'ed25519-dalek': {
        'opt-level': 3,
        'debug-assertions': true,
        'overflow-checks': true,
      },
      'curve25519-dalek': {
        'opt-level': 3,
        'debug-assertions': true,
        'overflow-checks': true,
      },
    }
    expect(other.profile).toEqual({
      dev: {
        package: relative.includes('managed-media')
          ? { sha2: { 'opt-level': 3 }, ...packageOverrides }
          : packageOverrides,
      },
    })
  }
})

test('Desktop requires the native Node companion and strictly checks its JavaScript', () => {
  const desktop = JSON.parse(
    readFileSync(
      new URL('../apps/desktop/package.json', import.meta.url),
      'utf8'
    )
  ) as { scripts: Record<string, string> }
  expect(desktop.scripts.test).toBe(
    'bun test test ../../scripts && node --test ../../scripts/ci-native-contracts.node.mjs && bun run test/rust-test-inventory.ts && cargo test --locked --features codegen --lib --test bindings --manifest-path src-tauri/Cargo.toml'
  )
  expect(desktop.scripts['check-types']).toBe(
    'tsc --noEmit && tsc --noEmit -p tsconfig.test.json && tsc --noEmit -p ../../scripts/tsconfig.node.json'
  )
  expect(desktop.scripts.lint).toContain(
    '&& bun run --cwd ../.. lint:ci-native'
  )
  const root = JSON.parse(
    readFileSync(new URL('../package.json', import.meta.url), 'utf8')
  ) as { scripts: Record<string, string> }
  expect(root.scripts['lint:ci-native']).toBe(
    'oxlint --type-aware --type-check --tsconfig scripts/tsconfig.node.json scripts/ci-native-contracts.node.mjs'
  )
  const config = JSON.parse(
    readFileSync(new URL('./tsconfig.node.json', import.meta.url), 'utf8')
  ) as { compilerOptions: Record<string, unknown>; include: string[] }
  expect(config.compilerOptions).toMatchObject({
    strict: true,
    allowJs: true,
    checkJs: true,
    noEmit: true,
    types: ['node'],
  })
  expect(config.include).toEqual(['ci-native-contracts.node.mjs'])
})

test('Node companion retains all actual CSS lint assertions and the original total deadline', () => {
  const source = readFileSync(
    new URL('./ci-native-contracts.node.mjs', import.meta.url),
    'utf8'
  )
  expect(source).toContain('exactly two native contracts must complete')
  expect(source).toContain("spawnSync('bun', ['x', 'oxlint', fixture]")
  expect(source).toContain('for (const cwd of [root, consumer])')
  expect(source).toContain('appearance-lab-unknown')
  expect(source).toContain('assert.equal(allowed.status, 0,')
  expect(source).toContain('assert.equal(denied.status, 1,')
  expect(source).toContain('no-unknown-classes')
  expect(source).toContain('timeout: 30_000')
  expect(source).toContain('timeout: remaining')
  expect(source).toContain('performance.now() - started < 30_000')
  expect(source).toContain('unlinkSync(fixture)')
})

function psLiteral(value: string): string {
  return `'${value.replaceAll("'", "''")}'`
}

const helperPath = psLiteral(
  fileURLToPath(new URL('./ci-gates.ps1', import.meta.url))
)

test('Core isolates full native cases without filtering tests or changing job deadlines', () => {
  const core = JSON.parse(
    readFileSync(
      new URL('../packages/runtime-core/package.json', import.meta.url),
      'utf8'
    )
  ) as { scripts: { test: string } }
  expect(core.scripts.test.split(' && ')).toEqual([
    'bun run manifest-conformance.ts --check',
    'bun run generate-catalog.ts',
    'cargo build --locked --manifest-path ../../apps/runtime/test/fixtures/isolation-probe/Cargo.toml',
    'bun run ../../scripts/check-rust-tests.ts Cargo.toml',
    'cargo test --locked --manifest-path Cargo.toml -- --test-threads=1',
  ])
  const scheduling = readFileSync(
    new URL(
      '../packages/runtime-core/src/runtime_scheduling.rs',
      import.meta.url
    ),
    'utf8'
  )
  expect(scheduling).toContain('timeout_ms: 1_000,')
  expect(scheduling).toContain(
    'fn notification_needs_current_policy_and_durable_claim_before_os_effect()'
  )
  expect(scheduling).toContain(
    'fn scheduled_acceptance_uses_private_inputs_and_survives_client_disconnect_without_replay()'
  )
})

test('real media candidate stays an explicit pinned developer-only acceptance workspace', () => {
  const root = fileURLToPath(new URL('../', import.meta.url))
  const candidate = join(root, 'apps/runtime/test/fixtures/media-candidate')
  const manifest = Bun.TOML.parse(
    readFileSync(join(candidate, 'Cargo.toml'), 'utf8')
  ) as {
    package: { publish: boolean; autobins: boolean }
    workspace: object
  }
  expect(manifest.package.publish).toBe(false)
  expect(manifest.package.autobins).toBe(false)
  expect(manifest.workspace).toEqual({})
  const runtime = JSON.parse(
    readFileSync(join(root, 'apps/runtime/package.json'), 'utf8')
  ) as {
    scripts: Record<string, string>
  }
  expect(runtime.scripts['validate:media-candidate']).toBe(
    'pwsh -NoProfile -File ../../scripts/test-media-candidate.ps1'
  )
  for (const name of ['build', 'test'])
    expect(runtime.scripts[name]).not.toContain('media-candidate')
  const source = readFileSync(join(candidate, 'acceptance.rs'), 'utf8')
  expect(source.match(/#\[test\]/g)).toHaveLength(6)
  expect(source).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(source).toContain('include_bytes!(".generated/bin/ffmpeg.exe")')
  expect(source).toContain('checked_image(FFMPEG')
  const sourceLock = readFileSync(
    join(root, 'scripts/media-candidate/source-lock.json')
  )
  const lock = JSON.parse(
    readFileSync(join(candidate, 'candidate-lock.json'), 'utf8')
  ) as {
    maturity: string
    sourceLockSha256: string
    programs: Record<string, string>
  }
  expect(lock.maturity).toBe('prototype')
  expect(lock.sourceLockSha256).toBe(
    createHash('sha256').update(sourceLock).digest('hex')
  )
  expect(Object.keys(lock.programs).sort()).toEqual(['ffmpeg', 'ffprobe'])
  for (const hash of Object.values(lock.programs))
    expect(hash).toMatch(/^[0-9a-f]{64}$/)
  const build = readFileSync(
    join(root, 'scripts/media-candidate/build.sh'),
    'utf8'
  )
  expect(build).toContain('--disable-autodetect --disable-network')
  expect(build).toContain('--enable-bsf=vp9_superframe')
  expect(build).not.toMatch(
    /--enable-(?:gpl|nonfree|network)|curl[^\n]*\|\s*(?:sh|bash)/
  )
  const parseLock = (path: string) =>
    Bun.TOML.parse(readFileSync(path, 'utf8')) as {
      package: Array<{
        name: string
        version: string
        source?: string
        checksum?: string
      }>
    }
  const known = new Set(
    parseLock(join(root, 'Cargo.lock'))
      .package.filter(p => p.source)
      .map(p => `${p.name}@${p.version}:${p.source}:${p.checksum}`)
  )
  for (const pkg of parseLock(join(candidate, 'Cargo.lock')).package.filter(
    p => p.source
  )) {
    expect(
      known.has(`${pkg.name}@${pkg.version}:${pkg.source}:${pkg.checksum}`)
    ).toBe(true)
  }
  const driver = readFileSync(
    join(root, 'scripts/test-media-candidate.ps1'),
    'utf8'
  )
  expect(driver).toContain(
    "Invoke-QualityCommand 'Media candidate test inventory'"
  )
  expect(driver).toContain("$mediaManifest, '6'")
  expect(driver).toContain(
    "Invoke-QualityCommand 'Media candidate all-target Clippy'"
  )
  expect(driver).toContain("Invoke-QualityCommand 'Six real-media LPAC checks'")
})

test('signed installed-media candidate uses actual Core test sources in a separate pinned workspace', () => {
  const root = fileURLToPath(new URL('../', import.meta.url))
  const fixture = join(
    root,
    'apps/runtime/test/fixtures/managed-media-candidate'
  )
  const read = (path: string) => readFileSync(join(root, path), 'utf8')
  type Dependency =
    | string
    | {
        version?: string
        workspace?: boolean
        features?: string[]
        [key: string]: unknown
      }
  type Cargo = {
    package?: { publish?: boolean; autobins?: boolean }
    workspace?: {
      members?: string[]
      dependencies?: Record<string, Dependency>
    }
    lib?: { path: string; doctest: boolean }
    dependencies: Record<string, Dependency>
    target?: Record<
      string,
      {
        dependencies?: Record<string, Dependency>
        'dev-dependencies'?: Record<string, Dependency>
      }
    >
  }
  const manifest = Bun.TOML.parse(
    readFileSync(join(fixture, 'Cargo.toml'), 'utf8')
  ) as Cargo
  const core = Bun.TOML.parse(read('packages/runtime-core/Cargo.toml')) as Cargo
  const workspace = Bun.TOML.parse(read('Cargo.toml')) as Cargo
  expect(manifest.package?.publish).toBe(false)
  expect(manifest.package?.autobins).toBe(false)
  expect(manifest.workspace).toEqual({})
  expect(manifest.lib).toEqual({ path: 'validation.rs', doctest: false })
  expect(
    readFileSync(join(fixture, '.gitignore'), 'utf8').trim().split(/\r?\n/)
  ).toEqual(['/.generated/', '/target/'])
  expect(workspace.workspace?.members).not.toContain(
    'apps/runtime/test/fixtures/managed-media-candidate'
  )
  const normalize = (dependency: Dependency) =>
    typeof dependency === 'string' ? { version: dependency } : dependency
  expect(Object.keys(manifest.dependencies).sort()).toEqual(
    Object.keys(core.dependencies).sort()
  )
  for (const [name, value] of Object.entries(core.dependencies)) {
    const local = normalize(value)
    const expected = local.workspace
      ? {
          ...normalize(workspace.workspace!.dependencies![name]!),
          ...Object.fromEntries(
            Object.entries(local).filter(([key]) => key !== 'workspace')
          ),
        }
      : local
    expect(normalize(manifest.dependencies[name]!)).toEqual(expected)
  }
  const windows = 'cfg(windows)'
  const mergedFeatures = new Set([
    ...normalize(core.target![windows]!.dependencies!['windows-sys']!)
      .features!,
    ...normalize(core.target![windows]!['dev-dependencies']!['windows-sys']!)
      .features!,
  ])
  const actualWindows = normalize(
    manifest.target![windows]!.dependencies!['windows-sys']!
  )
  expect(actualWindows.version).toBe('=0.61.2')
  expect([...actualWindows.features!].sort()).toEqual(
    [...mergedFeatures].sort()
  )
  const parseLock = (path: string) =>
    Bun.TOML.parse(readFileSync(path, 'utf8')) as {
      package: Array<{
        name: string
        version: string
        source?: string
        checksum?: string
      }>
    }
  const trustedDependencies = new Set(
    parseLock(join(root, 'Cargo.lock'))
      .package.filter(p => p.source)
      .map(p => `${p.name}@${p.version}:${p.source}:${p.checksum}`)
  )
  for (const pkg of parseLock(join(fixture, 'Cargo.lock')).package.filter(
    p => p.source
  )) {
    expect(
      trustedDependencies.has(
        `${pkg.name}@${pkg.version}:${pkg.source}:${pkg.checksum}`
      )
    ).toBe(true)
  }
  const source = readFileSync(join(fixture, 'validation.rs'), 'utf8')
  expect(source).toContain(
    'include!("../../../../../packages/runtime-core/src/lib.rs");'
  )
  expect(source).toContain('#[cfg(test)]')
  const tests = readFileSync(join(fixture, 'managed_media.rs'), 'utf8')
  expect(tests.match(/#\[test\]/g)).toHaveLength(6)
  expect(tests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(tests).toContain(
    'include_bytes!("../media-candidate/.generated/bin/ffmpeg.exe")'
  )
  expect(tests).toContain('invoke_media_tool')
  expect(tests).toContain(
    'use crate::packages::tools::native_fixture as harness;'
  )
  expect(tests).toContain('launch_installed_media')
  expect(tests).toMatch(/child\s*\.admitted_image/)
  const harness = read('apps/runtime/tests/isolation/harness.rs')
  const imageCheck = harness.indexOf('unexpected installed image before resume')
  expect(imageCheck).toBeGreaterThanOrEqual(0)
  expect(imageCheck).toBeLessThan(harness.indexOf('if ResumeThread('))
  expect(tests).toContain('Duration::from_millis(500)')
  const runtime = JSON.parse(read('apps/runtime/package.json')) as {
    scripts: Record<string, string>
  }
  expect(runtime.scripts['validate:managed-media-candidate']).toBe(
    'pwsh -NoProfile -File ../../scripts/test-managed-media-candidate.ps1'
  )
  for (const task of ['build', 'test', 'lint', 'check-types'])
    expect(runtime.scripts[task]).not.toContain('managed-media-candidate')
  const driver = read('scripts/test-managed-media-candidate.ps1')
  expect(driver.match(/--target-dir/g)).toHaveLength(14)
  expect(driver).toContain("'--target-dir', $managedTargetRoot")
  expect(driver).toContain('$managedCount -ne 6')
  expect(driver).toContain(
    '$nativeCount -ne 10 -or $audioCount -ne 8 -or $videoCount -ne 9 -or $continuationCount -ne 6 -or $healthCount -ne 5 -or $progressCount -ne 5 -or $cohortCount -ne 4 -or $queueCount -ne 4 -or $officialCount -ne 4 -or $discardCount -ne 6 -or $allManagedCount -ne 67'
  )
  expect(driver).toContain("'--skip', 'managed_media::native_inspection::'")
  expect(driver).toContain("'Ten Core-owned native inspection checks'")
  const nativeTests = readFileSync(
    join(fixture, 'native_inspection.rs'),
    'utf8'
  )
  expect(nativeTests.match(/#\[test\]/g)).toHaveLength(10)
  expect(nativeTests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(nativeTests).toContain('start_media_inspection')
  expect(nativeTests).toContain('Duration::from_secs(5)')
  expect(nativeTests).toContain('assert_eq!(observations[0].1, 0)')
  const continuationTests = readFileSync(
    join(fixture, 'native_continuation.rs'),
    'utf8'
  )
  expect(continuationTests.match(/#\[test\]/g)).toHaveLength(6)
  expect(continuationTests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(driver).toContain("'Six live media continuation checks'")
  expect(driver).toContain("'--skip', 'managed_media::native_continuation::'")
  const healthTests = readFileSync(join(fixture, 'native_health.rs'), 'utf8')
  expect(healthTests.match(/#\[test\]/g)).toHaveLength(5)
  expect(healthTests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(driver).toContain("'Five fixed native health checks'")
  expect(driver).toContain("'--skip', 'managed_media::native_health::'")

  const progressTests = readFileSync(
    join(fixture, 'native_progress.rs'),
    'utf8'
  )
  expect(progressTests.match(/#\[test\]/g)).toHaveLength(5)
  expect(progressTests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(driver).toContain("'Five current native progress checks'")
  expect(driver).toContain("'--skip', 'managed_media::native_progress::'")

  const cohortTests = readFileSync(join(fixture, 'native_cohort.rs'), 'utf8')
  expect(cohortTests.match(/#\[test\]/g)).toHaveLength(4)
  expect(cohortTests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(driver).toContain("'Four signed native cohort update checks'")
  expect(driver).toContain("'--skip', 'managed_media::native_cohort::'")

  const queueTests = readFileSync(join(fixture, 'native_queue.rs'), 'utf8')
  expect(queueTests.match(/#\[test\]/g)).toHaveLength(4)
  expect(queueTests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(driver).toContain("'Four signed queued-tool policy checks'")
  expect(driver).toContain("'--skip', 'managed_media::native_queue::'")

  const officialTests = readFileSync(
    join(fixture, 'native_official.rs'),
    'utf8'
  )
  expect(officialTests.match(/#\[test\]/g)).toHaveLength(4)
  expect(officialTests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(driver).toContain("'Four official signed-source media checks'")
  expect(driver).toContain("'--skip', 'managed_media::native_official::'")
  expect(driver).toContain("'prepare-official-media-candidate.ps1'")
  expect(driver).toContain('-BuildRoot $OfficialBuildRoot')
  const officialPreparation = read(
    'scripts/prepare-official-media-candidate.ps1'
  )
  expect(officialPreparation).toContain('usr/bin/gpgv.exe')
  expect(officialPreparation).toContain(
    '--keyring $officialGpgKey --status-fd 1'
  )
  expect(officialPreparation).toContain(
    'FCF986EA15E6E293A5644F10B4322F04D67658D8'
  )
  expect(officialPreparation).toContain('[IO.FileMode]::CreateNew')
  expect(officialPreparation).not.toMatch(
    /trust-model always|skip.signature|skip.verification/i
  )
  const officialRefusals = read('scripts/test-official-media-source.ps1')
  expect(driver).toContain(
    "Invoke-QualityCommand 'Four official source refusal controls'"
  )
  expect(driver).toContain("'test-official-media-source.ps1'")
  expect(officialRefusals).toContain('$sourceCompleted -ne 4')
  expect(officialRefusals).toContain(
    "'Official PGP signature/fingerprint refused'"
  )
  expect(officialRefusals).toContain(
    "'Invalid source/program material published fixture output'"
  )
  expect(officialRefusals).toContain("'Unknown download was changed'")
  const officialBuild = read('scripts/media-candidate/build-official.sh')
  expect(officialBuild).toContain('verify_official_source.py')
  expect(officialBuild).toContain(
    '--disable-autodetect --disable-network --disable-shared'
  )
  expect(officialBuild).toContain('--enable-protocol=file,pipe')
  const officialSource = read(
    'scripts/media-candidate/verify_official_source.py'
  )
  expect(officialSource).toContain('input=archive')
  expect(officialSource).toContain(
    'verify_or_extract(sources / "ffmpeg-8.1.3", entries, True)'
  )
  expect(officialSource).not.toMatch(/shell=True|check=False|verify=False/)
  expect(tests).toContain(
    'assert_eq!(world.row(&second)["toolArtifacts"]["retained"], false)'
  )
  expect(tests).toContain('released.elapsed() < Duration::from_millis(500)')
  const discardTests = readFileSync(
    join(fixture, 'native_queue_discard.rs'),
    'utf8'
  )
  expect(discardTests.match(/#\[test\]/g)).toHaveLength(6)
  expect(discardTests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(driver).toContain(
    "Invoke-QualityCommand 'Six explicit queued-tool discard checks'"
  )
  expect(driver).toContain("'--skip', 'managed_media::native_queue_discard::'")
  const discard = read(
    'packages/runtime-core/src/runtime_tool_queue_discard.rs'
  )
  expect(discard).toContain('pub struct QueuedToolDiscardPlan')
  expect(discard).toContain('job.started_at != job.accepted_at')
  expect(discard).toContain('!self.services.root_drained(run)')
  expect(discard).toContain('Some(&authority)')
  expect(discard).toContain(
    '.apply_confirmed_release(stored.tool_artifacts.unwrap())'
  )
  expect(discard).not.toMatch(
    /Serialize,|Deserialize,|std::fs::|Command::new|std::env::/
  )

  const videoTests = readFileSync(join(fixture, 'native_video.rs'), 'utf8')
  expect(videoTests.match(/#\[test\]/g)).toHaveLength(9)
  expect(videoTests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(driver).toContain("'Nine Core-owned bounded WebM checks'")
  expect(driver).toContain("'--skip', 'managed_media::native_video::'")
  const audioTests = readFileSync(join(fixture, 'native_audio.rs'), 'utf8')
  expect(audioTests.match(/#\[test\]/g)).toHaveLength(8)
  expect(audioTests).not.toMatch(/#\[ignore|std::env::(?:var|args)/)
  expect(driver).toContain("'Eight Core-owned bounded WAV checks'")
  expect(driver).toContain(
    "'--lib', 'managed_media::', '--', '--test-threads=1'"
  )
  expect(driver).toContain(
    "Invoke-QualityCommand 'Managed candidate all-target Clippy'"
  )
  expect(driver).toContain(
    "Invoke-QualityCommand 'Six signed installed-image LPAC checks'"
  )
  expect(driver).not.toMatch(/--features|--pass-with-no-tests/)
  const preparation = read('scripts/prepare-managed-media-candidate.ps1')
  expect(preparation).toContain("'prepare-media-candidate.ps1'")
  expect(preparation).toContain('Exact source notice hash required')
  expect(preparation).toContain(
    'Unknown managed notice output preserved and refused'
  )
})

test('native media stays T0-only and fatal maintenance includes actual call drain', () => {
  const root = fileURLToPath(new URL('../', import.meta.url))
  const read = (path: string) => readFileSync(join(root, path), 'utf8')
  const native = read('packages/runtime-core/src/runtime_media/inspection.rs')
  const kernel = read(
    'packages/runtime-core/src/packages/tools/install/images/inspection/kernel.rs'
  )
  const profile = read(
    'packages/runtime-core/src/packages/tools/install/images/inspection/profile.rs'
  )
  expect(native).toMatch(
    /pub struct MediaInspectionHandle\s*\{\s*instance: String,\s*id: String,/
  )
  expect(profile).toContain('media::recipe(MediaActionV1::Inspect).arguments()')
  expect(kernel).toContain('profile.arguments()')
  expect(profile).not.toMatch(
    /std::env::|Command::new|powershell|cmd\.exe|Deserialize|Serialize/i
  )
  expect(kernel).not.toMatch(/std::env::|Command::new|powershell|cmd\.exe/i)
  for (const hook of [
    'start_media_inspection_fixture',
    'media_inspection_fixture_observations',
    'tick_media_inspections_fixture',
  ])
    expect(native).toMatch(new RegExp(`#\\[cfg\\(test\\)\\]\\s*pub fn ${hook}`))
  const server = read('apps/runtime/src/server.rs')
  expect(server).toContain('locked.tick_media_inspections()')
  expect(server).toContain('native_failure = true')
  expect(server).toContain('locked.active_media_inspections() != 0')
  expect(server).toContain('return Err("NATIVE_MEDIA_FAILED")')
})

test('native isolation probe is mandatory for tests and excluded from normal builds', () => {
  const root = Bun.TOML.parse(
    readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8')
  ) as { workspace: { members: string[]; 'default-members': string[] } }
  const fixture = 'apps/runtime/test/fixtures/isolation-probe'
  expect(root.workspace.members).toContain(fixture)
  expect(root.workspace['default-members']).toEqual([
    'packages/runtime-core',
    'apps/runtime',
  ])
  const manifest = Bun.TOML.parse(
    readFileSync(new URL(`../${fixture}/Cargo.toml`, import.meta.url), 'utf8')
  ) as { package: { publish: boolean } }
  expect(manifest.package.publish).toBe(false)
  const runtime = JSON.parse(
    readFileSync(
      new URL('../apps/runtime/package.json', import.meta.url),
      'utf8'
    )
  ) as { scripts: Record<string, string> }
  expect(runtime.scripts.build).toBe(
    'bun run prepare.ts && cargo build --locked --manifest-path Cargo.toml'
  )
  expect(runtime.scripts.test?.split(' && ')).toEqual([
    'bun run prepare.ts',
    'cargo build --locked --manifest-path Cargo.toml',
    'cargo build --locked --manifest-path test/fixtures/isolation-probe/Cargo.toml',
    'bun run ../../scripts/check-rust-tests.ts Cargo.toml',
    'cargo test --locked --manifest-path Cargo.toml',
    'bun test test',
  ])
  expect(runtime.scripts.lint).toContain(
    '&& cargo clippy --locked --manifest-path test/fixtures/isolation-probe/Cargo.toml --all-targets -- -D warnings'
  )
  expect(runtime.scripts['check-types']).toContain(
    '&& cargo check --locked --manifest-path test/fixtures/isolation-probe/Cargo.toml --all-targets'
  )
  const core = JSON.parse(
    readFileSync(
      new URL('../packages/runtime-core/package.json', import.meta.url),
      'utf8'
    )
  ) as { scripts: Record<string, string> }
  expect(core.scripts.test?.split(' && ')).toEqual([
    'bun run manifest-conformance.ts --check',
    'bun run generate-catalog.ts',
    'cargo build --locked --manifest-path ../../apps/runtime/test/fixtures/isolation-probe/Cargo.toml',
    'bun run ../../scripts/check-rust-tests.ts Cargo.toml',
    'cargo test --locked --manifest-path Cargo.toml -- --test-threads=1',
  ])
  expect(core.scripts.build).not.toContain('isolation-probe')
})

function runPowerShell(command: string, cwd?: string) {
  return spawnSync(
    'pwsh',
    ['-NoProfile', '-NonInteractive', '-Command', command],
    {
      cwd,
      encoding: 'utf8',
    }
  )
}

test('uses unprivileged PR events and immutable action references', () => {
  expect('pull_request' in workflow.on).toBe(true)
  expect('workflow_dispatch' in workflow.on).toBe(true)
  expect('pull_request_target' in workflow.on).toBe(false)
  expect(workflow.permissions).toEqual({ contents: 'read' })
  expect(workflow.jobs.quality['continue-on-error']).toBeUndefined()
  for (const step of workflow.jobs.quality.steps) {
    expect(step['continue-on-error']).toBeUndefined()
    if (step.uses) {
      expect(step.uses).toMatch(/^[\w-]+\/[\w-]+@[a-f0-9]{40}$/)
    }
  }
  expect(workflow.jobs.quality.steps[0]?.with?.['persist-credentials']).toBe(
    false
  )
})

test('security gates require full history and remain fatal on PR and release branches', async () => {
  expect(workflow.on.push).toEqual({
    branches: ['main', 'codex/**', 'release/**'],
  })
  expect(workflow.jobs.quality.steps[0]?.with?.['fetch-depth']).toBe(0)
  const entry = await Bun.file(
    new URL('./check-ci.ps1', import.meta.url)
  ).text()
  expect(entry).toContain("'run', 'test:security'")
  expect(entry.indexOf("'test:security'")).toBeGreaterThan(
    entry.indexOf("'install', '--frozen-lockfile'")
  )
  expect(entry.indexOf("'test:security'")).toBeLessThan(
    entry.indexOf("'build:packages'")
  )
  const root = (await Bun.file(
    new URL('../package.json', import.meta.url)
  ).json()) as { scripts: Record<string, string> }
  expect(root.scripts['test:security']).toBe(
    'bun test ./scripts/security-contracts.test.ts && pwsh -NoProfile -File scripts/check-security.ps1'
  )
  const scanner = await Bun.file(
    new URL('./check-security.ps1', import.meta.url)
  ).text()
  expect(scanner).toContain(". (Join-Path $PSScriptRoot 'ci-gates.ps1')")
  expect(scanner).not.toMatch(
    /continue-on-error|SilentlyContinue|catch\s*\{\s*\}/
  )
})

test('uses runner-dependent Bun cache paths only in step contexts', () => {
  const job = workflow.jobs.quality
  expect(job.env.BUN_INSTALL_CACHE_DIR).toBeUndefined()
  for (const value of Object.values(job.env)) {
    expect(value).not.toMatch(/\$\{\{[^}]*\b(runner|env|steps|job)\./)
  }
  const cachePath = '${{ runner.temp }}/flowtools-bun-cache'
  expect(job.steps.some(step => step.with?.path === cachePath)).toBe(true)
  const gates = job.steps.find(step => step.run === './scripts/check-ci.ps1')
  expect(gates?.env?.BUN_INSTALL_CACHE_DIR).toBe(cachePath)
})

test('runs bounded Windows gates without caching JS build products', async () => {
  const turbo = (await Bun.file(
    new URL('../turbo.json', import.meta.url)
  ).json()) as {
    globalPassThroughEnv: string[]
    tasks: Record<
      'test' | 'build' | 'lint' | 'check-types',
      { passThroughEnv: string[]; outputs: string[] }
    >
  }
  expect(turbo.globalPassThroughEnv).toEqual(['PATHEXT'])
  expect(turbo.tasks.test.passThroughEnv).toEqual(['CARGO_TARGET_DIR'])
  expect(turbo.tasks.build.passThroughEnv).toEqual(['CARGO_TARGET_DIR'])
  expect(turbo.tasks.lint.passThroughEnv).toEqual(['CARGO_TARGET_DIR'])
  expect(turbo.tasks['check-types'].passThroughEnv).toEqual([
    'CARGO_TARGET_DIR',
  ])
  expect(turbo.tasks.build.outputs).toEqual(['dist/**/*', '.generated/**/*'])
  const job = workflow.jobs.quality
  expect(job.name).toBe('Windows quality gates')
  expect(job['runs-on']).toBe('windows-2022')
  expect(job['timeout-minutes']).toBeGreaterThan(0)
  expect(job.defaults.run.shell).toBe('pwsh')
  expect(job.steps.some(step => step.run === './scripts/check-ci.ps1')).toBe(
    true
  )
  expect(job.steps.at(-1)?.if).toBe('always()')
  for (const step of job.steps.filter(item => item.with?.path)) {
    expect(step.with?.path).not.toMatch(/node_modules|\.turbo|\bdist\b/)
  }
  const rawWorkflow = await Bun.file(
    new URL('../.github/workflows/windows-quality.yml', import.meta.url)
  ).text()
  expect(rawWorkflow).toContain(
    "hashFiles('Cargo.lock', 'apps/desktop/src-tauri/Cargo.lock')"
  )
  const rawTurbo = (await Bun.file(
    new URL('../turbo.json', import.meta.url)
  ).json()) as { tasks: Record<string, { cache?: boolean }> }
  expect(rawTurbo['tasks']['@flowtools/runtime#build'].cache).toBe(false)
  expect(rawTurbo['tasks']['@flowtools/runtime-core#build'].cache).toBe(false)
  const entry = await Bun.file(
    new URL('./check-ci.ps1', import.meta.url)
  ).text()
  expect(entry.indexOf("'build:packages'")).toBeLessThan(
    entry.indexOf("'lint'")
  )
  expect(entry.indexOf("'generate:hosts'")).toBeLessThan(
    entry.indexOf("'lint'")
  )
  expect(entry.indexOf("'test'")).toBeLessThan(
    entry.indexOf("'build', '--force'")
  )
  expect(entry).toContain("'install', '--frozen-lockfile'")
  expect(entry).toContain("'run', 'docs:check'")
  expect(entry).toContain("'run', 'verify:plugin-catalog'")
  expect(entry).toContain("'run', 'verify:runtime-contracts'")
  expect(entry.indexOf("'generate:hosts'")).toBeLessThan(
    entry.indexOf("'verify:runtime-contracts'")
  )
  expect(entry.indexOf("'verify:runtime-contracts'")).toBeLessThan(
    entry.indexOf("'lint'")
  )
  expect(entry.indexOf("'verify:plugin-catalog'")).toBeLessThan(
    entry.indexOf("'lint'")
  )
  expect(entry.indexOf("'docs:check'")).toBeLessThan(entry.indexOf("'lint'"))
  expect(entry).toContain("'check', '--locked'")
})

test('clean bootstrap builds the declared Runtime client used by the native harness', async () => {
  const root = (await Bun.file(
    new URL('../package.json', import.meta.url)
  ).json()) as { scripts: Record<string, string> }
  const consumer = (await Bun.file(
    new URL('../apps/ui-test/package.json', import.meta.url)
  ).json()) as { devDependencies: Record<string, string> }
  const harness = await Bun.file(
    new URL(
      '../apps/ui-test/scripts/validate-runtime-hosts.ts',
      import.meta.url
    )
  ).text()
  expect(root.scripts['build:packages']).toContain(
    '--filter=@flowtools/runtime-client'
  )
  expect(consumer.devDependencies['@flowtools/runtime-client']).toBe(
    'workspace:*'
  )
  expect(harness).toContain("from '@flowtools/runtime-client'")
  expect(harness).toContain("from '@flowtools/runtime-client/node'")
  expect(harness).not.toContain('runtime-client/dist')
})

test('uncached native builds retain their actual Turbo prerequisite graph', () => {
  const rootPackage = JSON.parse(
    readFileSync(new URL('../package.json', import.meta.url), 'utf8')
  ) as { scripts: Record<string, string> }
  const plan = spawnSync(
    process.execPath,
    [
      'x',
      '--no-install',
      ...rootPackage.scripts['build:packages']!.split(' '),
      '--dry=json',
    ],
    {
      cwd: fileURLToPath(new URL('../', import.meta.url)),
      encoding: 'utf8',
      maxBuffer: 4 * 1024 * 1024,
    }
  )
  if (plan.status !== 0) {
    throw new Error(`Turbo build plan failed: ${plan.stderr}`)
  }
  const tasks = (
    JSON.parse(plan.stdout) as {
      tasks: {
        taskId: string
        dependencies: string[]
        resolvedTaskDefinition: {
          cache: boolean
          passThroughEnv: string[] | null
        }
      }[]
    }
  ).tasks
  const dependencies = (taskId: string) =>
    tasks.find(task => task.taskId === taskId)?.dependencies
  for (const taskId of [
    '@flowtools/runtime#build',
    '@flowtools/runtime-core#build',
  ]) {
    const task = tasks.find(task => task.taskId === taskId)
    expect(task?.resolvedTaskDefinition.cache).toBe(false)
    expect(task?.resolvedTaskDefinition.passThroughEnv).toEqual([
      'CARGO_TARGET_DIR',
    ])
  }
  expect(dependencies('@flowtools/runtime#build')).toEqual(
    expect.arrayContaining([
      '@flowtools/plugin-runner#build',
      '@flowtools/runtime-core#build',
      '@flowtools/runtime-client#build',
    ])
  )
  expect(dependencies('@flowtools/runtime-core#build')).toEqual(
    expect.arrayContaining(['@flowtools/plugins#build', '@flowtools/sdk#build'])
  )
  expect(dependencies('@flowtools/cli#build')).toEqual(
    expect.arrayContaining(['@flowtools/runtime-client#build'])
  )
  expect(
    tasks.find(task => task.taskId === '@flowtools/cli#build')
      ?.resolvedTaskDefinition.passThroughEnv
  ).toEqual(['CARGO_TARGET_DIR'])
  expect(dependencies('@flowtools/runtime-client#build')).not.toContain(
    '@flowtools/runtime#build'
  )
}, 30_000)

test('Runtime codegen stays clean after Windows CRLF checkout and rejects content drift', () => {
  const fixture = mkdtempSync(join(tmpdir(), 'flowtools-ci-generated-'))
  if (dirname(resolve(fixture)) !== resolve(tmpdir())) {
    throw new Error('Fixture cleanup target escaped temporary directory')
  }
  const attributes = readFileSync(
    new URL('../.gitattributes', import.meta.url),
    'utf8'
  )
  const files = [
    'bindings.ts',
    'wire-schema.json',
    'wire-fixtures.json',
    'wire-validators.cjs',
    'wire-validators.d.cts',
  ].map(name => `packages/runtime-client/src/${name}`)
  files.push(
    'apps/desktop/src-tauri/permissions/autogenerated/get_plugins.toml'
  )
  const runGit = (args: string[]) => {
    const result = spawnSync('git', args, { cwd: fixture, encoding: 'utf8' })
    if (result.status !== 0) {
      throw new Error(`Git fixture failed: ${result.stderr}`)
    }
  }
  try {
    runGit(['init', '--quiet'])
    runGit(['config', 'core.autocrlf', 'true'])
    writeFileSync(join(fixture, '.gitattributes'), attributes)
    for (const file of files) {
      mkdirSync(dirname(join(fixture, file)), { recursive: true })
      writeFileSync(join(fixture, file), 'generated\nfixture\n')
    }
    runGit(['add', '.'])
    runGit([
      '-c',
      'user.name=CI fixture',
      '-c',
      'user.email=ci@fixture.invalid',
      'commit',
      '--quiet',
      '-m',
      'fixture',
    ])
    runGit(['checkout-index', '--force', '--all'])
    for (const file of files) {
      expect(readFileSync(join(fixture, file), 'utf8')).toBe(
        'generated\nfixture\n'
      )
      // The Rust generator writes LF regardless of the checkout platform.
      writeFileSync(join(fixture, file), 'generated\nfixture\n')
    }
    const clean = runPowerShell(
      `. ${helperPath}; Assert-CleanWorktree`,
      fixture
    )
    expect(clean.status).toBe(0)
    writeFileSync(join(fixture, files[0]!), 'changed\nfixture\n')
    const dirty = runPowerShell(
      `. ${helperPath}; Assert-CleanWorktree`,
      fixture
    )
    expect(dirty.status).toBe(1)
    expect(dirty.stdout).toContain(files[0]!)
  } finally {
    rmSync(fixture, { recursive: true, force: true })
  }
}, 30_000)

test('Manifest conformance gate refuses drift without writes and requires explicit regeneration', () => {
  const root = fileURLToPath(new URL('../', import.meta.url))
  const fixtureParent = resolve(root, 'packages/runtime-core/.generated')
  const fixture = resolve(
    fixtureParent,
    `g5-manifest-conformance-${randomUUID()}`
  )
  if (dirname(fixture) !== fixtureParent) {
    throw new Error('Fixture cleanup target escaped generated directory')
  }
  const runtime = join(fixture, 'packages', 'runtime-core')
  const source = join(root, 'packages', 'runtime-core')
  const generator = join(runtime, 'manifest-conformance.ts')
  const corpus = join(runtime, 'manifest-conformance.json')
  const dependencies = join(runtime, 'node_modules')
  let linkedDependencies = false
  const expected = readFileSync(join(source, 'manifest-conformance.json'))
  const manifest = JSON.parse(
    readFileSync(join(source, 'package.json'), 'utf8')
  ) as { scripts: Record<string, string> }
  for (const task of ['generate', 'verify:bindings', 'test']) {
    expect(manifest.scripts[task]).toMatch(
      /^bun run manifest-conformance\.ts --check && /
    )
  }
  const run = (args: string[] = []) =>
    spawnSync(process.execPath, [generator, ...args], {
      cwd: runtime,
      encoding: 'utf8',
      env: {
        PATH: process.env.PATH,
        SystemRoot: process.env.SystemRoot,
        TEMP: process.env.TEMP,
        TMP: process.env.TMP,
      },
    })
  try {
    mkdirSync(runtime, { recursive: true })
    symlinkSync(join(source, 'node_modules'), dependencies, 'junction')
    linkedDependencies = true
    writeFileSync(
      generator,
      readFileSync(join(source, 'manifest-conformance.ts'))
    )
    writeFileSync(corpus, expected)
    writeFileSync(
      join(fixture, '.oxfmtrc.json'),
      readFileSync(join(root, '.oxfmtrc.json'))
    )
    for (const args of [[], ['--check']]) {
      const clean = run(args)
      expect(clean.error).toBeUndefined()
      expect(clean.status, clean.stdout + clean.stderr).toBe(0)
      expect(clean.stdout).toContain('unchanged')
      expect(readFileSync(corpus).equals(expected)).toBe(true)
    }
    const changed = JSON.parse(expected.toString('utf8')) as {
      cases: { normalized?: { name: string } }[]
    }
    const first = changed.cases[0]?.normalized
    expect(first).toBeDefined()
    first!.name = 'CI controlled conformance drift'
    const drift = Buffer.from(JSON.stringify(changed))
    writeFileSync(corpus, drift)
    const denied = run(['--check'])
    expect(denied.error).toBeUndefined()
    expect(denied.status, denied.stdout + denied.stderr).not.toBe(0)
    expect(denied.stderr).toContain('SDK Manifest conformance drift')
    expect(readFileSync(corpus).equals(drift)).toBe(true)
    const regenerated = run(['--write'])
    expect(regenerated.error).toBeUndefined()
    expect(regenerated.status, regenerated.stdout + regenerated.stderr).toBe(0)
    expect(regenerated.stdout).toContain('written')
    expect(readFileSync(corpus, 'utf8')).toBe(
      expected.toString('utf8').replaceAll('\r\n', '\n')
    )
  } finally {
    if (linkedDependencies) unlinkSync(dependencies)
    rmSync(fixture, { recursive: true, force: true })
  }
}, 30_000)

test('browser gate bounds file concurrency without replacing or narrowing the runner', async () => {
  const config = await Bun.file(
    new URL('../apps/ui-test/vitest.browser.config.ts', import.meta.url)
  ).text()
  const app = (await Bun.file(
    new URL('../apps/ui-test/package.json', import.meta.url)
  ).json()) as {
    scripts: Record<string, string>
    devDependencies: Record<string, string>
  }
  expect(config).toMatch(/fileParallelism:\s*false/)
  expect(config).toContain('provider: playwright()')
  expect(config).toContain("instances: [{ browser: 'chromium' }]")
  expect(config).not.toMatch(/(?:exclude|retry|passWithNoTests)\s*:/)
  expect(app.scripts.test).toBe('vitest run --config vitest.browser.config.ts')
  expect(app.devDependencies.playwright).toBe('1.59.1')
})

test('browser API uses the actual OS-selected IPv4 loopback port with strict binding', () => {
  const config = new URL(
    '../apps/ui-test/vitest.browser.config.ts',
    import.meta.url
  )
  const probe = spawnSync(
    process.execPath,
    [
      '-e',
      `import assert from 'node:assert/strict'
import { Server, createServer } from 'node:net'
const selected = []
const original = Server.prototype.listen
Server.prototype.listen = function (...args) {
  if (args[0] === 0 && args[1] === '127.0.0.1') {
    this.once('listening', () => selected.push(this.address().port))
  }
  return original.apply(this, args)
}
const exported = (await import(${JSON.stringify(config.href)})).default
const config = await exported({ command: 'serve', mode: 'test' })
const api = config.test.browser.api
assert.equal(selected.length, 1)
assert.equal(api.port, selected[0])
assert.ok(api.port > 0)
assert.equal(api.host, '127.0.0.1')
assert.equal(api.strictPort, true)
const rebound = createServer()
await new Promise((resolve, reject) => {
  rebound.once('error', reject)
  rebound.listen({ port: api.port, host: api.host, exclusive: true }, resolve)
})
await new Promise((resolve, reject) => rebound.close(error => error ? reject(error) : resolve()))
process.stdout.write('OS-selected browser API port verified\\n')`,
    ],
    {
      cwd: fileURLToPath(new URL('../apps/ui-test', import.meta.url)),
      encoding: 'utf8',
    }
  )
  expect(probe.error).toBeUndefined()
  expect(probe.status, probe.stdout + probe.stderr).toBe(0)
  expect(probe.stdout.trim()).toBe('OS-selected browser API port verified')
}, 30_000)

test('PowerShell propagates native failure before subsequent gates run', () => {
  for (const host of ['bun', 'node']) {
    const success = runPowerShell(
      `. ${helperPath}; Invoke-QualityCommand 'fixture' ${host} @('-e', 'process.exit(0)')`
    )
    expect(success.error).toBeUndefined()
    if (success.status !== 0) {
      throw new Error(`PowerShell success probe failed: ${success.stderr}`)
    }
    expect(success.status).toBe(0)
    const failure = runPowerShell(
      `. ${helperPath}; Invoke-QualityCommand 'fixture' ${host} @('-e', 'process.exit(17)'); Write-Output 'UNEXPECTED_CONTINUATION'`
    )
    expect(failure.error).toBeUndefined()
    expect(failure.status).toBe(1)
    expect(failure.stderr).toContain('exit code 17')
    expect(failure.stdout).not.toContain('UNEXPECTED_CONTINUATION')
  }
}, 30_000)

test('production artifact refusal is a fatal post-build gate before the clean-worktree check', async () => {
  const entry = await Bun.file(
    new URL('./check-ci.ps1', import.meta.url)
  ).text()
  const gate = entry.indexOf("'verify:production-entrypoints'")
  expect(entry).toContain(
    "Invoke-QualityCommand 'Production external-entrypoint artifacts' bun @("
  )
  expect(gate).toBeGreaterThan(entry.indexOf("'build', '--force'"))
  expect(gate).toBeLessThan(entry.lastIndexOf('Assert-CleanWorktree'))
  const root = (await Bun.file(
    new URL('../package.json', import.meta.url)
  ).json()) as {
    scripts: Record<string, string>
  }
  expect(root.scripts['verify:production-entrypoints']).toBe(
    'bun run scripts/verify-production-entrypoints.ts'
  )
})

test('worktree guard accepts clean repositories and rejects untracked drift', () => {
  const fixture = mkdtempSync(join(tmpdir(), 'flowtools-ci-contract-'))
  try {
    const init = spawnSync('git', ['init', '--quiet'], { cwd: fixture })
    expect(init.status).toBe(0)
    const clean = runPowerShell(
      `. ${helperPath}; Assert-CleanWorktree`,
      fixture
    )
    if (clean.status !== 0) {
      throw new Error(`Clean fixture probe failed: ${clean.stderr}`)
    }
    expect(clean.status).toBe(0)
    writeFileSync(join(fixture, 'generated.txt'), 'fixture')
    const dirty = runPowerShell(
      `. ${helperPath}; Assert-CleanWorktree`,
      fixture
    )
    expect(dirty.status).toBe(1)
    expect(dirty.stdout).toContain('generated.txt')
    expect(dirty.stderr).toContain('tracked or untracked repository files')
  } finally {
    rmSync(fixture, { recursive: true, force: true })
  }
}, 30_000)
