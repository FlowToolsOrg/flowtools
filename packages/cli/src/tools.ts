import { Command, InvalidArgumentError } from 'commander'

import { connectHost } from './host'
import { userProfile } from './native-runtime'

export function addToolCommands(program: Command) {
  program
    .command('tools')
    .description('Read managed tool storage observations')
    .command('list')
    .description(
      'Read one bounded inventory page from an already running Runtime'
    )
    .option('--profile <directory>')
    .option(
      '--offset <number>',
      'Page offset (multiple of eight)',
      value => {
        if (
          !/^(0|[1-9][0-9]*)$/.test(value) ||
          Number(value) > 64 ||
          Number(value) % 8
        )
          throw new InvalidArgumentError('INPUT_INVALID')
        return Number(value)
      },
      0
    )
    .option('--snapshot <digest>', 'Snapshot from the first page')
    .action(
      async (options: {
        profile?: string
        offset: number
        snapshot?: string
      }) => {
        if (
          (options.offset && !options.snapshot) ||
          (options.snapshot && !/^[a-f0-9]{64}$/.test(options.snapshot))
        )
          throw new InvalidArgumentError('INPUT_INVALID')
        const client = await connectHost(userProfile(options.profile), true)
        try {
          const page = await client.toolInventory({
            offset: options.offset,
            snapshot: options.snapshot ?? null,
          })
          process.stdout.write(
            JSON.stringify({ formatVersion: 1, success: true, data: page }) +
              '\n'
          )
        } finally {
          client.close()
        }
      }
    )
}
