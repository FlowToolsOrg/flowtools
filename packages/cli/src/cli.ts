#!/usr/bin/env bun

import type { OutputFormat } from './types'
import type { ExecutionErrorCode } from '@flowtools/sdk/execution'
import type {
  PluginManifestV1,
  CommandManifestV1,
} from '@flowtools/sdk/manifest'

import { createExecutionFailure } from '@flowtools/sdk/execution'
import { commandIdentity } from '@flowtools/sdk/manifest'
import { manifestPackageDigest } from '@flowtools/sdk/manifest/package'
import { Command } from 'commander'

import {
  commandExample,
  commandFlags,
  commandFlagHelp,
  parseCommandFlags,
  validateCommandInput,
} from './command-schema'
import {
  getBuiltinPluginInfo,
  getBuiltinCommandManifest,
  scanPlugins,
} from './discovery'
import { addRuntimeCommands } from './host'
import { addJobCommands } from './jobs'
import { addManagementCommands, managementFailure } from './management'
import { parseRunArguments, requestedRunFormat } from './run-arguments'
import { printExecutionResult, runPlugin } from './runner'
import { CLIInputError, isCLIInputError, parseJsonInput } from './schema'

const program = new Command()
  .name('flowtools')
  .description('FlowTools built-in command CLI')
  .version('0.1.0')
  .configureOutput({ outputError: () => {} })
  .exitOverride()

addManagementCommands(program)
addRuntimeCommands(program)
addJobCommands(program)

function writeJson(value: unknown) {
  process.stdout.write(JSON.stringify(value, null, 2) + '\n')
}

function fail(
  pluginId: string,
  code: ExecutionErrorCode,
  format: OutputFormat
): never {
  const messages: Partial<Record<ExecutionErrorCode, string>> = {
    PLUGIN_NOT_FOUND: 'Plugin is not in the host built-in inventory',
    LOAD_FAILED:
      'Built-in compiled contract unavailable; run bun run build:packages',
    INPUT_INVALID:
      'Input must match the declared command options and JSON schema',
    NOT_RUNNABLE: 'Command is unavailable in this built-in adapter',
    TIMEOUT_INVALID: 'Timeout must be an integer between 1 and 2147483647 ms',
  }
  process.exit(
    printExecutionResult(
      createExecutionFailure(
        pluginId,
        getBuiltinPluginInfo(pluginId)?.version ?? null,
        {},
        { code, message: messages[code] ?? 'Failed to prepare command' }
      ),
      format
    )
  )
}

function formatOption(format: string): OutputFormat {
  if (['json', 'text', 'stdio'].includes(format)) return format as OutputFormat
  throw new CLIInputError('INVALID_INPUT_SHAPE', 'Unsupported format')
}

function manifestFor(pluginId: string, format: OutputFormat): PluginManifestV1 {
  if (!getBuiltinPluginInfo(pluginId))
    fail(pluginId, 'PLUGIN_NOT_FOUND', format)
  try {
    const manifest = getBuiltinCommandManifest(pluginId)
    if (manifest) return manifest
  } catch {
    /* Bounded catalog/file diagnostics are not user input. */
  }
  return fail(pluginId, 'LOAD_FAILED', format)
}

function operationFor(
  manifest: PluginManifestV1,
  commandId: string,
  format: OutputFormat
): CommandManifestV1 {
  const command = manifest.commands.find(item => item.id === commandId)
  if (!command) fail(manifest.id, 'NOT_RUNNABLE', format)
  return command
}

function describeCommand(
  manifest: PluginManifestV1,
  command: CommandManifestV1
) {
  return {
    formatVersion: 1,
    identity: commandIdentity(manifest, command),
    publisher: manifest.publisher,
    packageDigest: manifestPackageDigest(manifest),
    pluginId: manifest.id,
    pluginVersion: manifest.version,
    maturity: manifest.maturity,
    engines: manifest.engines,
    targets: manifest.targets,
    command,
    flags: commandFlags(command),
    example: commandExample(command),
    authorization: 'declarations-only',
  }
}

program
  .command('list')
  .alias('ls')
  .description('List built-in plugins')
  .option('-f, --format <format>', 'json or text', 'text')
  .option('-a, --all', 'Include view-only entries', false)
  .action((opts: { format: string; all: boolean }) => {
    const format = formatOption(opts.format)
    let plugins
    try {
      plugins = scanPlugins()
    } catch {
      return fail('builtin-inventory', 'LOAD_FAILED', format)
    }
    const filtered = opts.all ? plugins : plugins.filter(item => item.hasRun)
    if (format === 'json') return writeJson(filtered)
    for (const plugin of filtered)
      process.stdout.write(
        `${plugin.hasRun ? '✓' : '✗'} ${plugin.id} [${plugin.maturity}]${plugin.hasSchema ? ' [schema]' : ''}${plugin.description ? ` — ${plugin.description}` : ''}\n`
      )
  })

program
  .command('commands [plugin-id]')
  .description('Discover commands without importing executors')
  .option('-f, --format <format>', 'json or text', 'text')
  .action((pluginId: string | undefined, opts: { format: string }) => {
    const format = formatOption(opts.format)
    let ids: string[]
    if (pluginId) ids = [pluginId]
    else {
      try {
        ids = scanPlugins().map(item => item.id)
      } catch {
        return fail('builtin-inventory', 'LOAD_FAILED', format)
      }
    }
    const commands = ids.flatMap(id => {
      const manifest = manifestFor(id, format)
      return manifest.commands.map(command =>
        describeCommand(manifest, command)
      )
    })
    if (format === 'json') return writeJson({ formatVersion: 1, commands })
    for (const item of commands)
      process.stdout.write(
        `${item.identity} [${item.maturity}] ${item.command.name} — ${item.command.description}\n`
      )
  })

program
  .command('describe <plugin-id> <command-id>')
  .description('Show the versioned operation contract')
  .option('-f, --format <format>', 'json or text', 'text')
  .action((pluginId: string, commandId: string, opts: { format: string }) => {
    const format = formatOption(opts.format)
    const manifest = manifestFor(pluginId, format)
    const command = operationFor(manifest, commandId, format)
    const description = describeCommand(manifest, command)
    if (format === 'json') writeJson(description)
    else {
      process.stdout.write(
        `${description.identity}\n${command.description}\n${commandFlagHelp(command)}\n`
      )
      process.stdout.write(
        `Effects: ${command.effects.join(', ') || 'none'}\nPermission requests (not grants): ${JSON.stringify(command.permissions)}\n`
      )
    }
  })

function printPluginInfo(pluginId: string, format: OutputFormat) {
  const manifest = manifestFor(pluginId, format)
  const command = operationFor(manifest, 'run', format)
  const info = getBuiltinPluginInfo(pluginId)!
  const schema = Object.fromEntries(
    commandFlags(command).map(field => [
      field.property,
      {
        type: field.type,
        required: field.required,
        ...(field.description !== undefined
          ? { description: field.description }
          : {}),
        ...(field.default !== undefined ? { default: field.default } : {}),
        ...(field.enum ? { enum: field.enum } : {}),
        ...(field.schema.type === 'array'
          ? {
              itemType:
                field.schema.items?.type === 'number' ||
                field.schema.items?.type === 'integer'
                  ? 'number'
                  : 'string',
            }
          : {}),
      },
    ])
  )
  if (format === 'json')
    return writeJson({ ...info, schema, example: commandExample(command) })
  process.stdout.write(
    `${info.id}\nName: ${info.name}\nVersion: ${info.version}\nMaturity: ${info.maturity}\nType: ${info.type}\nCLI: ${info.hasRun ? 'yes' : 'no'}\nDescription: ${info.description ?? ''}\n\nOptions:\n${commandFlagHelp(command)}\n`
  )
}

program
  .command('info [plugin-id]')
  .description('Show plugin details and default command options')
  .option('-f, --format <format>', 'json or text', 'text')
  .option('-a, --all', 'Show each built-in plugin', false)
  .action(
    (pluginId: string | undefined, opts: { format: string; all: boolean }) => {
      const format = formatOption(opts.format)
      if (opts.all) {
        let ids: string[]
        try {
          ids = scanPlugins().map(item => item.id)
        } catch {
          return fail('builtin-inventory', 'LOAD_FAILED', format)
        }
        for (const id of ids) printPluginInfo(id, format)
      } else if (pluginId) printPluginInfo(pluginId, format)
      else fail('builtin-inventory', 'INPUT_INVALID', format)
    }
  )

program
  .command('run <plugin-id>')
  .description('Execute a declared command; default command: run')
  .option('--command <command-id>', 'Select a declared operation', 'run')
  .option('-f, --format <format>', 'json or text')
  .option(
    '-i, --input <json>',
    'JSON object; cannot combine with operation flags'
  )
  .option('--batch-input <json>', '1–100 JSON objects; sequential execution')
  .option('-t, --timeout <ms>', 'Execution deadline in ms', '30000')

async function executeRun(pluginId: string, args: string[]) {
  let format = requestedRunFormat(args)
  try {
    const options = parseRunArguments(args)
    format = options.format
    // JSON syntax/shape errors are rejected before catalog IO or code import.
    let jsonInput: unknown
    if (options.input !== undefined) {
      if (new TextEncoder().encode(options.input).length > 4_194_304)
        throw new CLIInputError('INVALID_INPUT_SHAPE', 'Input budget exceeded')
      jsonInput = parseJsonInput(options.input)
    }
    if (options.batchInput !== undefined) {
      if (new TextEncoder().encode(options.batchInput).length > 4_194_304)
        throw new CLIInputError('INVALID_INPUT_SHAPE', 'Batch budget exceeded')
      try {
        jsonInput = JSON.parse(options.batchInput)
      } catch {
        throw new CLIInputError('INVALID_JSON', 'Invalid batch JSON')
      }
      if (
        !Array.isArray(jsonInput) ||
        jsonInput.length < 1 ||
        jsonInput.length > 100
      )
        throw new CLIInputError(
          'INVALID_INPUT_SHAPE',
          'Batch must contain 1–100 input objects'
        )
    }
    const manifest = manifestFor(pluginId, format)
    const command = operationFor(manifest, options.commandId, format)
    if (options.help) {
      process.stdout.write(
        `Usage: flowtools run ${pluginId} --command ${command.id} [options]\n\n${manifest.name}\n${command.description}\n\n${commandFlagHelp(command)}\n\nHost options: --input <JSON object>, --batch-input <JSON array>, --format json|text, --timeout <ms>\nBoolean: --flag false or --no-flag. Arrays: repeat --flag or pass a JSON array.\n`
      )
      return
    }
    if (
      !Number.isInteger(options.timeout) ||
      options.timeout < 1 ||
      options.timeout > 2_147_483_647
    )
      fail(pluginId, 'TIMEOUT_INVALID', format)
    const inputs =
      options.batchInput !== undefined
        ? (jsonInput as unknown[]).map(input =>
            validateCommandInput(command, input)
          )
        : [
            options.input !== undefined
              ? validateCommandInput(command, jsonInput)
              : parseCommandFlags(command, options.flags),
          ]
    // Prepare all schema/flag inputs before importing code or creating context.
    const results = []
    for (const input of inputs)
      results.push(
        await runPlugin(pluginId, input, {
          commandId: command.id,
          profile: options.profile,
          timeout: options.timeout,
        })
      )
    if (options.batchInput !== undefined) {
      const success = results.every(result => result.success)
      if (format === 'json')
        writeJson({
          formatVersion: 1,
          type: 'batch',
          identity: commandIdentity(manifest, command),
          success,
          results,
        })
      else for (const result of results) printExecutionResult(result, format)
      process.exit(success ? 0 : 1)
    }
    process.exit(printExecutionResult(results[0]!, format))
  } catch (error) {
    fail(
      pluginId,
      isCLIInputError(error) ? 'INPUT_INVALID' : 'LOAD_FAILED',
      format
    )
  }
}

async function main() {
  const args = process.argv.slice(2)
  if (args[0] === 'run' && args[1] && !args[1].startsWith('-'))
    return executeRun(args[1], args.slice(2))
  await program.parseAsync()
}

main().catch(error => {
  if (
    error &&
    typeof error === 'object' &&
    'code' in error &&
    ['commander.helpDisplayed', 'commander.help', 'commander.version'].includes(
      String(error.code)
    )
  )
    return
  if (
    ['init', 'permissions', 'runtime', 'jobs'].includes(process.argv[2] ?? '')
  ) {
    managementFailure(error)
    return
  }
  fail(
    'builtin-inventory',
    isCLIInputError(error) ||
      (error &&
        typeof error === 'object' &&
        'code' in error &&
        String(error.code).startsWith('commander.'))
      ? 'INPUT_INVALID'
      : 'LOAD_FAILED',
    requestedRunFormat(process.argv.slice(2))
  )
})
