import type {
  Call,
  JobSnapshot,
  UserMediaAction,
} from '@flowtools/runtime-client'

import { Command } from 'commander'

import { connectHost, startUserRuntime } from './host'
import { startManagement } from './management'
import { userProfile } from './native-runtime'

const print = (data: unknown) =>
  process.stdout.write(
    JSON.stringify({ formatVersion: 1, success: true, data }) + '\n'
  )
export function addMediaCommands(program: Command) {
  const media = program
    .command('media')
    .description(
      'Inspect / extract WAV / transcode fixed WebM through one approved media plan (max 1 MiB)'
    )
  media
    .command('plan')
    .requiredOption('--input <file>')
    .option('--output <new-file>')
    .option('--action <inspect|audio|webm>', 'Fixed media action', 'inspect')
    .option('--request-key <uuid>')
    .option('--approve-read')
    .option('--profile <directory>')
    .action(
      async (o: {
        input: string
        output?: string
        action: string
        requestKey?: string
        approveRead?: boolean
        profile?: string
      }) => {
        if (!o.approveRead) throw new Error('APPROVAL_REQUIRED')
        if (!['inspect', 'audio', 'webm'].includes(o.action))
          throw new Error('INPUT_INVALID')
        const host = await startManagement(userProfile(o.profile), false)
        try {
          const result = await host.client.call({
            method: 'media.plan',
            payload: {
              requestKey: o.requestKey ?? crypto.randomUUID(),
              inputPath: o.input,
              outputPath: o.output ?? null,
              action: o.action as UserMediaAction,
            },
          })
          if (result.type !== 'media-plan') throw new Error('INVALID_RESPONSE')
          print(result.data)
        } finally {
          await host.close()
        }
      }
    )
  media
    .command('preview <id>')
    .option('--profile <directory>')
    .action(async (id: string, o: { profile?: string }) => {
      const host = await startManagement(userProfile(o.profile), false)
      try {
        const result = await host.client.call({
          method: 'media.preview',
          payload: { id, revision: 1 },
        })
        if (result.type !== 'media-plan') throw new Error('INVALID_RESPONSE')
        print(result.data)
      } finally {
        await host.close()
      }
    })
  media
    .command('start <id>')
    .requiredOption('--digest <sha256>')
    .option('--approve')
    .option('--wait')
    .option('--profile <directory>')
    .action(
      async (
        id: string,
        o: {
          digest: string
          approve?: boolean
          wait?: boolean
          profile?: string
        }
      ) => {
        if (!o.approve) throw new Error('APPROVAL_REQUIRED')
        if (!/^[a-f0-9]{64}$/.test(o.digest)) throw new Error('INPUT_INVALID')
        const profile = userProfile(o.profile)
        await startUserRuntime(profile)
        const host = await startManagement(profile, false)
        let run: string
        try {
          const result = await host.client.call({
            method: 'media.start',
            payload: { id, digest: o.digest, target: 'cli' },
          })
          if (result.type !== 'receipt') throw new Error('INVALID_RESPONSE')
          run = result.data.runId
          if (!o.wait) print(result.data)
        } finally {
          await host.close()
        }
        if (o.wait) await wait(profile, run!)
      }
    )
  for (const mode of ['status', 'cancel', 'wait'] as const)
    media
      .command(`${mode} <run>`)
      .option('--profile <directory>')
      .action(async (run: string, o: { profile?: string }) => {
        const profile = userProfile(o.profile)
        if (mode === 'wait') {
          await wait(profile, run)
          return
        }
        const client = await connectHost(profile, false, false)
        try {
          const call: Call =
            mode === 'cancel'
              ? { method: 'jobs.cancel', payload: { runId: run } }
              : { method: 'jobs.status', payload: { runId: run } }
          const result = await client.call(call)
          if (result.type !== 'job') throw new Error('INVALID_RESPONSE')
          print(result.data)
        } finally {
          client.close()
        }
      })
}
async function wait(profile: string, run: string) {
  const client = await connectHost(profile, false, false)
  try {
    const deadline = Date.now() + 40000
    let lastStage = ''
    while (Date.now() < deadline) {
      const result = await client.call({
        method: 'jobs.status',
        payload: { runId: run },
      })
      if (result.type !== 'job') throw new Error('INVALID_RESPONSE')
      const job: JobSnapshot = result.data
      if (
        ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(job.state)
      ) {
        process.stdout.write(
          JSON.stringify({
            formatVersion: 1,
            success: job.state === 'succeeded',
            data: job,
          }) + '\n'
        )
        if (job.state !== 'succeeded') process.exitCode = 1
        return
      }
      const progress = await client.call({
        method: 'media.progress',
        payload: { runId: run },
      })
      if (progress.type !== 'media-progress')
        throw new Error('INVALID_RESPONSE')
      if (progress.data.stage !== lastStage) {
        lastStage = progress.data.stage
        process.stderr.write(lastStage + '\n')
      }
      await new Promise(resolve => setTimeout(resolve, 100))
    }
    throw new Error('TIMEOUT')
  } finally {
    client.close()
  }
}
