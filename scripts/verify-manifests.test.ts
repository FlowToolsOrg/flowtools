import { expect, test } from 'bun:test'

import { verifyManifestContracts } from './verify-manifests'

test('the serialized protocol fixture passes the same public manifest validator', () => {
  expect(verifyManifestContracts()).toEqual([
    'flowtools/fixture-plugin/convert',
  ])
})
