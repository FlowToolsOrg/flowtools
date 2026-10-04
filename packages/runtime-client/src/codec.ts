import type { Request, Response } from './bindings'

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

export function encodeRequest(request: Request): string {
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
  return value
}
