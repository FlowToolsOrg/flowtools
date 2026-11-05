import { expect, test } from 'bun:test'

import {
  verifyManifestContracts,
  verifyBuiltinManifests,
} from './verify-manifests'

test('the serialized protocol fixture passes the same public manifest validator', () => {
  expect(verifyManifestContracts()).toEqual([
    'flowtools/fixture-plugin/convert',
  ])
})

test('actual sixteen built-in package manifests and bytes match the fixed inventory', () => {
  const manifests = verifyBuiltinManifests()
  expect(manifests).toHaveLength(16)
  expect(manifests.filter(m => m.formatVersion === 1)).toHaveLength(15)
  expect(manifests.filter(m => m.formatVersion === 2).map(m => m.id)).toEqual([
    'plugin-resource-reader',
  ])
})
