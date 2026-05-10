export const PLUGIN_MARKER = Symbol('__flow_tools__')
export const PLUGIN_FEATURE = Symbol('__plugin_feature__')
export const IMPORT_MAP_ID = '__flowtools_importmap__'
export const SDK_BRIDGE_ID = '__flowtools_sdk_bridge__'

export const APP_PREFIX = 'flowtools'
export const GLOBAL_KEY = '__FLOWTOOLS__' // ! cannot use Symbol as key, because it use in template string

export const SDK_MARKERS = {
  PLUGIN_MARKER,
  PLUGIN_FEATURE,
  IMPORT_MAP_ID,
  SDK_BRIDGE_ID,
  GLOBAL_KEY,
} as const
