import type { StorageAction } from '@flowtools/runtime-client'
import type { Command } from 'commander'

import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

import {
  decodeStorageReport,
  encodeStorageAction,
} from '@flowtools/runtime-client'

import { runtimeExecutable, userProfile } from './native-runtime'

export function addStorageCommands(runtime: Command) {
  const storage = runtime
    .command('storage')
    .description('Offline private backup and recovery; stop Runtime first')
  for (const name of ['list', 'create', 'restore', 'retry'] as const) {
    storage
      .command(name === 'restore' ? 'restore <backup-id>' : name)
      .option('--profile <directory>')
      .option('--confirm', 'Explicitly approve rollback and revoke all grants')
      .action(
        async (
          idOrOptions: string | { profile?: string; confirm?: boolean },
          supplied?: { profile?: string; confirm?: boolean }
        ) => {
          const options =
            typeof idOrOptions === 'string' ? supplied! : idOrOptions
          if ((name === 'restore' || name === 'retry') && !options.confirm)
            throw new Error('APPROVAL_REQUIRED')
          const action: StorageAction =
            name === 'restore'
              ? {
                  operation: name,
                  parameters: {
                    backupId:
                      typeof idOrOptions === 'string' ? idOrOptions : '',
                  },
                }
              : { operation: name }
          const args = [
            '--storage',
            userProfile(options.profile),
            encodeStorageAction(action),
          ]
          let stdout: string
          try {
            stdout = (
              await promisify(execFile)(await runtimeExecutable(), args, {
                windowsHide: true,
                timeout: 45000,
                maxBuffer: 1048576,
                env: { SystemRoot: process.env.SystemRoot },
              })
            ).stdout
          } catch (error) {
            const failure = error as { stdout?: unknown }
            let code = 'STORAGE_FAILED'
            try {
              const value: unknown = JSON.parse(
                typeof failure.stdout === 'string' ? failure.stdout : ''
              )
              if (
                value &&
                typeof value === 'object' &&
                'error' in value &&
                value.error &&
                typeof value.error === 'object' &&
                'code' in value.error &&
                typeof value.error.code === 'string' &&
                /^[A-Z_]+$/.test(value.error.code)
              )
                code = value.error.code
            } catch {
              /* Native output is never used as an exception message. */
            }
            throw new Error(code)
          }
          const report = decodeStorageReport(JSON.parse(stdout))
          process.stdout.write(
            JSON.stringify({ formatVersion: 1, success: true, data: report }) +
              '\n'
          )
        }
      )
  }
}
