import type { Call } from '@flowtools/runtime-client'

import { Command } from 'commander'

import { startManagement } from './management'
import { userProfile } from './native-runtime'

export function addFileCommands(program: Command) {
  const files = program
    .command('files')
    .description(
      'Preview, commit and undo an explicitly selected NTFS rename plan'
    )
  const invoke = async (call: Call, profile?: string) => {
    const host = await startManagement(userProfile(profile), false)
    try {
      const result = await host.client.call(call)
      if (!('data' in result)) throw new Error('INVALID_RESPONSE')
      process.stdout.write(
        JSON.stringify({ formatVersion: 1, success: true, data: result.data }) +
          '\n'
      )
    } finally {
      await host.close()
    }
  }
  files
    .command('list')
    .option('--profile <directory>')
    .action(async (o: { profile?: string }) =>
      invoke({ method: 'files.list' }, o.profile)
    )
  files
    .command('recovery')
    .option('--profile <directory>')
    .action(async (o: { profile?: string }) =>
      invoke({ method: 'files.recovery' }, o.profile)
    )
  files
    .command('discard-restored')
    .requiredOption('--challenge <sha256>')
    .option('--approve')
    .option('--profile <directory>')
    .action(
      async (o: { challenge: string; approve?: boolean; profile?: string }) => {
        if (!o.approve) throw new Error('APPROVAL_REQUIRED')
        if (!/^[a-f0-9]{64}$/.test(o.challenge))
          throw new Error('INPUT_INVALID')
        await invoke(
          {
            method: 'files.discard-restored',
            payload: { challenge: o.challenge },
          },
          o.profile
        )
      }
    )
  files
    .command('plan')
    .requiredOption('--directory <directory>')
    .requiredOption('--prefix <prefix>')
    .option('--request-key <uuid>')
    .option('--approve-read')
    .option('--profile <directory>')
    .action(
      async (o: {
        directory: string
        prefix: string
        requestKey?: string
        approveRead?: boolean
        profile?: string
      }) => {
        if (!o.approveRead) throw new Error('APPROVAL_REQUIRED')
        await invoke(
          {
            method: 'files.plan',
            payload: {
              requestKey: o.requestKey ?? crypto.randomUUID(),
              directory: o.directory,
              prefix: o.prefix,
            },
          },
          o.profile
        )
      }
    )
  for (const operation of ['preview', 'commit', 'undo', 'cancel'] as const) {
    files
      .command(`${operation} <id>`)
      .requiredOption('--revision <number>')
      .option('--digest <sha256>')
      .option('--approve')
      .option('--profile <directory>')
      .action(
        async (
          id: string,
          o: {
            revision: string
            digest?: string
            approve?: boolean
            profile?: string
          }
        ) => {
          if (!/^[1-9][0-9]{0,8}$/.test(o.revision))
            throw new Error('INPUT_INVALID')
          const key = { id, revision: Number(o.revision) }
          if (operation === 'commit' || operation === 'undo') {
            if (!o.approve) throw new Error('APPROVAL_REQUIRED')
            if (!o.digest || !/^[a-f0-9]{64}$/.test(o.digest))
              throw new Error('INPUT_INVALID')
            await invoke(
              {
                method: operation === 'commit' ? 'files.commit' : 'files.undo',
                payload: { ...key, digest: o.digest },
              },
              o.profile
            )
          } else
            await invoke(
              {
                method:
                  operation === 'preview' ? 'files.preview' : 'files.cancel',
                payload: key,
              },
              o.profile
            )
        }
      )
  }
}
