import { PLUGIN_FEATURE } from '../constants/marker'

export type FeatureTag = string

export type PluginFeature<
  MainFn extends (...args: any[]) => any = (...args: any[]) => any,
> = {
  name: string
  description?: string
  version?: string
  tags?: FeatureTag[]
  main: MainFn
}

export type MarkedPluginFeature = PluginFeature & {
  [PLUGIN_FEATURE]: true
}
