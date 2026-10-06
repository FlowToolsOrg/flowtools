import type { Call, StorageAction } from '@flowtools/runtime-client'
import type { DataCapability } from '@flowtools/sdk/data'

import {
  PluginDataClient,
  RuntimeClient,
  RuntimeClientError,
  decodeStorageReport,
} from '@flowtools/runtime-client'

import { commands } from '../utils/bindings'

export async function connectManagedRuntime() {
  let sessionId: string | undefined
  const client = new RuntimeClient({
    async exchange(request) {
      const response = await commands.managedRuntime(request)
      if (response.status === 'error')
        throw new RuntimeClientError(response.error.code)
      if (response.data.outcome.type === 'session')
        sessionId = response.data.outcome.data.sessionId
      return response.data
    },
    close() {
      if (sessionId) {
        const id = sessionId
        sessionId = undefined
        void commands.managedRuntimeDisconnect(id).catch(() => {})
      }
    },
  })
  try {
    await client.connect('native-bootstrap')
    return client
  } catch (error) {
    client.close()
    throw error
  }
}
export async function controlManagedRuntime(call: Call, initialize = false) {
  const response = await commands.managedRuntimeControl(call, initialize)
  if (response.status === 'error')
    throw new RuntimeClientError(response.error.code)
  return response.data
}
export async function storageManagedRuntime(action: StorageAction) {
  const response = await commands.managedRuntimeStorage(action)
  if (response.status === 'error')
    throw new RuntimeClientError(response.error.code)
  return decodeStorageReport(response.data)
}
export function managedPluginData(
  pluginId: string,
  connect: () => Promise<RuntimeClient> = connectManagedRuntime
): { data: DataCapability; close: () => void } {
  let pending: Promise<RuntimeClient> | undefined
  const acquire = () => {
    if (!pending) {
      const owned = connect()
      pending = owned
      void owned.catch(() => {
        if (pending === owned) pending = undefined
      })
    }
    return pending
  }
  const release = (owned: Promise<RuntimeClient>) => {
    if (pending === owned) pending = undefined
    void owned.then(
      client => client.close(),
      () => {}
    )
  }
  const failed = (owned: Promise<RuntimeClient>, error: unknown) => {
    if (
      error instanceof RuntimeClientError &&
      ['RUNTIME_DISCONNECTED', 'SESSION_INVALID', 'INVALID_RESPONSE'].includes(
        error.code
      )
    )
      release(owned)
  }
  const run = async <T>(operation: (data: PluginDataClient) => Promise<T>) => {
    const owned = acquire()
    try {
      return await operation(new PluginDataClient(await owned, pluginId))
    } catch (error) {
      failed(owned, error)
      throw error
    }
  }
  return {
    data: {
      read: key => run(data => data.read(key)),
      write: mutation => run(data => data.write(mutation)),
      transaction: mutations => run(data => data.transaction(mutations)),
      async *watch(key, signal) {
        const owned = acquire()
        try {
          yield* new PluginDataClient(await owned, pluginId).watch(key, signal)
        } catch (error) {
          failed(owned, error)
          throw error
        }
      },
    },
    close() {
      if (pending) release(pending)
    },
  }
}
