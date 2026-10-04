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

import { createExecutionFailure } from '@flowtools/sdk/execution'
import { Command } from 'commander'

import { scanPlugins, loadPlugin } from './discovery'
import { printExecutionResult, runPluginAndPrint } from './runner'
import {
  buildInputFromOptions,
  parseJsonInput,
  introspectSchema,
  toKebab,
  generateMockFromSchema,
  buildFlagExample,
  CLIInputError,
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
  .option('-f, --format <format>', 'Output format: json or text', 'text')
  .option('-a, --all', 'Show all plugins (including non-CLI)', false)
  .action((opts: { format: string; all: boolean }) => {
    const plugins = scanPlugins()
    const filtered = opts.all ? plugins : plugins.filter(p => p.hasRun)
    const format = normalizeFormat(opts.format)

    if (format === 'json') {
      writeJson(filtered)
      return
    }

    if (filtered.length === 0) {
      process.stdout.write('No plugins found\n')
      return
    }

    for (const p of filtered) {
      const cli = p.hasRun ? '✓' : '✗'
      const schema = p.hasSchema ? ' [schema]' : ''
      const desc = p.description ? ` — ${p.description}` : ''
      process.stdout.write(`${cli} ${p.id} [${p.maturity}]${schema}${desc}\n`)
    }
  })

// ─── info ───────────────────────────────────────────────────────────

program
  .command('info [plugin-id]')
  .description('Show plugin details and input schema')
  .option('-f, --format <format>', 'Output format: json or text', 'text')
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
        process.stderr.write(
          'Missing plugin id. Use --all to show all plugins.\n'
        )
        process.exit(1)
      }

      await printPluginInfo(pluginId, opts.format)
    }
  )

// ─── run ────────────────────────────────────────────────────────────

program
  .command('run <plugin-id>')
  .description('Execute a plugin (use --help with plugin-id for options)')
  .option('-f, --format <format>', 'Output format: json or text')
  .option('-i, --input <json>', 'Raw JSON input (default output: json)')
  .option('-t, --timeout <ms>', 'Execution timeout in ms', '30000')
  .allowUnknownOption(true)
  .allowExcessArguments(true)
  .action(
    async (
      pluginId: string,
      opts: { format?: string; input?: string; timeout: string }
    ) => {
      const format = normalizeFormat(
        opts.format ?? (opts.input !== undefined ? 'json' : 'text')
      )
      const startedAt = Date.now()
      let version: string | null = null
      let input: Record<string, unknown> = {}
      let exitCode: number
      try {
        // Parse before discovery; JSON errors must not execute or load code.
        if (opts.input !== undefined) input = parseJsonInput(opts.input)
        const pluginInfo = scanPlugins().find(p => p.id === pluginId)
        if (!pluginInfo) {
          exitCode = printExecutionResult(
            createExecutionFailure(
              pluginId,
              null,
              input,
              {
                code: 'PLUGIN_NOT_FOUND',
                message: `Plugin not found: ${pluginId}`,
              },
              startedAt
            ),
            format
          )
        } else {
          version = pluginInfo.version
          const plugin = await loadPlugin(pluginId)
          if (!plugin) {
            exitCode = printExecutionResult(
              createExecutionFailure(
                pluginId,
                version,
                input,
                {
                  code: 'LOAD_FAILED',
                  message: `Failed to load plugin: ${pluginId}`,
                },
                startedAt
              ),
              format
            )
          } else {
            if (opts.input === undefined && plugin.inputSchema) {
              input = buildInputFromOptions(
                parseUnknownArgs(getPluginArgs()),
                plugin.inputSchema
              )
            }
            exitCode = await runPluginAndPrint(pluginId, input, format, {
              timeout: Number(opts.timeout),
            })
          }
        }
      } catch (error) {
        const invalid = error instanceof CLIInputError
        exitCode = printExecutionResult(
          createExecutionFailure(
            pluginId,
            version,
            opts.input ?? input,
            {
              code: invalid ? 'INPUT_INVALID' : 'LOAD_FAILED',
              // Parser diagnostics can contain raw input; never echo them.
              message: invalid
                ? 'Input must be valid JSON matching the plugin schema'
                : 'Failed to prepare plugin execution',
            },
            startedAt
          ),
          format
        )
      }
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
    process.stderr.write(`Plugin not found: ${pluginId}\n`)
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

    writeJson({ ...pluginInfo, schema, example: schema ? mock : null })
    return
  }

  process.stdout.write(`${pluginInfo.id}\n`)
  process.stdout.write(`Name: ${pluginInfo.name}\n`)
  process.stdout.write(`Version: ${pluginInfo.version}\n`)
  process.stdout.write(`Maturity: ${pluginInfo.maturity}\n`)
  process.stdout.write(`Type: ${pluginInfo.type}\n`)
  process.stdout.write(`CLI: ${pluginInfo.hasRun ? 'yes' : 'no'}\n`)

  if (pluginInfo.description) {
    process.stdout.write(`Description: ${pluginInfo.description}\n`)
  }

  if (pluginInfo.hasSchema) {
    try {
      const plugin = await loadPlugin(pluginId)
      if (plugin?.inputSchema) {
        const fields = introspectSchema(plugin.inputSchema as ZodObject<any>)

        process.stdout.write('\nOptions:\n')
        printSchemaFlags(fields)

        const mock = await generateMockFromSchema(
          plugin.inputSchema as ZodObject<any>
        )
        const example = buildFlagExample(fields, mock)
        if (example) {
          process.stdout.write(
            `\nExample:\n  flowtools run ${pluginId} ${example}\n`
          )
        }
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      process.stderr.write(`Failed to load schema: ${message}\n`)
    }
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

    const choices = def.enum ? ` [choices: ${def.enum.join(', ')}]` : ''
    process.stdout.write(`  ${flag}${type}${req}${defVal}${choices}\n`)
    process.stdout.write(`    ${desc}\n`)
  }
}

async function showPluginHelp(pluginId: string): Promise<void> {
  const plugins = scanPlugins()
  const pluginInfo = plugins.find(p => p.id === pluginId)

  if (!pluginInfo) {
    process.stderr.write(`Plugin not found: ${pluginId}\n`)
    process.exit(1)
  }

  process.stdout.write(`Usage: flowtools run ${pluginId} [options]\n`)
  process.stdout.write(`\n${pluginInfo.name}\n`)
  if (pluginInfo.description) {
    process.stdout.write(`${pluginInfo.description}\n`)
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
        const example = buildFlagExample(fields, mock)
        if (example) {
          process.stdout.write(
            `\nExample:\n  flowtools run ${pluginId} ${example}\n`
          )
        }
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      process.stderr.write(`Failed to load schema: ${message}\n`)
    }
  } else {
    process.stdout.write('\nThis plugin does not declare input options.\n')
  }
}

function normalizeFormat(format: string): OutputFormat {
  if (format === 'json') return 'json'
  if (format === 'stdio' || format === 'text') return 'text'

  process.stderr.write(`Unsupported format: ${format}\n`)
  process.exit(1)
}

function writeJson(value: unknown): void {
  process.stdout.write(`${JSON.stringify(value, null, 2)}\n`)
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

  await program.parseAsync()
}

main().catch(error => {
  const message = error instanceof Error ? error.message : String(error)
  process.stderr.write(`${message}\n`)
  process.exitCode = 1
})
