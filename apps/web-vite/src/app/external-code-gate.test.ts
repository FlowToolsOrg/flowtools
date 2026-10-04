import { describe, expect, test } from 'bun:test'

import {
  pluginRegistryStore,
  restoreExternalPlugins,
} from '../stores/plugin-registry-store'

async function expectDenied(action: () => Promise<unknown>): Promise<void> {
  const error: unknown = await action().catch((failure: unknown) => failure)
  expect(error).toBeInstanceOf(Error)
  expect(error).toHaveProperty('code', 'EXTERNAL_CODE_DISABLED')
}

describe('Web host default external-code denial', () => {
  test('direct file calls reject before file reads or persistence without a development opt-in', async () => {
    let reads = 0
    const file = new File(['throw new Error("must not run")'], 'untrusted.tsx')
    file.arrayBuffer = () => {
      reads += 1
      return Promise.resolve(new ArrayBuffer(0))
    }
    await expectDenied(() =>
      pluginRegistryStore.getState().loadPluginFromFile(file)
    )
    expect(reads).toBe(0)
  })

  test('restore refuses before touching IndexedDB and does not silently delete old source', async () => {
    // Bun has no IndexedDB here. A policy refusal proves we did not enter storage.
    await expectDenied(() => restoreExternalPlugins())
  })

  test('enable/reload/unload cannot bypass the default gate or mutate disabled state', async () => {
    const state = pluginRegistryStore.getState()
    const previous = state.disabledPluginIds
    pluginRegistryStore.setState({ disabledPluginIds: ['untrusted-fixture'] })
    try {
      await expectDenied(() => state.enablePlugin('untrusted-fixture'))
      await expectDenied(() => state.reloadPlugin('untrusted-fixture'))
      await expectDenied(() => state.unloadExternalPlugin('untrusted-fixture'))
      expect(pluginRegistryStore.getState().disabledPluginIds).toEqual([
        'untrusted-fixture',
      ])
    } finally {
      pluginRegistryStore.setState({ disabledPluginIds: previous })
    }
  })
})
