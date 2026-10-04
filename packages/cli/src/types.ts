/**
 * CLI-internal types for Flow Tool CLI.
 */
import type { PluginMaturity } from '@flowtools/sdk/types'

export type OutputFormat = 'json' | 'stdio' | 'text'

export interface CLIPluginInfo {
  id: string
  name: string
  version: string
  maturity: PluginMaturity
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
