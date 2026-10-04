import { expect, test } from 'bun:test'
import { createHash } from 'node:crypto'
import {
  mkdtempSync,
  mkdirSync,
  rmSync,
  writeFileSync,
  symlinkSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { inspectHtmlPluginCatalog } from './inspect-html-plugins'
import { verifyCatalogFiles } from './verify-plugin-catalog'

test('checked-in catalogs share valid portable identities and actual fixture digests', async () => {
  verifyCatalogFiles(
    await Bun.file(
      new URL(
        '../apps/desktop/src/data/html-plugin-catalog.json',
        import.meta.url
      )
    ).json(),
    await Bun.file(
      new URL('../docs/html-plugin-catalog.json', import.meta.url)
    ).json()
  )
})

test('scanner records real entry hashes, never source/remote entries or package-controlled certification', () => {
  const root = mkdtempSync(join(tmpdir(), 'flowtools-catalog-'))
  const add = (id: string, main: string, html?: string) => {
    const dir = join(root, 'plugins', id)
    mkdirSync(dir, { recursive: true })
    writeFileSync(
      join(dir, 'plugin.json'),
      JSON.stringify({
        name: id,
        main,
        maturity: 'production',
        certified: true,
        features: [{ code: 'fixture', mainPush: 'true' }],
        development: { main: 'http://localhost:5173' },
      })
    )
    if (html) writeFileSync(join(dir, 'index.html'), html)
  }
  try {
    add('static', 'index.html', '<html>controlled static fixture</html>')
    add(
      'compiled',
      'index.html',
      '<script src="./assets/index-Ab123.js" type="module"></script>'
    )
    add(
      'source',
      'index.html',
      '<script type="module" src="/src/main.tsx"></script>'
    )
    add('remote', 'https://fixture.invalid/index.html')
    add('escape', '../index.html')
    add('missing', 'index.html')
    const catalog = inspectHtmlPluginCatalog(root)
    const entry = catalog.plugins.find(plugin => plugin.id === 'static')!
    expect(entry.maturity).toBe('prototype')
    expect(entry.html.features[0]?.mainPush).toBeUndefined()
    expect(entry.html.compatibility.notes.join(' ')).toContain('非布尔')
    expect(entry.evidence).toEqual({
      status: 'entry-resolved',
      fixtureId: 'static-entry-v1',
      path: 'index.html',
      sha256: createHash('sha256')
        .update('<html>controlled static fixture</html>')
        .digest('hex'),
    })
    expect(
      catalog.plugins.filter(
        plugin => plugin.evidence.status === 'entry-resolved'
      )
    ).toHaveLength(2)
    expect(JSON.stringify(catalog)).not.toContain(root)
    expect(JSON.stringify(catalog)).not.toContain('localhost')
    expect(JSON.stringify(catalog)).not.toContain('/src/main.tsx')
    const fake = structuredClone(catalog)
    Reflect.set(fake.plugins[0]!, 'evidence', {
      status: 'production-certified',
      certified: true,
    })
    expect(() => verifyCatalogFiles(fake, fake)).toThrow()
    const badHash = structuredClone(catalog)
    badHash.fixtures[0]!.sha256 = 'a'.repeat(64)
    expect(() => verifyCatalogFiles(badHash, badHash)).toThrow(
      'digest mismatch'
    )
    expect(() =>
      verifyCatalogFiles(
        { ...catalog, fixtures: [] },
        { ...catalog, fixtures: [] }
      )
    ).toThrow()
    expect(() =>
      verifyCatalogFiles(catalog, { ...catalog, source: 'wrong' })
    ).toThrow()
    verifyCatalogFiles(catalog, structuredClone(catalog))
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test('scanner refuses symlink entry files outside their package', () => {
  const root = mkdtempSync(join(tmpdir(), 'flowtools-catalog-link-'))
  try {
    const dir = join(root, 'plugins', 'escape')
    mkdirSync(dir, { recursive: true })
    const outside = join(root, 'outside')
    mkdirSync(outside)
    writeFileSync(join(outside, 'index.html'), '<html>outside</html>')
    writeFileSync(
      join(dir, 'plugin.json'),
      JSON.stringify({ name: 'escape', main: 'assets/index.html' })
    )
    symlinkSync(outside, join(dir, 'assets'), 'junction')
    expect(() => inspectHtmlPluginCatalog(root)).toThrow('outside package')
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})
