import { expect, test } from 'bun:test'

import { resolveCatalogEntry } from '../src/runtime/catalog-entry'

test('portable catalog discovery needs an explicit development checkout, never a published path', () => {
  const entry = { packageRoot: 'plugins/example', entry: 'dist/index.html' }
  expect(resolveCatalogEntry(entry, { development: true })).toBeUndefined()
  expect(
    resolveCatalogEntry(entry, {
      development: false,
      checkoutRoot: 'D:/fixture',
    })
  ).toBeUndefined()
  expect(
    resolveCatalogEntry(entry, {
      development: true,
      checkoutRoot: 'D:/fixture',
    })
  ).toBe('D:/fixture/plugins/example/dist/index.html')
  for (const path of ['../index.html', '/index.html', 'C:/bad.html'])
    expect(
      resolveCatalogEntry(
        { ...entry, entry: path },
        { development: true, checkoutRoot: 'D:/fixture' }
      )
    ).toBeUndefined()
  for (const checkoutRoot of [
    'D:/fixture?query',
    'D:/fixture#hash',
    'D:/\u0000fixture',
  ])
    expect(
      resolveCatalogEntry(entry, { development: true, checkoutRoot })
    ).toBeUndefined()
})
