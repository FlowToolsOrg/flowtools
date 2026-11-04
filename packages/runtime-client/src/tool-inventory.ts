import type { ToolInventoryPage, ToolInventoryQuery } from './bindings'

import { decodeResponse } from './codec'

const id = (s: string) =>
  /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(s) && s.length <= 128
const version = (s: string) =>
  /^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/.test(s) &&
  s.length <= 128
const digest = (s: string) => /^[a-f0-9]{64}$/.test(s)

/** Same schema and consistency checks for native GUI and named-pipe CLI. */
export function decodeToolInventoryPage(
  value: unknown,
  query: ToolInventoryQuery
): ToolInventoryPage {
  const response = decodeResponse({
    version: 1,
    requestId: 'tool-inventory-observation',
    outcome: { type: 'tool-inventory', data: value },
  })
  const invalid = () => {
    throw new Error('INVALID_RESPONSE')
  }
  if (response.outcome.type !== 'tool-inventory') return invalid()
  const page = response.outcome.data
  if (
    !Number.isInteger(query.offset) ||
    query.offset < 0 ||
    query.offset > 64 ||
    query.offset % 8 !== 0 ||
    (query.offset > 0 && !query.snapshot)
  )
    return invalid()
  if (page.availability === 'not-configured') {
    if (
      query.offset ||
      query.snapshot ||
      page.snapshot !== null ||
      page.storage !== null ||
      page.total ||
      page.nextOffset !== null ||
      page.items.length
    )
      return invalid()
    return page
  }
  const storage = page.storage
  const end = Math.min(query.offset + 8, page.total)
  if (
    !page.snapshot ||
    !digest(page.snapshot) ||
    (query.snapshot !== null && query.snapshot !== page.snapshot) ||
    page.total > 64 ||
    query.offset > page.total ||
    (query.offset > 0 && query.offset === page.total) ||
    page.items.length !== end - query.offset ||
    page.nextOffset !== (end < page.total ? end : null) ||
    !storage ||
    storage.maxBytes !== 268435456 ||
    storage.maxGroups !== 384 ||
    storage.bytes > storage.maxBytes ||
    storage.groups > storage.maxGroups
  )
    return invalid()
  const identities = new Set<string>()
  for (const item of page.items) {
    const key = [
      item.publisher,
      item.id,
      item.version,
      item.archiveDigest,
    ].join('/')
    if (
      !id(item.publisher) ||
      !id(item.id) ||
      !version(item.version) ||
      !id(item.buildFlavor) ||
      !id(item.sourceRegistry) ||
      !id(item.sourceArtifact) ||
      !digest(item.archiveDigest) ||
      item.archiveBytes > storage.bytes ||
      item.dependents.length > 128 ||
      item.retainedJobs > 1024 ||
      identities.has(key)
    )
      return invalid()
    identities.add(key)
    const dependents = new Set<string>()
    for (const dependent of item.dependents) {
      const identity = [
        dependent.publisher,
        dependent.id,
        dependent.version,
      ].join('/')
      if (
        !id(dependent.publisher) ||
        !id(dependent.id) ||
        !version(dependent.version) ||
        dependents.has(identity)
      )
        return invalid()
      dependents.add(identity)
    }
    const blockers = [
      'deletion-not-implemented',
      ...(item.dependents.length ? ['installed-dependent'] : []),
      ...(item.retainedJobs ? ['retained-task'] : []),
    ]
    if (JSON.stringify(item.deletionBlockers) !== JSON.stringify(blockers))
      return invalid()
  }
  return page
}
