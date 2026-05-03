/**
 * @flow-tool/cli — public API.
 * Import this module to use the CLI as a library (e.g., from Tauri desktop host).
 */

export { createCLIToolContext } from './context'
export { scanPlugins, loadPlugin } from './discovery'
export { formatResult } from './formatter'
export {
  addSchemaFlags,
  buildInputFromOptions,
  parseJsonInput,
  introspectSchema,
  toKebab,
  generateMockFromSchema,
  buildFlagExample,
} from './schema'
export type { FieldMeta } from './schema'
export { runPlugin, runPluginAndPrint } from './runner'
export type { RunResult } from './runner'
export type {
  CLIPluginInfo,
  OutputFormat,
  RunOptions,
  ListOptions,
} from './types'
