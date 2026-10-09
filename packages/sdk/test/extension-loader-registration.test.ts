import type { RegisterHooksOptions } from 'node:module'

import { expect, test } from 'bun:test'

import {
  registerDependencyGuard,
  resolve,
} from './extension-dependency-guard.mjs'

test('uses synchronous hooks without calling the deprecated loader API', () => {
  let registered: RegisterHooksOptions | undefined
  registerDependencyGuard({
    register() {
      throw new Error('Deprecated loader registration must not run')
    },
    registerHooks(options) {
      registered = options
      return { deregister() {} }
    },
  })
  expect(registered).toEqual({ resolve })
})

test('Node 20 registers the real ESM guard when synchronous hooks are absent', () => {
  const registered: (string | URL)[] = []
  registerDependencyGuard({
    register(specifier) {
      registered.push(specifier)
    },
  })
  expect(registered).toEqual([
    new URL('./extension-dependency-guard.mjs', import.meta.url),
  ])
})
