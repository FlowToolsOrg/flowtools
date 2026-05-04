#!/usr/bin/env bun

/**
 * Flow Tool CLI — extensible plugin runner.
 *
 * Usage:
 *   flowtools list [--format json|text]
 *   flowtools info <plugin-id>
 *   flowtools run <plugin-id> [flags]
 *   flowtools run <plugin-id> --input '{"key":"value"}'
 *   flowtools run <plugin-id> --help
 */

import type { OutputFormat } from './types'
import type { ZodObject } from 'zod'

import { Command } from 'commander'

import { scanPlugins, loadPlugin } from './discovery'
import { runPluginAndPrint } from './runner'
import {
  buildInputFromOptions,
  parseJsonInput,
  introspectSchema,
  toKebab,
  generateMockFromSchema,
  buildFlagExample,
} from './schema'

const program = new Command()

program
  .name('flowtools')
  .description('Flow Tool — extensible plugin CLI')
  .version('0.1.0')

// ─── list ───────────────────────────────────────────────────────────

program
  .command('list')
  .alias('ls')
  .description('List all plugins')
  .option('-f, --format <format>', 'Output format: json or stdio', 'stdio')
  .option('-a, --all', 'Show all plugins (including non-CLI)', false)
  .action((opts: { format: string; all: boolean }) => {
    const plugins = scanPlugins()
    const filtered = opts.all ? plugins : plugins.filter(p => p.hasRun)

    if (opts.format === 'json') {
      return
    }

    if (filtered.length === 0) {
      return
    }

    for (const p of filtered) {
      const cli = p.hasRun ? '✓' : '✗'
      const schema = p.hasSchema ? ' [schema]' : ''
      const desc = p.description ? ` — ${p.description}` : ''
    }
  })

// ─── info ───────────────────────────────────────────────────────────

program
  .command('info [plugin-id]')
  .description('Show plugin details and input schema')
  .option('-f, --format <format>', 'Output format: json or stdio', 'stdio')
  .option('-a, --all', 'Show info for all plugins', false)
  .action(
    async (
      pluginId: string | undefined,
      opts: { format: string; all: boolean }
    ) => {
      const plugins = scanPlugins()

      if (opts.all) {
        for (const p of plugins) {
          await printPluginInfo(p.id, opts.format)
        }
        return
      }

      if (!pluginId) {
        process.exit(1)
      }

      await printPluginInfo(pluginId, opts.format)
    }
  )

// ─── run ────────────────────────────────────────────────────────────

program
  .command('run <plugin-id>')
  .description('Execute a plugin (use --help with plugin-id for options)')
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
      }

      if (!pluginInfo.hasRun) {
        process.exit(1)
      }

      const plugin = await loadPlugin(pluginId)
      if (!plugin) {
        process.exit(1)
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

        const schema = plugin.inputSchema as Parameters<
          typeof parseJsonInput
        >[1]
        if (schema?.safeParse) {
          const result = schema.safeParse(input)
          if (!result.success) {
            for (const issue of (result as any).error.issues) {
            }
            process.exit(1)
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

// ─── Plugin-specific help ───────────────────────────────────────────

async function printPluginInfo(
  pluginId: string,
  format: string
): Promise<void> {
  const plugins = scanPlugins()
  const pluginInfo = plugins.find(p => p.id === pluginId)

  if (!pluginInfo) {
    process.exit(1)
  }

  if (format === 'json') {
    const plugin = pluginInfo.hasSchema ? await loadPlugin(pluginId) : null
    const schema = plugin?.inputSchema
      ? introspectSchema(plugin.inputSchema as ZodObject<any>)
      : null
    const mock = schema
      ? await generateMockFromSchema(plugin!.inputSchema as ZodObject<any>)
      : null

    return
  }

  if (pluginInfo.description) {
  }

  if (pluginInfo.hasSchema) {
    try {
      const plugin = await loadPlugin(pluginId)
      if (plugin?.inputSchema) {
        const fields = introspectSchema(plugin.inputSchema as ZodObject<any>)

        for (const [key, def] of Object.entries(fields)) {
          const flag = `--${toKebab(key)}`
          const type = def.type === 'boolean' ? '' : ` <${def.type}>`
          const req = def.required ? ' [required]' : ''
          const defVal =
            def.default !== undefined
              ? ` [default: ${JSON.stringify(def.default)}]`
              : ''
          const desc = def.description ?? key

          if (def.enum) {
          }
        }

        const mock = await generateMockFromSchema(
          plugin.inputSchema as ZodObject<any>
        )
      }
    } catch (err) {}
  }
}

function printSchemaFlags(
  schema: Record<string, import('./schema').FieldMeta>
): void {
  for (const [key, def] of Object.entries(schema)) {
    const flag = `--${toKebab(key)}`
    const type = def.type === 'boolean' ? '' : ` <${def.type}>`
    const req = def.required ? ' [required]' : ''
    const defVal =
      def.default !== undefined
        ? ` [default: ${JSON.stringify(def.default)}]`
        : ''
    const desc = def.description ?? key

    if (def.enum) {
    }
  }
}

async function showPluginHelp(pluginId: string): Promise<void> {
  const plugins = scanPlugins()
  const pluginInfo = plugins.find(p => p.id === pluginId)

  if (!pluginInfo) {
    process.exit(1)
  }

  if (pluginInfo.description) {
  }

  if (pluginInfo.hasSchema) {
    try {
      const plugin = await loadPlugin(pluginId)
      if (plugin?.inputSchema) {
        const fields = introspectSchema(plugin.inputSchema as ZodObject<any>)

        printSchemaFlags(fields)

        const mock = await generateMockFromSchema(
          plugin.inputSchema as ZodObject<any>
        )
      }
    } catch (err) {}
  } else {
  }
}

/**
 * Commander's `allowUnknownOption` silently skips unknown options but does
 * NOT put them in `cmd.args`. We must read raw process.argv instead.
 *
 * Returns everything after `run <plugin-id>`.
 */
function getPluginArgs(): string[] {
  const argv = process.argv
  const runIdx = argv.findIndex(a => a === 'run' || a.endsWith('/run'))
  if (runIdx === -1) return []

  const rest = argv.slice(runIdx + 2)

  const KNOWN = new Set(['-f', '--format', '-i', '--input', '-t', '--timeout'])
  const filtered: string[] = []
  for (let i = 0; i < rest.length; i++) {
    if (KNOWN.has(rest[i])) {
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
function parseUnknownArgs(args: string[]): Record<string, string | string[]> {
  const result: Record<string, string | string[]> = {}

  for (let i = 0; i < args.length; i++) {
    const arg = args[i]
    if (!arg) continue

    if (arg.startsWith('--')) {
      const key = arg.slice(2)
      const next = args[i + 1]
      if (next && !next.startsWith('-')) {
        if (key in result) {
          const prev = result[key]
          result[key] = Array.isArray(prev) ? [...prev, next] : [prev, next]
        } else {
          result[key] = next
        }
        i++
      } else {
        result[key] = 'true'
      }
    } else if (arg.startsWith('-')) {
      const key = arg.slice(1)
      const next = args[i + 1]
      if (next && !next.startsWith('-')) {
        if (key in result) {
          const prev = result[key]
          result[key] = Array.isArray(prev) ? [...prev, next] : [prev, next]
        } else {
          result[key] = next
        }
        i++
      } else {
        result[key] = 'true'
      }
    }
  }

  return result
}

// ─── Entry point ────────────────────────────────────────────────────

async function main() {
  const rawArgs = process.argv.slice(2)
  const runIdx = rawArgs.indexOf('run')
  if (runIdx !== -1 && runIdx + 1 < rawArgs.length) {
    const pluginArg = rawArgs[runIdx + 1]
    if (pluginArg && !pluginArg.startsWith('-')) {
      const rest = rawArgs.slice(runIdx + 2)
      if (rest.includes('--help') || rest.includes('-h')) {
        await showPluginHelp(pluginArg)
        return
      }
    }
  }

  program.parse()
}

main().catch(err => {
  process.exit(1)
})
