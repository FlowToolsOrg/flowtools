import { expect, test } from 'bun:test'

import {
  compatibilityEvidenceStatusSchema,
  pluginMaturitySchema,
  resolvePluginMaturity,
} from '../src/types/maturity'

test('maturity has one vocabulary and missing metadata is only prototype', () => {
  for (const value of [
    'prototype',
    'experimental',
    'beta',
    'production',
  ] as const) {
    expect(pluginMaturitySchema.parse(value)).toBe(value)
  }
  expect(resolvePluginMaturity(undefined)).toBe('prototype')
  for (const value of [
    'stable',
    'deprecated',
    'indexed',
    'production-certified',
    null,
    true,
  ]) {
    expect(pluginMaturitySchema.safeParse(value).success).toBe(false)
  }
})

test('compatibility evidence cannot be confused with product maturity', () => {
  for (const value of [
    'indexed',
    'entry-resolved',
    'api-verified',
    'production-certified',
  ] as const) {
    expect(compatibilityEvidenceStatusSchema.parse(value)).toBe(value)
  }
  for (const value of [
    'prototype',
    'beta',
    'production',
    'compatible',
    'verified',
    undefined,
  ]) {
    expect(compatibilityEvidenceStatusSchema.safeParse(value).success).toBe(
      false
    )
  }
})
