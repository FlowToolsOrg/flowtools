import type { ToolInventoryItem, ToolInventoryPage } from '../src/bindings'

import { expect, test } from 'bun:test'

import { encodeRequest } from '../src/codec'
import { decodeToolInventoryPage } from '../src/tool-inventory'

const query = { offset: 0, snapshot: null }
const empty: ToolInventoryPage = {
  availability: 'not-configured',
  snapshot: null,
  total: 0,
  nextOffset: null,
  storage: null,
  items: [],
}
const item: ToolInventoryItem = {
  publisher: 'reviewed-publisher',
  id: 'ffmpeg',
  version: '1.0.0',
  buildFlavor: 'static',
  archiveDigest: 'a'.repeat(64),
  sourceRegistry: 'registry',
  sourceArtifact: 'media',
  archiveBytes: 10,
  materialized: true,
  dependents: [
    { publisher: 'reviewed-publisher', id: 'converter', version: '1.0.0' },
  ],
  retainedJobs: 1,
  deletionBlockers: [
    'deletion-not-implemented',
    'installed-dependent',
    'retained-task',
  ],
}
const observed: ToolInventoryPage = {
  availability: 'observed',
  snapshot: 'b'.repeat(64),
  total: 1,
  nextOffset: null,
  storage: { bytes: 20, groups: 2, maxBytes: 268435456, maxGroups: 384 },
  items: [item],
}

test('inventory distinguishes unconfigured authority and checks counts, page cursors, blockers and privacy', () => {
  expect(decodeToolInventoryPage(empty, query)).toEqual(empty)
  expect(decodeToolInventoryPage(observed, query)).toEqual(observed)
  for (const bad of [
    { ...empty, storage: observed.storage },
    { ...empty, items: [item] },
    { ...observed, total: 2 },
    { ...observed, snapshot: 'private-path' },
    { ...observed, nextOffset: 8 },
    { ...observed, storage: { ...observed.storage, bytes: 268435457 } },
    { ...observed, items: [{ ...item, retainedJobs: 1025 }] },
    { ...observed, items: [{ ...item, deletionBlockers: [] }] },
    {
      ...observed,
      items: [
        { ...item, dependents: [...item.dependents, ...item.dependents] },
      ],
    },
    { ...observed, items: [{ ...item, path: 'C:/private', argv: [] }] },
    {
      ...observed,
      items: [{ ...item, sourceRegistry: 'https://private.invalid' }],
    },
  ])
    expect(() => decodeToolInventoryPage(bad, query)).toThrow(
      'INVALID_RESPONSE'
    )
  expect(() =>
    decodeToolInventoryPage(observed, { offset: 8, snapshot: 'c'.repeat(64) })
  ).toThrow('INVALID_RESPONSE')
})

test('inventory wire forbids client paths, selectors and unsafe integer offsets', () => {
  const request = {
    version: 1,
    requestId: 'inventory',
    session: null,
    call: { method: 'tools.inventory', payload: query },
  }
  expect(JSON.parse(encodeRequest(request))).toEqual(request)
  for (const payload of [
    { ...query, executable: 'ffmpeg.exe' },
    { ...query, profile: 'C:/private' },
    { ...query, offset: -1 },
    { ...query, offset: 0.5 },
    { ...query, offset: Number.MAX_SAFE_INTEGER },
  ]) {
    expect(() =>
      encodeRequest({ ...request, call: { ...request.call, payload } })
    ).toThrow('INVALID_REQUEST')
  }
})
