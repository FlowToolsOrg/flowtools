/// <reference types="bun-types" />

import { expect, test } from 'bun:test'

import { assertRustTestsPresent, countRustTests } from './rust-test-inventory'

test('counts actual Rust tests without counting benchmarks or summaries', () => {
  const output =
    'dto::tests::valid: test\r\nrepository::roundtrip: test\r\n' +
    'performance: benchmark\r\n2 tests, 1 benchmark\r\n'
  expect(countRustTests(output)).toBe(2)
  expect(assertRustTestsPresent(output)).toBe(2)
})

test('rejects zero collected Rust tests', () => {
  expect(() => assertRustTestsPresent('0 tests, 0 benchmarks\n')).toThrow(
    'Desktop Rust test inventory is empty'
  )
})
