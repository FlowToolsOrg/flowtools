import { expect, test } from 'bun:test'
import { createHash } from 'node:crypto'
import {
  mkdirSync,
  mkdtempSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'

import {
  artifactDigests,
  assertProductionArtifacts,
  readProductionArtifacts,
  type ProductionArtifact,
} from './production-artifacts'

function artifact(path: string, text: string): ProductionArtifact {
  return {
    path,
    size: Buffer.byteLength(text),
    sha256: createHash('sha256').update(text).digest('hex'),
    text,
  }
}

function fixture(js = 'const denial = "EXTERNAL_CODE_DISABLED";') {
  return [
    artifact(
      'index.html',
      '<script type="module" src="/assets/app.js"></script>'
    ),
    artifact('assets/app.js', js),
  ]
}

test('accepts production denial and documentation strings without certifying security', () => {
  const artifacts = fixture(
    'const note = "production-certified is not authorization";'
  )
  expect(() => assertProductionArtifacts(artifacts)).not.toThrow()
  expect(artifactDigests(artifacts)).toEqual(
    artifacts.map(({ path, size, sha256 }) => ({ path, size, sha256 }))
  )
})

test('rejects empty, missing HTML, missing JavaScript and zero-byte entry artifacts', () => {
  for (const files of [
    [],
    [artifact('assets/app.js', 'const fixture = 1')],
    [artifact('index.html', '<html></html>')],
    [artifact('index.html', ''), artifact('assets/app.js', '')],
  ])
    expect(() => assertProductionArtifacts(files)).toThrow(
      'Incomplete production artifacts'
    )
})

for (const marker of [
  '__flowtools_plugin_loaded__',
  '__flowtools_importmap__',
  'https://esm.sh/',
  'flowtools:html-plugin-call',
  'flowtoolsHtmlPluginBridge',
  'development-html-plugin-surface',
  'development-plugin-file-loader',
  '/@fs/private-checkout/main.html',
]) {
  test(`rejects real external execution fingerprint: ${marker}`, () => {
    expect(() =>
      assertProductionArtifacts(
        fixture(`const leaked = ${JSON.stringify(marker)}`)
      )
    ).toThrow('Forbidden external execution marker')
  })
}

for (const js of [
  'const claims = { certified: true };',
  'if (request.certified) execute();',
  'if (request["productionCertified"]) execute();',
  'const isProductionCertified = !0;',
]) {
  test(`rejects certification switch syntax: ${js}`, () => {
    expect(() => assertProductionArtifacts(fixture(js))).toThrow(
      'Unsupported certification switch'
    )
  })
}

test('rejects all supplied path, URL and private-key canaries, including non-JS output', () => {
  for (const canary of [
    'X:/private-checkout-canary',
    'https://preview.invalid/',
    'fake-private-key',
  ]) {
    expect(() =>
      assertProductionArtifacts(
        [...fixture(), artifact('assets/output.svg', `<svg>${canary}</svg>`)],
        [canary]
      )
    ).toThrow('Production environment canary leaked')
  }
})

test('reads real bytes, stable portable identities and hashes without modifying artifacts', () => {
  const root = mkdtempSync(join(tmpdir(), 'flowtools-release-gate-'))
  try {
    mkdirSync(join(root, 'assets'))
    writeFileSync(join(root, 'index.html'), '<html>fixture</html>')
    writeFileSync(join(root, 'assets', 'app.js'), 'const fixture = 1')
    const first = readProductionArtifacts(root)
    expect(first.map(file => file.path)).toEqual([
      'assets/app.js',
      'index.html',
    ])
    expect(first[0]?.sha256).toBe(
      createHash('sha256').update('const fixture = 1').digest('hex')
    )
    expect(readProductionArtifacts(root)).toEqual(first)
  } finally {
    expect(dirname(realpathSync(root))).toBe(realpathSync(tmpdir()))
    rmSync(realpathSync(root), { recursive: true })
  }
})

test('refuses redirected artifact directories before reading outside the fixed build tree', () => {
  const root = mkdtempSync(join(tmpdir(), 'flowtools-release-gate-'))
  try {
    const dist = join(root, 'dist')
    const outside = join(root, 'outside')
    mkdirSync(dist)
    mkdirSync(outside)
    writeFileSync(join(outside, 'secret.js'), 'must not read')
    symlinkSync(
      outside,
      join(dist, 'redirect'),
      process.platform === 'win32' ? 'junction' : 'dir'
    )
    expect(() => readProductionArtifacts(dist)).toThrow(
      'Redirected production artifact'
    )
  } finally {
    expect(dirname(realpathSync(root))).toBe(realpathSync(tmpdir()))
    rmSync(realpathSync(root), { recursive: true })
  }
})
