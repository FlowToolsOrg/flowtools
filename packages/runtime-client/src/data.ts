import type { LegacyImport } from './bindings'
import type {
  DataCapability,
  DataMutation,
  DataSnapshot,
} from '@flowtools/sdk/data'

import { RuntimeClient, RuntimeClientError } from './client'

/** Host-client facade. Runtime resolves the selector and owns the namespace. */
export class PluginDataClient implements DataCapability {
  constructor(
    private client: RuntimeClient,
    private pluginId: string
  ) {}
  async read(key: string): Promise<DataSnapshot> {
    const result = await this.client.call({
      method: 'data.read',
      payload: { pluginId: this.pluginId, key },
    })
    if (result.type !== 'data' || result.data.key !== key)
      throw new RuntimeClientError('INVALID_RESPONSE')
    return result.data
  }
  async write(mutation: DataMutation): Promise<DataSnapshot> {
    const result = await this.client.call({
      method: 'data.write',
      payload: { pluginId: this.pluginId, mutation },
    })
    if (
      result.type !== 'data' ||
      result.data.key !== mutation.key ||
      result.data.revision !== mutation.expectedRevision + 1
    )
      throw new RuntimeClientError('INVALID_RESPONSE')
    return result.data
  }
  async transaction(mutations: DataMutation[]): Promise<DataSnapshot[]> {
    const result = await this.client.call({
      method: 'data.transaction',
      payload: { pluginId: this.pluginId, mutations },
    })
    if (
      result.type !== 'data-batch' ||
      result.data.length !== mutations.length ||
      result.data.some(
        (item, index) =>
          item.key !== mutations[index]?.key ||
          item.revision !== mutations[index]!.expectedRevision + 1
      )
    )
      throw new RuntimeClientError('INVALID_RESPONSE')
    return result.data
  }
  async importLegacy(imported: LegacyImport): Promise<DataSnapshot> {
    const result = await this.client.call({
      method: 'data.import-legacy',
      payload: { pluginId: this.pluginId, import: imported },
    })
    if (result.type !== 'data' || result.data.key !== 'todos')
      throw new RuntimeClientError('INVALID_RESPONSE')
    return result.data
  }
  async *watch(key: string, signal: AbortSignal): AsyncGenerator<DataSnapshot> {
    let revision = -1
    while (!signal.aborted) {
      const snapshot = await this.read(key)
      if (signal.aborted) return
      if (snapshot.revision > revision) {
        revision = snapshot.revision
        yield snapshot
      }
      await new Promise<void>(resolve => setTimeout(resolve, 100))
    }
  }
}
