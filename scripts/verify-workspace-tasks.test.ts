/// <reference types="bun-types" />

import { expect, test } from 'bun:test'

import { validateWorkspace } from './verify-workspace-tasks'

const scripts = {
  build: 'tsdown',
  lint: 'oxlint',
  'check-types': 'tsgo --noEmit',
  test: 'bun test test',
}

test('accepts supported nonempty test runners', () => {
  for (const runner of ['bun test test', 'cargo test --lib', 'vitest run']) {
    expect(
      validateWorkspace('fixture', {
        scripts: { ...scripts, test: runner },
      })
    ).toEqual([])
  }
})

test('rejects unknown runners and empty-test success flag spellings', () => {
  for (const runner of [
    'echo deprecated',
    'vitest run --pass-with-no-tests',
    'vitest run --passWithNoTests',
    'bun test --pass_with_no_tests',
  ]) {
    expect(
      validateWorkspace('fixture', {
        scripts: { ...scripts, test: runner },
      }).length
    ).toBeGreaterThan(0)
  }
})

test('keeps lint read-only and requires the standard type-check name', () => {
  const errors = validateWorkspace('fixture', {
    scripts: {
      ...scripts,
      lint: 'oxlint --fix',
      'check:types': 'tsgo --noEmit',
    },
  })
  expect(errors.some(error => error.includes('lint must be read-only'))).toBe(
    true
  )
  expect(errors.some(error => error.includes('use "check-types"'))).toBe(true)
})
