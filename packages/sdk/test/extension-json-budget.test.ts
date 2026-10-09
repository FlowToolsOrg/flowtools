import { describe, expect, spyOn, test } from 'bun:test'

import { boundedJsonBytes } from '../src/extensions/json-budget'
import { parseExtensionContributions } from '../src/extensions/schema'
import { ExtensionContributionError } from '../src/extensions/types'
import { isJsonValue } from '../src/manifest/json-schema'

const examples: readonly (readonly [string, unknown])[] = [
  ['null', null],
  ['booleans', [true, false]],
  ['empty containers', [{}, [], '']],
  [
    'finite number spellings',
    [-0, 1, -12.5, 1e-7, 1e21, Number.MIN_VALUE, Number.MAX_VALUE],
  ],
  ['ASCII and JSON escapes', '"\\/\b\t\n\f\r\u0000\u000b\u001f'],
  ['multibyte Unicode', 'é汉😀\u2028\u2029'],
  ['lone and paired surrogates', '\ud800x\udfff\ud800\ud800\udc00'],
  ['escaped object keys', { '"\\\n汉😀\ud800': [null, { value: 'é' }] }],
]

describe('bounded JSON byte counting', () => {
  for (const [name, value] of examples) {
    test(`matches actual UTF-8 JSON bytes for ${name}`, () => {
      const expected = new TextEncoder().encode(JSON.stringify(value)).length
      expect(isJsonValue(value)).toBe(true)
      expect(boundedJsonBytes(value, expected)).toBe(expected)
      expect(() => boundedJsonBytes(value, expected - 1)).toThrow(
        'BUDGET_EXCEEDED'
      )
    })
  }

  test('accepts exactly the contribution and document byte limits', () => {
    for (const limit of [65_536, 262_144]) {
      const value = '汉'.repeat(Math.floor((limit - 2) / 3))
      const padding = limit - 2 - value.length * 3
      const exact = value + 'x'.repeat(padding)
      expect(boundedJsonBytes(exact, limit)).toBe(limit)
      expect(() => boundedJsonBytes(exact + 'x', limit)).toThrow(
        'BUDGET_EXCEEDED'
      )
    }
  })

  const checks: readonly (readonly [string, (value: unknown) => unknown])[] = [
    ['counter', value => boundedJsonBytes(value, 262_144)],
    [
      'public parser',
      value =>
        parseExtensionContributions({
          formatVersion: 1,
          contributions: [{ id: 'large', kind: 'theme', value }],
        }),
    ],
  ]
  for (const [name, check] of checks) {
    test(`${name} rejects repeated shared strings before serialization`, () => {
      // The input retains one 32KiB string; its JSON would exceed one GiB.
      const shared = 'x'.repeat(32_768)
      const value: unknown = Array.from({ length: 32_768 }, () => shared)
      expect(isJsonValue(value)).toBe(true)
      const stringify = spyOn(JSON, 'stringify').mockImplementation(() => {
        throw new Error('Document serialization must not run before rejection')
      })
      let error: unknown
      try {
        check(value)
      } catch (caught) {
        error = caught
      } finally {
        stringify.mockRestore()
      }
      expect(error).toBeInstanceOf(ExtensionContributionError)
      expect((error as ExtensionContributionError).code).toBe('BUDGET_EXCEEDED')
    })
  }
})
