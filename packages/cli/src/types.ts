/**
 * CLI-internal types for Flow Tool CLI.
 */

export type OutputFormat = 'json' | 'text'

export interface CLIPluginInfo {
  id: string
  name: string
  version: string
  description?: string
  type: 'app' | 'tool'
  hasRun: boolean
  hasSchema: boolean
}

export interface RunOptions {
  format: OutputFormat
  input?: string
  timeout?: number
}

export interface ListOptions {
  format: OutputFormat
}
