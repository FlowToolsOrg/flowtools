import type { FlowToolPlugin, PluginMeta } from '../types/plugin'

export interface ExtractedMeta extends PluginMeta {
  path?: string
}

export function extractMeta(
  plugins: FlowToolPlugin[],
  pathMap?: Record<string, string>
): ExtractedMeta[] {
  return plugins.map(plugin => ({
    ...plugin.meta,
    path: pathMap?.[plugin.meta.id],
  }))
}
