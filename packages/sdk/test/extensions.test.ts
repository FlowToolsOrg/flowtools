import { describe, expect, test } from 'bun:test'

import {
  ExtensionContributionError,
  ExtensionContributionRegistry,
  extensionContributionLimits,
  parseExtensionContributions,
  type ExtensionContributionErrorCode,
  type ExtensionContributionOwner,
} from '../src/extensions'

const owner = (
  pluginId = 'fixture',
  generation = 1,
  version = '1.0.0'
): ExtensionContributionOwner => ({ pluginId, version, generation })

const contribution = (
  id = 'main',
  value: unknown = 'value',
  kind = 'theme'
) => ({
  id,
  kind,
  value,
})

const document = (value: unknown = 'value') => ({
  formatVersion: 1,
  contributions: [contribution('main', value)],
})

function expectCode(
  action: () => unknown,
  code: ExtensionContributionErrorCode
): void {
  let caught: unknown
  try {
    action()
  } catch (error) {
    caught = error
  }
  expect(caught).toBeInstanceOf(ExtensionContributionError)
  expect(caught).toHaveProperty('code', code)
  expect(caught).toHaveProperty('message', code)
}

describe('extension contribution document', () => {
  test('clones and freezes data, preserving all three opaque contribution kinds', () => {
    const value = { nested: [{ a: 1, b: true }], nullable: null }
    const input = {
      formatVersion: 1,
      contributions: [
        contribution('theme', value),
        contribution('locale', { language: 'zh-CN' }, 'locale'),
        contribution('settings', { enabled: true }, 'settings'),
      ],
    }
    const parsed = parseExtensionContributions(input)
    value.nested[0]!.a = 9
    input.contributions.pop()
    expect(parsed.contributions).toHaveLength(3)
    expect(parsed.contributions[0]!.value).toEqual({
      nested: [{ a: 1, b: true }],
      nullable: null,
    })
    expect(Object.isFrozen(parsed)).toBe(true)
    expect(Object.isFrozen(parsed.contributions)).toBe(true)
    expect(Object.isFrozen(parsed.contributions[0])).toBe(true)
    expect(Object.isFrozen(parsed.contributions[0]!.value)).toBe(true)
    expect(() => {
      const stored = parsed.contributions[0]!.value as typeof value
      stored.nested[0]!.a = 5
    }).toThrow()
  })

  test('requires explicit value, version and unique local IDs, rejecting unknown fields', () => {
    for (const invalid of [
      { ...document(), formatVersion: 2 },
      { ...document(), pluginId: 'spoofed' },
      { ...document(), contributions: [{ id: 'main', kind: 'theme' }] },
      { ...document(), contributions: [contribution('Bad-ID')] },
      { ...document(), contributions: [contribution('main', {}, 'provider')] },
      {
        ...document(),
        contributions: [{ ...contribution(), pluginId: 'spoofed' }],
      },
      {
        ...document(),
        contributions: [contribution(), contribution('main', {}, 'locale')],
      },
    ])
      expectCode(() => parseExtensionContributions(invalid), 'INVALID_DOCUMENT')
    expect(
      parseExtensionContributions(document(null)).contributions[0]!.value
    ).toBeNull()
  })

  test('refuses executable, accessor, prototype and non-JSON data before reading it', () => {
    let reads = 0
    const accessor = Object.defineProperty({}, 'value', {
      enumerable: true,
      get: () => {
        reads++
        return 'secret'
      },
    })
    const toJson = {
      toJSON: () => {
        reads++
        return 'secret'
      },
    }
    const cycle: Record<string, unknown> = {}
    cycle.self = cycle
    const symbol = { [Symbol('hidden')]: true }
    const poison: unknown = JSON.parse('{"__proto__":{"polluted":true}}')
    for (const value of [
      accessor,
      toJson,
      cycle,
      symbol,
      poison,
      () => 'execute',
      undefined,
      Number.NaN,
      Number.POSITIVE_INFINITY,
      BigInt(1),
      new Date(),
      new Map(),
    ])
      expectCode(
        () =>
          parseExtensionContributions({
            formatVersion: 1,
            contributions: [{ id: 'main', kind: 'theme', value }],
          }),
        'INVALID_DOCUMENT'
      )
    const top = Object.defineProperty({}, 'formatVersion', {
      enumerable: true,
      get: () => {
        reads++
        return 1
      },
    })
    expectCode(() => parseExtensionContributions(top), 'INVALID_DOCUMENT')
    expect(reads).toBe(0)
    expect(Object.prototype).not.toHaveProperty('polluted')
  })

  test('enforces contribution count, UTF-8 value/document bytes and tree depth', () => {
    expectCode(
      () =>
        parseExtensionContributions({
          formatVersion: 1,
          contributions: Array.from(
            {
              length: extensionContributionLimits.maxContributionsPerOwner + 1,
            },
            (_, index) => contribution(`item-${index}`)
          ),
        }),
      'INVALID_DOCUMENT'
    )
    expectCode(
      () => parseExtensionContributions(document('中'.repeat(22_000))),
      'BUDGET_EXCEEDED'
    )
    expectCode(
      () =>
        parseExtensionContributions({
          formatVersion: 1,
          contributions: Array.from({ length: 5 }, (_, index) =>
            contribution(`item-${index}`, 'x'.repeat(60_000))
          ),
        }),
      'BUDGET_EXCEEDED'
    )
    let nested: unknown = 'leaf'
    for (let depth = 0; depth < 40; depth++) nested = [nested]
    expectCode(
      () => parseExtensionContributions(document(nested)),
      'INVALID_DOCUMENT'
    )
  })
})

describe('Host-owned extension contribution registry', () => {
  test('sorts snapshots deterministically and isolates identical local IDs', () => {
    const registry = new ExtensionContributionRegistry()
    const other = new ExtensionContributionRegistry()
    const first = {
      formatVersion: 1,
      contributions: [
        contribution('zulu', { b: 2, a: 1 }),
        contribution('main', null, 'locale'),
        contribution('alpha', true),
      ],
    }
    registry.replace(owner('z-plugin'), document())
    registry.replace(owner('a-plugin'), first)
    other.replace(owner('a-plugin'), first)
    other.replace(owner('z-plugin'), document())
    expect(registry.getSnapshot()).toEqual(other.getSnapshot())
    expect(registry.getSnapshot().map(item => item.key)).toEqual([
      'a-plugin:main',
      'a-plugin:alpha',
      'a-plugin:zulu',
      'z-plugin:main',
    ])
    expect(registry.getSnapshot()[0]!.owner).toEqual(owner('a-plugin'))
    expect(Object.isFrozen(registry.getSnapshot())).toBe(true)
    expect(Object.isFrozen(registry.getSnapshot()[0])).toBe(true)
    expect(Object.isFrozen(registry.getSnapshot()[0]!.owner)).toBe(true)
  })

  test('keeps snapshots stable for equal data, including payload property/input order', () => {
    const registry = new ExtensionContributionRegistry()
    let notifications = 0
    const unsubscribe = registry.subscribe(() => notifications++)
    registry.replace(owner(), {
      formatVersion: 1,
      contributions: [
        contribution('zulu', { b: 2, a: { z: 1, y: 2 } }),
        contribution('alpha', true),
      ],
    })
    const snapshot = registry.getSnapshot()
    registry.replace(owner(), {
      contributions: [
        contribution('alpha', true),
        contribution('zulu', { a: { y: 2, z: 1 }, b: 2 }),
      ],
      formatVersion: 1,
    })
    expect(registry.getSnapshot()).toBe(snapshot)
    expect(notifications).toBe(1)
    unsubscribe()
    registry.revoke(owner())
    expect(notifications).toBe(1)
  })

  test('rejects the entire replacement without altering snapshot or subscriptions', () => {
    const registry = new ExtensionContributionRegistry()
    registry.replace(owner(), document('original'))
    const snapshot = registry.getSnapshot()
    let notifications = 0
    registry.subscribe(() => notifications++)
    expectCode(
      () =>
        registry.replace(owner(), {
          formatVersion: 1,
          contributions: [
            contribution('new'),
            { ...contribution(), kind: 'invalid' },
          ],
        }),
      'INVALID_DOCUMENT'
    )
    expect(registry.getSnapshot()).toBe(snapshot)
    expect(notifications).toBe(0)
  })

  test('Host owner is cloned and checked before use; declared owners cannot override it', () => {
    const registry = new ExtensionContributionRegistry()
    const binding = { ...owner() }
    registry.replace(binding, document({ pluginId: 'victim', generation: 999 }))
    binding.pluginId = 'mutated'
    expect(registry.getSnapshot()[0]!.owner.pluginId).toBe('fixture')
    expect(registry.getSnapshot()[0]!.value).toEqual({
      pluginId: 'victim',
      generation: 999,
    })
    for (const invalid of [
      { ...owner(), pluginId: '../victim' },
      { ...owner(), generation: 0 },
      { ...owner(), generation: 1.5 },
      { ...owner(), generation: Number.MAX_SAFE_INTEGER + 1 },
      { ...owner(), version: 'v1.0.0' },
      { ...owner(), version: 'latest' },
      { ...owner(), extra: 'ignored' },
    ])
      expectCode(() => registry.replace(invalid, document()), 'INVALID_OWNER')
    let reads = 0
    const accessor = Object.defineProperty({ ...owner() }, 'pluginId', {
      enumerable: true,
      get: () => {
        reads++
        return 'victim'
      },
    })
    expectCode(() => registry.replace(accessor, document()), 'INVALID_OWNER')
    expect(reads).toBe(0)
  })

  test('replacement leases and owner generations protect newer contributions', () => {
    const registry = new ExtensionContributionRegistry()
    const stale = registry.replace(owner(), document('first'))
    const replacement = registry.replace(owner(), document('replacement'))
    stale()
    expect(registry.getSnapshot()[0]!.value).toBe('replacement')
    const newer = registry.replace(
      owner('fixture', 2, '2.0.0'),
      document('newer')
    )
    replacement()
    expect(registry.revoke(owner())).toBe(false)
    expectCode(() => registry.replace(owner(), document()), 'OWNER_EXPIRED')
    expectCode(
      () => registry.replace(owner('fixture', 2, '3.0.0'), document()),
      'OWNER_EXPIRED'
    )
    newer()
    newer()
    expect(registry.getSnapshot()).toHaveLength(0)
    expectCode(() => registry.replace(owner(), document()), 'OWNER_EXPIRED')
    // Disable/re-enable can reuse one loaded generation with a fresh lease.
    registry.replace(owner('fixture', 2, '2.0.0'), document('reenabled'))
    newer()
    expect(registry.getSnapshot()[0]!.value).toBe('reenabled')
    registry.replace(owner('fixture', 3, '3.0.0'), document('upgraded'))
    expect(registry.getSnapshot()[0]!.owner.generation).toBe(3)
  })

  test('Host issues monotonic contribution epochs independently of plugin load counters', () => {
    const registry = new ExtensionContributionRegistry()
    const first = registry.createOwner('fixture', '1.0.0')
    registry.replace(first, document())()
    const reinstalled = registry.createOwner('fixture', '1.0.0')
    expect(reinstalled.generation).toBeGreaterThan(first.generation)
    registry.replace(reinstalled, document('reinstalled'))
    expectCode(() => registry.replace(first, document()), 'OWNER_EXPIRED')
    expect(registry.revoke(first)).toBe(false)
    expect(registry.getSnapshot()[0]!.value).toBe('reinstalled')
  })

  test('enforces aggregate count, bytes and retained owner metadata budgets atomically', () => {
    const counted = new ExtensionContributionRegistry()
    const many = {
      formatVersion: 1,
      contributions: Array.from({ length: 128 }, (_, index) =>
        contribution(`item-${index}`)
      ),
    }
    for (let index = 0; index < 8; index++)
      counted.replace(owner(`plugin-${index}`), many)
    const countSnapshot = counted.getSnapshot()
    expectCode(
      () => counted.replace(owner('overflow'), document()),
      'BUDGET_EXCEEDED'
    )
    expect(counted.getSnapshot()).toBe(countSnapshot)
    counted.replace(owner('plugin-0'), document())
    counted.replace(owner('overflow'), document())

    const sized = new ExtensionContributionRegistry()
    const large = {
      formatVersion: 1,
      contributions: Array.from({ length: 3 }, (_, index) =>
        contribution(`item-${index}`, 'x'.repeat(60_000))
      ),
    }
    for (let index = 0; index < 5; index++)
      sized.replace(owner(`plugin-${index}`), large)
    const byteSnapshot = sized.getSnapshot()
    expectCode(() => sized.replace(owner('overflow'), large), 'BUDGET_EXCEEDED')
    expect(sized.getSnapshot()).toBe(byteSnapshot)
    sized.revoke(owner('plugin-0'))
    sized.replace(owner('overflow'), large)

    const retained = new ExtensionContributionRegistry()
    for (let index = 0; index < extensionContributionLimits.maxOwners; index++)
      retained.replace(owner(`plugin-${index}`), document())()
    const emptySnapshot = retained.getSnapshot()
    expectCode(
      () => retained.replace(owner('overflow'), document()),
      'BUDGET_EXCEEDED'
    )
    expect(retained.getSnapshot()).toBe(emptySnapshot)
    retained.replace(owner('plugin-0', 2), document())
    expect(retained.getSnapshot()).toHaveLength(1)
  })

  test('observer failures cannot undo replacements or block other observers', () => {
    const registry = new ExtensionContributionRegistry()
    let observed = 0
    registry.subscribe(() => {
      throw new Error('observer failed')
    })
    registry.subscribe(() => observed++)
    const dispose = registry.replace(owner(), document())
    expect(observed).toBe(1)
    expect(registry.getSnapshot()).toHaveLength(1)
    dispose()
    expect(observed).toBe(2)
    expect(registry.getSnapshot()).toHaveLength(0)
  })
})
