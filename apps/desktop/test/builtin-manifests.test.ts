import { expect, test } from 'bun:test'

import { builtInManifests } from '../src/plugin/manifests'

test.each(builtInManifests)(
  'Desktop manifest $id validates and loads its actual T1 UI module',
  async manifest => {
    const module = await manifest.loader()
    expect(module.default.meta.id).toBe(manifest.id)
    expect(module.default.meta.version).toBe(manifest.version)
    expect(module.default.type).toBe('app')
    expect(module.default.run).toBeFunction()
  },
  30_000
)
