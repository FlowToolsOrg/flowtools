import type { ResourceOperation, ResourceResponse } from './bindings'
import type { ResourceCapability, ResourceInfo } from '@flowtools/sdk/types'

import { isJsonValue } from '@flowtools/sdk/manifest'

import {
  validateResourceOperation,
  validateResourceResponse,
} from './wire-validators.cjs'

export function createResourceCapability(
  invoke: (
    operation: ResourceOperation,
    signal?: AbortSignal
  ) => Promise<unknown>
): ResourceCapability {
  const call = async (
    operation: ResourceOperation,
    signal?: AbortSignal
  ): Promise<ResourceResponse> => {
    if (signal?.aborted) throw new Error('ABORTED')
    if (!isJsonValue(operation) || !validateResourceOperation(operation))
      throw new Error('INPUT_INVALID')
    const value = await invoke(operation, signal)
    if (signal?.aborted) throw new Error('ABORTED')
    if (!isJsonValue(value) || !validateResourceResponse(value))
      throw new Error('INVALID_RESPONSE')
    return value
  }
  const info = async (
    operation: ResourceOperation,
    signal?: AbortSignal
  ): Promise<ResourceInfo> => {
    const result = await call(operation, signal)
    if (result.kind !== 'info') throw new Error('INVALID_RESPONSE')
    return result.value
  }
  return {
    resolve: (slot, options) =>
      info({ operation: 'resolve', parameters: { slot } }, options?.signal),
    stat: (ref, options) =>
      info({ operation: 'stat', parameters: { ref } }, options?.signal),
    async read(ref, request) {
      const result = await call(
        {
          operation: 'read',
          parameters: {
            ref,
            revision: request.revision,
            offset: request.offset,
            maxBytes: request.maxBytes,
          },
        },
        request.signal
      )
      if (result.kind !== 'chunk' || result.value.base64.length > 87384)
        throw new Error('INVALID_RESPONSE')
      const raw = atob(result.value.base64),
        bytes = Uint8Array.from(raw, c => c.charCodeAt(0))
      if (
        bytes.length > request.maxBytes ||
        result.value.nextOffset !== request.offset + bytes.length ||
        btoa(raw) !== result.value.base64
      )
        throw new Error('INVALID_RESPONSE')
      return {
        bytes,
        nextOffset: result.value.nextOffset,
        eof: result.value.eof,
      }
    },
    async list(ref, request) {
      const result = await call(
        {
          operation: 'list',
          parameters: {
            ref,
            revision: request.revision,
            cursor: request.cursor ?? null,
            limit: request.limit,
          },
        },
        request.signal
      )
      if (result.kind !== 'page' || result.value.entries.length > request.limit)
        throw new Error('INVALID_RESPONSE')
      return result.value
    },
    async release(ref, options) {
      const result = await call(
        { operation: 'release', parameters: { ref } },
        options?.signal
      )
      if (result.kind !== 'released') throw new Error('INVALID_RESPONSE')
    },
  }
}
