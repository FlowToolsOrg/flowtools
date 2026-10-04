import type {
  Permission,
  PluginMaturity,
  CompatibilityEvidenceStatus,
} from '@flowtools/sdk/types'

export type CommandSource = 'system' | 'html' | 'react'

export interface IndexedCommand {
  id: string
  title: string
  description?: string
  type: string
  pluginId: string
  pluginName: string
  category?: string
  pluginType: 'app' | 'tool'
  compatibilityLevel: string
  maturity?: PluginMaturity
  compatibilityEvidence?: CompatibilityEvidenceStatus
  source: CommandSource
  sourceDir?: string
  assetDir?: string
  main?: string
  mainAvailable?: boolean
  preload?: string
  developmentMain?: string
  permissions: readonly Permission[]
  featureCode?: string
  hasUi: boolean
  hasPreload: boolean
  requiresNative: boolean
}
