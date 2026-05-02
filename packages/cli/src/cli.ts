#!/usr/bin/env bun

/**
 * Flow Tool CLI — extensible plugin runner.
 *
 * Usage:
 *   flow-tool list [--format json|text]
 *   flow-tool info <plugin-id>
 *   flow-tool run <plugin-id> [flags]
 *   flow-tool run <plugin-id> --input '{"key":"value"}'
 */

import { Command } from 'commander'

import { scanPlugins } from './discovery'
import { runPluginAndPrint } from './runner'
import { buildInputFromOptions, parseJsonInput } from './schema'

const program = new Command()

program
  .name('flow-tool')
  .description('Flow Tool — extensible plugin CLI')
  .version('0.1.0')

// ─── list ───────────────────────────────────────────────────────────

program
  .command('list')
  .description('List all CLI-available plugins')
  .option('-f, --format <format>', 'Output format (json|text)', 'text')
  .action((opts: { format: string }) => {
    const plugins = scanPlugins()
    const cliPlugins = plugins.filter(p => p.hasRun)

    if (opts.format === 'json') {
      console.log(JSON.stringify(cliPlugins, null, 2))
      return
    }

    if (cliPlugins.length === 0) {
      console.log('No CLI-compatible plugins found.')
      console.log('Plugins need a run() function to be CLI-available.')
      return
    }

    console.log(`Found ${cliPlugins.length} CLI-available plugin(s):\n`)
    for (const p of cliPlugins) {
      const desc = p.description ? ` — ${p.description}` : ''
      const schema = p.hasSchema ? ' [schema]' : ''
      console.log(`  ${p.id}  ${p.name} (${p.version})${schema}${desc}`)
    }
  })

// ─── info ───────────────────────────────────────────────────────────

program
  .command('info <plugin-id>')
  .description('Show plugin details and input schema')
  .option('-f, --format <format>', 'Output format (json|text)', 'text')
  .action(async (pluginId: string, opts: { format: string }) => {
    const plugins = scanPlugins()
    const pluginInfo = plugins.find(p => p.id === pluginId)

    if (!pluginInfo) {
      console.error(`Plugin not found: ${pluginId}`)
      process.exit(1)
      return
    }

    if (opts.format === 'json') {
      console.log(JSON.stringify(pluginInfo, null, 2))
      return
    }

    console.log(`Plugin: ${pluginInfo.name} (${pluginInfo.id})`)
    console.log(`Version: ${pluginInfo.version}`)
    console.log(`Type: ${pluginInfo.type}`)
    console.log(`CLI Available: ${pluginInfo.hasRun ? 'Yes' : 'No'}`)
    console.log(`Has Schema: ${pluginInfo.hasSchema ? 'Yes' : 'No'}`)
    if (pluginInfo.description) {
      console.log(`Description: ${pluginInfo.description}`)
    }

    if (pluginInfo.hasSchema) {
      console.log('\nUse --input <json> or schema flags (see plugin source)')
      console.log('\nAlternatively, pass raw JSON:')
      console.log('  --input \'{"key":"value"}\'')
    }
  })

// ─── run ────────────────────────────────────────────────────────────

program
  .command('run <plugin-id>')
  .description('Execute a plugin')
  .option('-f, --format <format>', 'Output format (json|text)', 'json')
  .option('-i, --input <json>', 'Raw JSON input (alternative to flags)')
  .option('-t, --timeout <ms>', 'Execution timeout in ms', '30000')
  .allowUnknownOption(true)
  .action(
    async (
      pluginId: string,
      opts: { format: string; input?: string; timeout?: string },
      cmd: Command
    ) => {
      const plugins = scanPlugins()
      const pluginInfo = plugins.find(p => p.id === pluginId)

      if (!pluginInfo) {
        console.error(`Plugin not found: ${pluginId}`)
        process.exit(1)
        return
      }

      if (!pluginInfo.hasRun) {
        console.error(
          `Plugin ${pluginId} has no run() function — not CLI-compatible`
        )
        process.exit(1)
        return
      }

      // Load the actual plugin to access its Zod schema
      const { loadPlugin } = await import('./discovery')
      const plugin = await loadPlugin(pluginId)
      if (!plugin) {
        process.exit(1)
        return
      }

      let input: Record<string, unknown> = {}

      if (opts.input) {
        input = parseJsonInput(
          opts.input,
          plugin.inputSchema as Parameters<typeof parseJsonInput>[1]
        )
      } else if (plugin.inputSchema) {
        const unknownOpts = parseUnknownArgs(cmd.args)
        input = buildInputFromOptions(
          unknownOpts,
          plugin.inputSchema as Parameters<typeof buildInputFromOptions>[1]
        )

        // Validate with Zod
        const schema = plugin.inputSchema as Parameters<
          typeof parseJsonInput
        >[1]
        if (schema?.safeParse) {
          const result = schema.safeParse(input)
          if (!result.success) {
            console.error('Input validation failed:')
            for (const issue of (result as any).error.issues) {
              console.error(`  ${issue.path.join('.')}: ${issue.message}`)
            }
            process.exit(1)
            return
          }
          input = result.data as Record<string, unknown>
        }
      }

      const exitCode = await runPluginAndPrint(
        pluginId,
        input,
        opts.format as 'json' | 'text'
      )
      process.exit(exitCode)
    }
  )

/**
 * Parse unknown args from commander into a key-value object.
 */
function parseUnknownArgs(args: string[]): Record<string, string> {
  const result: Record<string, string> = {}
  const pluginArgs = args.slice(1)

  for (let i = 0; i < pluginArgs.length; i++) {
    const arg = pluginArgs[i]
    if (!arg) continue

    if (arg.startsWith('--')) {
      const key = arg.slice(2)
      const next = pluginArgs[i + 1]
      if (next && !next.startsWith('--')) {
        result[key] = next
        i++
      } else {
        result[key] = 'true'
      }
    } else if (arg.startsWith('-')) {
      const key = arg.slice(1)
      const next = pluginArgs[i + 1]
      if (next && !next.startsWith('-')) {
        result[key] = next
        i++
      } else {
        result[key] = 'true'
      }
    }
  }

  return result
}

program.parse()
