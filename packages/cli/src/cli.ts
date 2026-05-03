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
import { OutputFormat } from './types'

const program = new Command()

program
  .name('flow-tool')
  .description('Flow Tool — extensible plugin CLI')
  .version('0.1.0')

// ─── list ───────────────────────────────────────────────────────────

program
  .command('list')
  .description('List all CLI-available plugins')
  .option('-f, --format <format>', 'Output format: json or stdio', 'stdio')
  .action((opts: { format: string }) => {
    const plugins = scanPlugins()
    const cliPlugins = plugins.filter(p => p.hasRun)

    if (opts.format === 'json') {
      return
    }

    if (cliPlugins.length === 0) {
      return
    }

    for (const p of cliPlugins) {
      const desc = p.description ? ` — ${p.description}` : ''
      const schema = p.hasSchema ? ' [schema]' : ''
    }
  })

// ─── info ───────────────────────────────────────────────────────────

program
  .command('info <plugin-id>')
  .description('Show plugin details and input schema')
  .option('-f, --format <format>', 'Output format: json or stdio', 'stdio')
  .action(async (pluginId: string, opts: { format: string }) => {
    const plugins = scanPlugins()
    const pluginInfo = plugins.find(p => p.id === pluginId)

    if (!pluginInfo) {
      process.exit(1)
      return
    }

    if (opts.format === 'json') {
      return
    }

    if (pluginInfo.description) {
    }

    if (pluginInfo.hasSchema) {
    }
  })

// ─── run ────────────────────────────────────────────────────────────

program
  .command('run <plugin-id>')
  .description('Execute a plugin')
  .option('-f, --format <format>', 'Output format: json or stdio')
  .option('-i, --input <json>', 'Raw JSON input (default output: json)')
  .option('-t, --timeout <ms>', 'Execution timeout in ms', '30000')
  .allowUnknownOption(true)
  .allowExcessArguments(true)
  .action(
    async (
      pluginId: string,
      opts: { format?: string; input?: string; timeout: string }
    ) => {
      const plugins = scanPlugins()
      const pluginInfo = plugins.find(p => p.id === pluginId)

      if (!pluginInfo) {
        process.exit(1)
        return
      }

      if (!pluginInfo.hasRun) {
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
        const pluginArgs = getPluginArgs()
        const unknownOpts = parseUnknownArgs(pluginArgs)
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
        (opts.format ?? (opts.input ? 'json' : 'stdio')) as OutputFormat
      )
      process.exit(exitCode)
    }
  )

/**
 * Commander's `allowUnknownOption` silently skips unknown options but does
 * NOT put them in `cmd.args`. We must read raw process.argv instead.
 *
 * Returns everything after `run <plugin-id>`.
 */
function getPluginArgs(): string[] {
  const argv = process.argv
  // Find the 'run' subcommand position
  const runIdx = argv.findIndex(a => a === 'run' || a.endsWith('/run'))
  if (runIdx === -1) return []

  // Skip 'run' and <plugin-id>, collect the rest
  const rest = argv.slice(runIdx + 2)

  // Strip Commander's known options so they don't leak into plugin flags
  const KNOWN = new Set(['-f', '--format', '-i', '--input', '-t', '--timeout'])
  const filtered: string[] = []
  for (let i = 0; i < rest.length; i++) {
    if (KNOWN.has(rest[i])) {
      // skip the option and its value (next arg)
      i++
      continue
    }
    filtered.push(rest[i])
  }
  return filtered
}

/**
 * Parse CLI args into a key-value object.
 * Handles: --key value, --flag, -k value, -f
 */
function parseUnknownArgs(args: string[]): Record<string, string> {
  const result: Record<string, string> = {}

  for (let i = 0; i < args.length; i++) {
    const arg = args[i]
    if (!arg) continue

    if (arg.startsWith('--')) {
      const key = arg.slice(2)
      const next = args[i + 1]
      if (next && !next.startsWith('-')) {
        result[key] = next
        i++
      } else {
        result[key] = 'true'
      }
    } else if (arg.startsWith('-')) {
      const key = arg.slice(1)
      const next = args[i + 1]
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
