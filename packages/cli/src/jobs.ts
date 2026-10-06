import { withSubmittedRuntimeJob } from '@flowtools/runtime-client'
import { Command } from 'commander'

import { validateCommandInput } from './command-schema'
import { getBuiltinCommandManifest } from './discovery'
import { connectHost } from './host'
import { userProfile } from './management'

type Options = { profile?: string; format: string }
function output(value: unknown, options: Options) {
  if (!['json', 'text'].includes(options.format))
    throw new Error('INPUT_INVALID')
  if (options.format === 'json')
    process.stdout.write(
      JSON.stringify({
        formatVersion: 1,
        success: true,
        data: value,
      }) + '\n'
    )
  else process.stdout.write(JSON.stringify(value, null, 2) + '\n')
}
export function addJobCommands(program: Command) {
  const jobs = program
    .command('jobs')
    .description('Inspect and control shared Runtime jobs')
  jobs
    .command('submit <plugin-id>')
    .option('--command <id>', 'Command identity', 'run')
    .option('--input <json>', 'Declared JSON input', '{}')
    .requiredOption(
      '--idempotency-key <key>',
      'Stable key for this exact operation'
    )
    .option('--background', 'Explicitly granted background execution')
    .option('--deadline <timestamp>', 'Absolute epoch milliseconds')
    .option('--profile <directory>')
    .option('--format <format>', 'json or text', 'json')
    .action(
      async (
        pluginId: string,
        options: Options & {
          command: string
          input: string
          idempotencyKey: string
          background?: boolean
          deadline?: string
        }
      ) => {
        const manifest = getBuiltinCommandManifest(pluginId)
        if (!manifest) throw new Error('PLUGIN_NOT_FOUND')
        const command = manifest.commands.find(
          item => item.id === options.command
        )
        if (!command) throw new Error('INPUT_INVALID')
        let input: unknown
        try {
          input = validateCommandInput(command, JSON.parse(options.input))
        } catch {
          throw new Error('INPUT_INVALID')
        }
        await withSubmittedRuntimeJob(
          () => connectHost(userProfile(options.profile), false, true),
          {
            pluginId,
            commandId: options.command,
            input,
            idempotencyKey: options.idempotencyKey,
            background: Boolean(options.background),
            deadline: options.deadline
              ? Number(options.deadline)
              : Date.now() + 30000,
          },
          async (client, receipt) => {
            output(receipt, options)
            // Foreground submit remains attached until terminal; background detaches at receipt.
            if (!options.background) {
              const terminal = await client.waitForResult(receipt.runId)
              output(terminal, options)
              if (!terminal.result?.success) process.exitCode = 1
            }
          }
        )
      }
    )
  for (const name of ['list', 'status', 'watch', 'cancel', 'lookup'] as const) {
    jobs
      .command(name === 'list' ? name : name + ' <id>')
      .option('--profile <directory>')
      .option('--format <format>', 'json or text', 'json')
      .action(async (idOrOptions: string | Options, supplied?: Options) => {
        const options =
          typeof idOrOptions === 'object' ? idOrOptions : supplied!
        const id = typeof idOrOptions === 'string' ? idOrOptions : ''
        const client = await connectHost(userProfile(options.profile))
        try {
          if (name === 'list') output(await client.jobs(), options)
          else if (name === 'status') output(await client.job(id), options)
          else if (name === 'cancel') output(await client.cancel(id), options)
          else if (name === 'lookup') {
            const receipt = await client.call({
              method: 'jobs.lookup',
              payload: { idempotencyKey: id },
            })
            if (receipt.type !== 'receipt') throw new Error('INVALID_RESPONSE')
            output(receipt.data, options)
          } else {
            for await (const event of client.watch(id)) output(event, options)
            const terminal = await client.job(id)
            output(terminal, options)
            if (!terminal.result?.success) process.exitCode = 1
          }
        } finally {
          client.close()
        }
      })
  }
}
