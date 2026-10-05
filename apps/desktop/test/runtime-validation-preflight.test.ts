import { expect, test } from 'bun:test'
import { Buffer } from 'node:buffer'

import {
  assertRuntimeValidationPreflight,
  runtimePreflightMarker,
} from '../../ui-test/scripts/runtime-validation-preflight'

const expected = [
  'com.flowtools.g2-validation-20261004',
  'http://127.0.0.1:1420/',
  'FlowTools G2 Runtime validation',
] as const

test('an identifier literal in a stale binary never authorizes even a probe', () => {
  let probes = 0
  expect(() =>
    assertRuntimeValidationPreflight(Buffer.from(expected[0]), expected, () => {
      probes++
      return [runtimePreflightMarker, ...expected].join('\n')
    })
  ).toThrow('Unsupported validation preflight')
  expect(probes).toBe(0)
})

test('actual compiled default identity, other origin, title or probe version refuse startup', () => {
  const supported = Buffer.from(runtimePreflightMarker)
  for (const metadata of [
    ['wrong-version', ...expected],
    [runtimePreflightMarker, 'com.hmsuiji.desktop', ...expected.slice(1)],
    [
      runtimePreflightMarker,
      expected[0],
      'http://127.0.0.1:9999/',
      expected[2],
    ],
    [runtimePreflightMarker, expected[0], expected[1], 'desktop'],
    [runtimePreflightMarker, ...expected, 'extra'],
  ]) {
    expect(() =>
      assertRuntimeValidationPreflight(supported, expected, () =>
        metadata.join('\n')
      )
    ).toThrow()
  }
  expect(() =>
    assertRuntimeValidationPreflight(supported, expected, () =>
      [runtimePreflightMarker, ...expected].join('\r\n')
    )
  ).not.toThrow()
})
