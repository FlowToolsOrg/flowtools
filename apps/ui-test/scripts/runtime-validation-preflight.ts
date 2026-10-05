import assert from 'node:assert/strict'
import { Buffer } from 'node:buffer'

export const runtimePreflightMarker =
  'flowtools-runtime-validation-preflight-v1'

/** A supported probe exits before Tauri Builder/plugins/database initialization. */
export function assertRuntimeValidationPreflight(
  binary: Buffer,
  expected: readonly string[],
  probe: () => string
): void {
  assert.ok(
    binary.includes(Buffer.from(runtimePreflightMarker)),
    'Unsupported validation preflight; rebuild the dedicated native artifact'
  )
  assert.deepEqual(probe().trim().split(/\r?\n/), [
    runtimePreflightMarker,
    ...expected,
  ])
}
