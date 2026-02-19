import type { ReactNode } from 'react'

import type { ToolContext } from './ctx'

/**
 * Runtime mode for command execution.
 * - `panel`: open and render a React panel.
 * - `headless`: execute logic and return a structured result.
 */
export type CommandMode = 'panel' | 'headless'

/**
 * Shared metadata for every command.
 */
export interface CommandBase {
  /**
   * Stable command id in kebab-case.
   */
  id: string
  /**
   * Command title shown in host UI.
   */
  title: string
  /**
   * Optional command description.
   */
  description?: string
}

/**
 * Panel command definition.
 */
export interface PanelCommandDef<In = void, Out = void> extends CommandBase {
  /**
   * Panel mode marker.
   */
  mode: 'panel'
  /**
   * Render function for panel mode commands.
   */
  render: (input: In) => ReactNode
  /**
   * Panel commands do not expose `run`.
   */
  run?: never
  /**
   * Optional output type marker for generics.
   */
  __out?: Out
}

/**
 * Headless command definition.
 */
export interface HeadlessCommandDef<In = void, Out = void>
  extends CommandBase {
  /**
   * Headless mode marker.
   */
  mode: 'headless'
  /**
   * Execute function for tool commands.
   */
  run: (ctx: ToolContext, input: In) => Promise<Out> | Out
  /**
   * Headless commands do not expose `render`.
   */
  render?: never
}

/**
 * Unified command definition type.
 */
export type CommandDef<In = void, Out = void> =
  | PanelCommandDef<In, Out>
  | HeadlessCommandDef<In, Out>
