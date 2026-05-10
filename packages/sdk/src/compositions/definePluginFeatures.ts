import { PLUGIN_FEATURE } from '../constants'
import { PluginFeature } from '../types/plugin-features'

export function definePluginFeatures<T extends PluginFeature>(
  features: T[]
): (T & { [PLUGIN_FEATURE]: true })[] {
  return features.map(feature => ({
    ...feature,
    [PLUGIN_FEATURE]: true,
  }))
}
