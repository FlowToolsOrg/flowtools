import type { OutputFormat } from './types'

import { CLIInputError } from './schema'

export interface RunArguments {
  profile?: string
  commandId: string
  format: OutputFormat
  input?: string
  batchInput?: string
  timeout: number
  flags: string[]
  help: boolean
}

/** Host options are parsed separately from declared operation flags. */
export function parseRunArguments(args: string[]): RunArguments {
  const host = new Map<string, string>()
  const names: Record<string, string> = {
    '-f': 'format',
    '--format': 'format',
    '-i': 'input',
    '--input': 'input',
    '-t': 'timeout',
    '--timeout': 'timeout',
    '--command': 'command',
    '--batch-input': 'batch',
    '--profile': 'profile',
  }
  const flags: string[] = []
  let help = false
  for (let i = 0; i < args.length; i++) {
    const token = args[i]!
    if (token === '--help' || token === '-h') {
      help = true
      continue
    }
    const equal = token.indexOf('=')
    const name = equal === -1 ? token : token.slice(0, equal)
    const key = Object.prototype.hasOwnProperty.call(names, name)
      ? names[name]
      : undefined
    if (!key) {
      flags.push(token)
      continue
    }
    if (host.has(key))
      throw new CLIInputError('INVALID_INPUT_SHAPE', 'Duplicate host option')
    const value = equal === -1 ? args[++i] : token.slice(equal + 1)
    if (value === undefined || value.startsWith('--'))
      throw new CLIInputError(
        'INVALID_INPUT_SHAPE',
        'Missing host option value'
      )
    host.set(key, value)
  }
  const input = host.get('input'),
    batchInput = host.get('batch')
  if (
    (input !== undefined && batchInput !== undefined) ||
    ((input !== undefined || batchInput !== undefined) && flags.length)
  )
    throw new CLIInputError(
      'INVALID_INPUT_SHAPE',
      'Choose JSON input, batch input, or flags'
    )
  const format =
    host.get('format') ??
    (input !== undefined || batchInput !== undefined ? 'json' : 'text')
  if (!['text', 'stdio', 'json'].includes(format))
    throw new CLIInputError('INVALID_INPUT_SHAPE', 'Unsupported format')
  const commandId = host.get('command') ?? 'run'
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(commandId))
    throw new CLIInputError('INVALID_INPUT_SHAPE', 'Invalid command identity')
  return {
    profile: host.get('profile'),
    commandId,
    format: format as OutputFormat,
    input,
    batchInput,
    timeout: Number(host.get('timeout') ?? '30000'),
    flags,
    help,
  }
}

/** A malformed invocation requesting JSON must still emit a structured failure. */
export function requestedRunFormat(args: string[]): OutputFormat {
  for (let i = 0; i < args.length; i++)
    if (
      (['--format', '-f'].includes(args[i]!) && args[i + 1] === 'json') ||
      args[i] === '--format=json'
    )
      return 'json'
  return args.some(
    token =>
      ['--input', '-i', '--batch-input'].includes(token) ||
      token.startsWith('--input=') ||
      token.startsWith('--batch-input=')
  )
    ? 'json'
    : 'text'
}
