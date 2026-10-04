/**
 * @flowtools/cli — public API.
 * Import this module to use the CLI as a library (e.g., from Tauri desktop host).
 */

export { createCLIToolContext } from './context'
export { scanPlugins, loadPlugin, getBuiltinCommandManifest } from './discovery'
export {
  commandFlags,
  commandExample,
  parseCommandFlags,
  validateCommandInput,
  commandFlagHelp,
} from './command-schema'
export { formatResult } from './formatter'
export {
  addSchemaFlags,
  buildInputFromOptions,
  CLIInputError,
  parseJsonInput,
  introspectSchema,
  toKebab,
  generateMockFromSchema,
  buildFlagExample,
} from './schema'
export type { CLIInputErrorCode, FieldMeta } from './schema'
export { createPluginRunner, runPlugin, runPluginAndPrint } from './runner'
export type {
  RunResult,
  PluginRunnerDependencies,
  PluginRunOptions,
} from './runner'
export type {
  CLIPluginInfo,
  OutputFormat,
  RunOptions,
  ListOptions,
} from './types'
