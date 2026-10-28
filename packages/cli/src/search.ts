import type { Command } from 'commander'

import { CommandSearchIndex } from '@flowtools/sdk/search'

import { builtInCLIManifests } from './builtin-manifests'
import { CLIInputError } from './schema'

export function addSearchCommand(program: Command) {
  program
    .command('search <query>')
    .description('Search built-in metadata as JSON without starting Runtime')
    .option('--mode <mode>', 'text or regex', 'text')
    .action((query: string, options: { mode: string }) => {
      if (options.mode !== 'text' && options.mode !== 'regex')
        throw new CLIInputError('SCHEMA_VALIDATION', 'Invalid search mode')
      const index = new CommandSearchIndex()
      for (const plugin of builtInCLIManifests)
        index.upsert({
          id: `${plugin.id}:run`,
          pluginId: plugin.id,
          title: plugin.name,
          description: 'description' in plugin ? plugin.description : undefined,
        })
      try {
        process.stdout.write(
          JSON.stringify({
            formatVersion: 1,
            success: true,
            data: index.search(query, { mode: options.mode }),
          }) + '\n'
        )
      } catch {
        throw new CLIInputError(
          'SCHEMA_VALIDATION',
          'Invalid or over-budget search'
        )
      }
    })
}
