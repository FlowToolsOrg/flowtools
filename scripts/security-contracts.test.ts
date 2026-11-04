/// <reference types="bun-types" />

import { expect, test } from 'bun:test'
import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const read = (path: string) => readFileSync(join(root, path), 'utf8')

test('scanner bootstrap pins both upstream ZIPs and extracted executables', () => {
  const source = read('scripts/security-tools.ps1')
  expect(source.match(/ArchiveSha256 = '[a-f0-9]{64}'/g)).toHaveLength(2)
  expect(source.match(/ExecutableSha256 = '[a-f0-9]{64}'/g)).toHaveLength(2)
  expect(source).toContain('Assert-SecurityDigest $archive $pin.ArchiveSha256')
  expect(source).toContain(
    'Assert-SecurityDigest $executable $pin.ExecutableSha256'
  )
  expect(source).toContain(
    'https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/'
  )
  expect(source).toContain(
    'https://github.com/rustsec/rustsec/releases/download/cargo-audit/v0.22.2/'
  )
  expect(source).not.toMatch(/releases\/latest|Invoke-Expression/)
})

test('Desktop does not compile the obsolete networked devtools dependency', () => {
  expect(read('apps/desktop/src-tauri/Cargo.toml')).not.toContain(
    'tauri-plugin-devtools'
  )
  expect(read('apps/desktop/src-tauri/src/lib.rs')).not.toContain(
    'tauri_plugin_devtools'
  )
})

test('security scans cover every tracked Cargo lock and fail without fresh advisories', () => {
  const result = spawnSync('git', ['ls-files', '*Cargo.lock'], {
    cwd: root,
    encoding: 'utf8',
  })
  expect(result.status).toBe(0)
  const source = read('scripts/check-security.ps1')
  const locks = result.stdout.trim().split(/\r?\n/)
  expect(locks).toHaveLength(4)
  for (const lock of locks) expect(source).toContain(`'${lock}'`)
  expect(source).toContain("bun @('audit')")
  expect(source).toContain("'audit', '--file', $lockfile")
  expect(source).not.toMatch(
    /'--(?:ignore|stale|no-fetch|no-yanked|target-os|target-arch|audit-level)'/
  )
  expect(source).toContain("$shallow -cne 'false'")
  expect(source).toContain("'--redact=100'")
  expect(source).toContain("'--ignore-gitleaks-allow'")
})

test('secret exceptions identify only three existing commits, never whole files', () => {
  const entries = read('.gitleaksignore')
    .split(/\r?\n/)
    .filter(line => line && !line.startsWith('#'))
  expect(entries).toHaveLength(3)
  for (const entry of entries)
    expect(entry).toMatch(
      /^[a-f0-9]{40}:[^:]+:(?:private-key|generic-api-key):\d+$/
    )
  const config = Bun.TOML.parse(read('.gitleaks.toml'))
  expect(config).toEqual({
    title: 'FlowTools: upstream rules, no path or rule exclusions',
    extend: { useDefault: true },
  })
  const controls = read('scripts/test-security-scanners.ps1')
  expect(controls).toContain('RUSTSEC-2020-0071')
  expect(controls).toContain('$denied.Contains($credential)')
  expect(controls).toContain("'remove synthetic secret'")
})
