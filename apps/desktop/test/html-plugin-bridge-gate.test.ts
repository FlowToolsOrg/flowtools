import type { HtmlPluginBridgeCommand } from '../src/runtime/html-plugin-bridge'
import type { PluginRuntimeContextValue } from '@flowtools/sdk/types'

import { expect, test } from 'bun:test'

import { ExternalCodeDisabledError } from '@flowtools/sdk'

import {
  createHtmlPluginBridgeScript,
  handleHtmlPluginBridgeRequest,
  injectHtmlPluginBridge,
} from '../src/runtime/html-plugin-bridge'

function commandFixture() {
  let reads = 0
  const command: HtmlPluginBridgeCommand = {
    id: 'unknown:open',
    title: 'Untrusted fixture',
    type: 'open',
    pluginId: 'unknown',
    pluginName: 'Untrusted fixture',
    pluginType: 'app',
    permissions: ['native', 'fs', 'db'],
    preload: 'untrusted-preload.js',
  }
  Object.defineProperty(command, 'pluginId', {
    get() {
      reads++
      return 'unknown'
    },
  })
  return { command, reads: () => reads }
}

test('ordinary HTML bridge cannot generate a legacy script from claimed permissions', () => {
  const fixture = commandFixture()
  expect(() => createHtmlPluginBridgeScript(fixture.command)).toThrow(
    ExternalCodeDisabledError
  )
  expect(fixture.reads()).toBe(0)
})

test('ordinary HTML bridge cannot inject HTML or a preload script', () => {
  const fixture = commandFixture()
  expect(() =>
    injectHtmlPluginBridge(
      '<html><head></head></html>',
      fixture.command,
      'https://fixture.invalid/'
    )
  ).toThrow(ExternalCodeDisabledError)
  expect(fixture.reads()).toBe(0)
})

for (const method of [
  'ui.closePanel',
  'ui.toast',
  'clipboard.readText',
  'native.invoke',
  'fs.readFile',
  'db.query',
]) {
  test(`ordinary HTML bridge rejects ${method} before reading caller mode or payload`, async () => {
    let reads = 0
    let effects = 0
    const context: PluginRuntimeContextValue = {
      env: {
        pluginId: 'unknown',
        pluginType: 'app',
        platform: 'desktop',
        mode: 'development',
      },
      ui: {
        toast: () => {
          effects++
        },
        openPanel: () => {
          effects++
        },
        closePanel: () => {
          effects++
        },
      },
      utils: { now: () => 0 },
    }
    const request = {
      type: 'flowtools:html-plugin-call' as const,
      id: 'fixture',
      method,
      get payload() {
        reads++
        return {
          certified: true,
          granted: true,
          mode: 'development',
          command: 'unsafe',
          sql: 'select 1',
        }
      },
    }
    await handleHtmlPluginBridgeRequest(context, request).then(
      () => {
        throw new Error('Expected bridge rejection')
      },
      (error: unknown) =>
        expect(error).toMatchObject({ code: 'EXTERNAL_CODE_DISABLED' })
    )
    expect(reads).toBe(0)
    expect(effects).toBe(0)
  })
}
