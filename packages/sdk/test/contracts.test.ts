import { describe, expect, test } from 'bun:test'

import { definePlugin } from '../src/compositions/definePlugin'
import { PLUGIN_MARKER } from '../src/constants'
import { result } from '../src/result'
import {
  MissingCapabilityError,
  MissingPluginStoreError,
  MissingRuntimeContextError,
  createMissingCapabilityError,
  createMissingPluginStoreError,
  createMissingRuntimeContextError,
} from '../src/runtime/errors'
import { pickCapability } from '../src/utils/capability'

describe('result helpers', () => {
  test('builds text and json results', () => {
    expect(result.text('ready')).toEqual({
      type: 'text',
      text: 'ready',
    })

    const value = { result: ['one', 'two'], count: 2 }
    const output = result.json(value)

    expect(output).toEqual({
      type: 'json',
      value,
    })
    expect(output.value).toBe(value)
  })

  test('builds table results without changing input references', () => {
    const columns = [{ key: 'name', title: 'Name' }] as const
    const rows = [{ name: 'FlowTools' }] as const
    const output = result.table(columns, rows)

    expect(output).toEqual({ type: 'table', columns, rows })
    expect(output.columns).toBe(columns)
    expect(output.rows).toBe(rows)
  })

  test('builds open and multi results', () => {
    const first = result.text('first')
    const second = result.open('https://example.com')
    const items = [first, second] as const

    expect(second).toEqual({
      type: 'open',
      target: 'https://example.com',
    })
    const output = result.multi(items)
    expect(output).toEqual({ type: 'multi', items })
    expect(output.items).toBe(items)
  })
})

describe('definePlugin', () => {
  test('marks and returns the original plugin object', () => {
    const plugin = {
      type: 'tool' as const,
      meta: {
        id: 'contract-plugin',
        name: 'Contract Plugin',
        version: '1.0.0',
        permissions: ['storage'] as const,
      },
      run: () => result.text('ok'),
    }

    const defined = definePlugin(plugin)

    expect(Object.is(defined, plugin)).toBe(true)
    expect(defined[PLUGIN_MARKER]).toBe(true)
  })
})

describe('pickCapability', () => {
  test('does not construct a capability when permission is denied', () => {
    let factoryCalls = 0
    const capability = pickCapability('storage', new Set(), () => {
      factoryCalls += 1
      return { read: true }
    })

    expect(capability).toBeUndefined()
    expect(factoryCalls).toBe(0)
  })

  test('constructs an allowed capability exactly once', () => {
    let factoryCalls = 0
    const expected = { read: true }
    const capability = pickCapability(
      'storage',
      new Set(['storage'] as const),
      () => {
        factoryCalls += 1
        return expected
      }
    )

    expect(capability).toBe(expected)
    expect(factoryCalls).toBe(1)
  })
})

describe('runtime errors', () => {
  test('uses stable error names and SDK-prefixed messages', () => {
    const runtimeError = new MissingRuntimeContextError()
    const capabilityError = new MissingCapabilityError('clipboard')
    const storeError = new MissingPluginStoreError()

    expect(runtimeError.name).toBe('MissingRuntimeContextError')
    expect(runtimeError.message).toContain('[flowtools-sdk]')
    expect(runtimeError.message).toContain('Runtime context is missing')

    expect(capabilityError.name).toBe('MissingCapabilityError')
    expect(capabilityError.message).toContain('[flowtools-sdk]')
    expect(capabilityError.message).toContain('"clipboard"')

    expect(storeError.name).toBe('MissingPluginStoreError')
    expect(storeError.message).toContain('[flowtools-sdk]')
    expect(storeError.message).toContain('Plugin store is unavailable')
  })

  test('factory functions return their public error classes', () => {
    expect(createMissingRuntimeContextError()).toBeInstanceOf(
      MissingRuntimeContextError
    )
    expect(createMissingCapabilityError('fs')).toBeInstanceOf(
      MissingCapabilityError
    )
    expect(createMissingPluginStoreError()).toBeInstanceOf(
      MissingPluginStoreError
    )
  })
})
