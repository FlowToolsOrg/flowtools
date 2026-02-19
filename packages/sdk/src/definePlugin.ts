import type { FlowToolPlugin } from "./types/plugin";

/**
 * Marker field added by `definePlugin`.
 */
export interface FlowToolPluginMarker {
  /**
   * Flow Tool plugin signature marker.
   */
  readonly __flow_tool: true;
}

/**
 * Plugin type after `definePlugin` tagging.
 */
export type DefinedFlowToolPlugin<T extends FlowToolPlugin = FlowToolPlugin> = T &
  FlowToolPluginMarker;

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
export function definePlugin<T extends FlowToolPlugin>(plugin: T): DefinedFlowToolPlugin<T> {
  (plugin as T & { __flow_tool?: true }).__flow_tool = true;

  return plugin as DefinedFlowToolPlugin<T>;
}
