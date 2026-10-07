import type {
  Request,
  Response,
  StorageAction,
  StorageReport,
} from './bindings'

import { isJsonValue } from '@flowtools/sdk/manifest'
import Ajv2020 from 'ajv/dist/2020'

import schemas from './wire-schema.json'

export const MAX_FRAME_BYTES = 1_048_576
const ajv = new Ajv2020({ strict: false, allErrors: false })
ajv.addFormat('double', { type: 'number', validate: Number.isFinite })
ajv.addFormat('uint16', {
  type: 'number',
  validate: value => Number.isInteger(value) && value >= 0 && value <= 65535,
})
ajv.addFormat('uint32', {
  type: 'number',
  validate: value =>
    Number.isInteger(value) && value >= 0 && value <= 4294967295,
})
const validateRequest = ajv.compile<Request>(schemas.request)
const validateResponse = ajv.compile<Response>(schemas.response)
const validateStorageAction = ajv.compile<StorageAction>(schemas.storageAction)
const validateStorageReport = ajv.compile<StorageReport>(schemas.storageReport)

export function encodeStorageAction(value: unknown): string {
  if (!isJsonValue(value) || !validateStorageAction(value))
    throw new Error('INVALID_REQUEST')
  return JSON.stringify(value)
}
export function decodeStorageReport(value: unknown): StorageReport {
  if (
    !isJsonValue(value) ||
    !validateStorageReport(value) ||
    value.formatVersion !== 1 ||
    value.backups.length > 64
  )
    throw new Error('INVALID_RESPONSE')
  return value
}

export function encodeRequest(request: unknown): string {
  if (!isJsonValue(request) || !validateRequest(request))
    throw new Error('INVALID_REQUEST')
  const value = JSON.stringify(request)
  if (new TextEncoder().encode(value).length > MAX_FRAME_BYTES)
    throw new Error('FRAME_TOO_LARGE')
  return value
}

export function decodeResponse(value: unknown): Response {
  if (!isJsonValue(value) || !validateResponse(value))
    throw new Error('INVALID_RESPONSE')
  if (value.outcome.type === 'job') {
    const job = value.outcome.data
    const terminal = [
      'succeeded',
      'failed',
      'cancelled',
      'interrupted',
    ].includes(job.state)
    const result = job.result
    if (job.formatVersion !== 1 || terminal !== (result !== null))
      throw new Error('INVALID_RESPONSE')
    if (
      result &&
      (result.formatVersion !== 1 ||
        result.runId !== job.runId ||
        result.pluginId !== job.pluginId ||
        result.pluginVersion !== job.packageVersion ||
        typeof result.startedAt !== 'number' ||
        typeof result.finishedAt !== 'number' ||
        result.finishedAt < result.startedAt ||
        result.durationMs !== result.finishedAt - result.startedAt ||
        result.success !== (job.state === 'succeeded') ||
        (result.success
          ? !('data' in result) || 'error' in result
          : !('error' in result) || 'data' in result))
    )
      throw new Error('INVALID_RESPONSE')
  }
  return value
}
