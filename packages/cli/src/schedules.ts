import type { CreateSchedule } from '@flowtools/runtime-client'

import { readFile } from 'node:fs/promises'

import {
  encodeRequest,
  validScheduleDefinition,
} from '@flowtools/runtime-client'
import { Command } from 'commander'

import { startManagement } from './management'
import { userProfile } from './native-runtime'
import { regularFile } from './regular-file'

export function addScheduleCommands(program: Command) {
  const schedules = program
    .command('schedules')
    .description('Manage explicitly authorized finite background schedules')
  schedules
    .command('recovery')
    .option('--profile <directory>')
    .action(async (options: { profile?: string }) => {
      const host = await startManagement(userProfile(options.profile), false)
      try {
        const result = await host.client.call({ method: 'schedules.recovery' })
        if (result.type !== 'schedule-recovery')
          throw new Error('INVALID_RESPONSE')
        process.stdout.write(
          JSON.stringify({
            formatVersion: 1,
            success: true,
            data: result.data,
          }) + '\n'
        )
      } finally {
        await host.close()
      }
    })
  schedules
    .command('discard-restored')
    .requiredOption('--challenge <digest>')
    .option('--approve-discard')
    .option('--profile <directory>')
    .action(
      async (options: {
        challenge: string
        approveDiscard?: boolean
        profile?: string
      }) => {
        if (!options.approveDiscard) throw new Error('APPROVAL_REQUIRED')
        if (!/^[a-f0-9]{64}$/.test(options.challenge))
          throw new Error('INVALID_REQUEST')
        const host = await startManagement(userProfile(options.profile), false)
        try {
          const result = await host.client.call({
            method: 'schedules.discard-restored',
            payload: { challenge: options.challenge },
          })
          if (result.type !== 'schedules') throw new Error('INVALID_RESPONSE')
          process.stdout.write(
            JSON.stringify({
              formatVersion: 1,
              success: true,
              data: result.data,
            }) + '\n'
          )
        } finally {
          await host.close()
        }
      }
    )
  schedules
    .command('replace <id>')
    .requiredOption('--revision <number>')
    .requiredOption('--definition <file>')
    .option('--approve-schedule')
    .option('--profile <directory>')
    .action(
      async (
        id: string,
        options: {
          revision: string
          definition: string
          approveSchedule?: boolean
          profile?: string
        }
      ) => {
        if (!options.approveSchedule) throw new Error('APPROVAL_REQUIRED')
        const input: unknown = JSON.parse(
          await readFile(await regularFile(options.definition, 8192), 'utf8')
        )
        const payload = {
          previous: { id, revision: Number(options.revision) },
          create: input,
        }
        encodeRequest({
          version: 1,
          requestId: crypto.randomUUID(),
          session: null,
          call: { method: 'schedules.replace', payload },
        })
        const host = await startManagement(userProfile(options.profile), false)
        try {
          process.stdout.write(
            JSON.stringify({
              formatVersion: 1,
              success: true,
              data: await host.client.replaceSchedule(
                payload as import('@flowtools/runtime-client').ReplaceSchedule
              ),
            }) + '\n'
          )
        } finally {
          await host.close()
        }
      }
    )
  schedules
    .command('resolve-time')
    .requiredOption('--local <datetime>')
    .requiredOption('--zone <iana-name>')
    .option('--choice <policy>', 'reject, earlier or later', 'reject')
    .option('--profile <directory>')
    .action(
      async (options: {
        local: string
        zone: string
        choice: string
        profile?: string
      }) => {
        if (!['reject', 'earlier', 'later'].includes(options.choice))
          throw new Error('INPUT_INVALID')
        const host = await startManagement(userProfile(options.profile), false)
        try {
          const result = await host.client.call({
            method: 'schedules.resolve-time',
            payload: {
              local: options.local,
              timeZone: options.zone,
              choice: options.choice as 'reject' | 'earlier' | 'later',
            },
          })
          if (result.type !== 'schedule-time')
            throw new Error('INVALID_RESPONSE')
          process.stdout.write(
            JSON.stringify({
              formatVersion: 1,
              success: true,
              data: result.data,
            }) + '\n'
          )
        } finally {
          await host.close()
        }
      }
    )
  schedules
    .command('list')
    .option('--profile <directory>')
    .action(async (options: { profile?: string }) => {
      const host = await startManagement(userProfile(options.profile), false)
      try {
        process.stdout.write(
          JSON.stringify({
            formatVersion: 1,
            success: true,
            data: await host.client.schedules(),
          }) + '\n'
        )
      } finally {
        await host.close()
      }
    })
  schedules
    .command('create')
    .requiredOption('--definition <file>')
    .option('--approve-schedule')
    .option('--profile <directory>')
    .action(
      async (options: {
        definition: string
        approveSchedule?: boolean
        profile?: string
      }) => {
        if (!options.approveSchedule) throw new Error('APPROVAL_REQUIRED')
        const path = await regularFile(options.definition, 8192)
        const input: unknown = JSON.parse(await readFile(path, 'utf8'))
        encodeRequest({
          version: 1,
          requestId: crypto.randomUUID(),
          session: null,
          call: { method: 'schedules.create', payload: input },
        })
        const create = input as CreateSchedule
        if (!validScheduleDefinition(create.definition))
          throw new Error('INPUT_INVALID')
        const host = await startManagement(userProfile(options.profile), false)
        try {
          process.stdout.write(
            JSON.stringify({
              formatVersion: 1,
              success: true,
              data: await host.client.createSchedule(create),
            }) + '\n'
          )
        } finally {
          await host.close()
        }
      }
    )
  schedules
    .command('cancel <id>')
    .requiredOption('--revision <number>')
    .option('--profile <directory>')
    .action(
      async (id: string, options: { revision: string; profile?: string }) => {
        const revision = Number(options.revision)
        if (
          !Number.isInteger(revision) ||
          revision < 1 ||
          revision > 4294967295 ||
          !/^[a-f0-9-]{36}$/.test(id)
        )
          throw new Error('INVALID_REQUEST')
        const host = await startManagement(userProfile(options.profile), false)
        try {
          process.stdout.write(
            JSON.stringify({
              formatVersion: 1,
              success: true,
              data: await host.client.cancelSchedule(id, revision),
            }) + '\n'
          )
        } finally {
          await host.close()
        }
      }
    )
}
