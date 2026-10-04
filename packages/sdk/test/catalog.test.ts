import { expect, test } from 'bun:test'

import {
  htmlPluginCatalogSchema,
  portablePluginPathSchema,
} from '../src/compat/catalog'

test('portable catalog paths reject absolute, encoded, traversal and Windows aliases', () => {
  expect(portablePluginPathSchema.parse('dist/assets/index.html')).toBe(
    'dist/assets/index.html'
  )
  for (const path of [
    '../index.html',
    '/index.html',
    'C:/checkout/index.html',
    '\\\\host\\share',
    'a\\b',
    'a//b',
    './a',
    '%2e%2e/a',
    'a:stream',
    'NUL.html',
    'a./b',
    'a /b',
    'a?x',
    'a#x',
    'a|b',
    '*/b',
  ])
    expect(portablePluginPathSchema.safeParse(path).success).toBe(false)
})

test('catalog evidence cannot be fabricated from a compatibility or maturity label', () => {
  const empty = {
    formatVersion: 1,
    source: 'legacy-html-checkout',
    fixtures: [],
    totals: { plugins: 0, commands: 0, appPlugins: 0, toolPlugins: 0 },
    byCompatibility: {},
    byCategory: {},
    plugins: [],
  }
  expect(htmlPluginCatalogSchema.safeParse(empty).success).toBe(true)
  expect(
    htmlPluginCatalogSchema.safeParse({ ...empty, source: 'D:/checkout' })
      .success
  ).toBe(false)
  expect(
    htmlPluginCatalogSchema.safeParse({ ...empty, certified: true }).success
  ).toBe(false)
  expect(
    htmlPluginCatalogSchema.safeParse({
      ...empty,
      totals: { ...empty.totals, plugins: 1 },
    }).success
  ).toBe(false)
})
