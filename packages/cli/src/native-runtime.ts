import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { regularFile } from './regular-file'

declare const __FLOWTOOLS_NATIVE_BUILD_PATH__: string
declare const __FLOWTOOLS_BUNDLE_RUNTIME_LOCK__: {
  path: string
  sha256: string
} | null

export function userProfile(explicit?: string): string {
  const path =
    explicit ??
    (process.env.LOCALAPPDATA &&
      join(process.env.LOCALAPPDATA, 'FlowToolsRuntimeV1'))
  if (!path || !/^[a-z]:[\\/]/i.test(path)) throw new Error('SETUP_REQUIRED')
  return resolve(path)
}

export async function runtimeExecutable(): Promise<string> {
  const lock =
    typeof __FLOWTOOLS_BUNDLE_RUNTIME_LOCK__ === 'object'
      ? __FLOWTOOLS_BUNDLE_RUNTIME_LOCK__
      : null
  const candidate = lock
    ? fileURLToPath(new URL('../../../' + lock.path, import.meta.url))
    : typeof __FLOWTOOLS_NATIVE_BUILD_PATH__ === 'string'
      ? __FLOWTOOLS_NATIVE_BUILD_PATH__
      : fileURLToPath(
          new URL(
            '../../../target/debug/flowtools-runtime.exe',
            import.meta.url
          )
        )
  const path = await regularFile(candidate)
  if (
    lock &&
    createHash('sha256')
      .update(await readFile(path))
      .digest('hex') !== lock.sha256
  )
    throw new Error('BUNDLE_INTEGRITY_FAILED')
  return path
}

export async function readBootstrap(profile: string) {
  const path = await regularFile(join(profile, 'bootstrap.json'), 2048)
  const value: unknown = JSON.parse(await readFile(path, 'utf8'))
  if (
    !value ||
    typeof value !== 'object' ||
    !('formatVersion' in value) ||
    value.formatVersion !== 1
  )
    throw new Error('SETUP_REQUIRED')
  const record = value as Record<string, unknown>
  for (const field of ['cliToken', 'desktopToken', 'managementToken'] as const)
    if (
      typeof record[field] !== 'string' ||
      !/^[a-f0-9]{64}$/.test(record[field])
    )
      throw new Error('SETUP_REQUIRED')
  return value as {
    formatVersion: 1
    cliToken: string
    desktopToken: string
    managementToken: string
  }
}
