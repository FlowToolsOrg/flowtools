import type {
  AppPlugin,
  EnforceUniquePluginPermissions,
  ToolPlugin,
} from '../types/plugin'

/**
 * Marker field added by `definePlugin`.
 */
export interface FlowToolPluginMarker {
  /**
   * Flow Tool plugin signature marker.
   */
  readonly __flow_tools__: true
}

/**
 * Plugin type after `definePlugin` tagging.
 */
type AnyFlowToolPlugin = AppPlugin | ToolPlugin<never, any>

export type DefinedFlowToolPlugin<
  T extends AnyFlowToolPlugin = AnyFlowToolPlugin,
> = T & FlowToolPluginMarker

/**
 * Define a plugin contract for Flow Tool runtime.
 * @example
 * export default definePlugin({
 *  type: "app",
 *  meta: {
 *    name: "app",
 *    version: "1.0.0",
 *    id: "app",
 *  },
 *  setup() {
 *    return () => <>...</>
 *  }
 *})
 */
export function definePlugin<const T extends AnyFlowToolPlugin>(
  plugin: EnforceUniquePluginPermissions<T>
): DefinedFlowToolPlugin<T> {
  const normalizedPlugin = plugin as unknown as T
  ;(normalizedPlugin as T & { __flow_tools__?: true }).__flow_tools__ = true

  return normalizedPlugin as DefinedFlowToolPlugin<T>
}
