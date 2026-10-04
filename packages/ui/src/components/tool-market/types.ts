import type { PluginMaturity } from '@flowtools/sdk/types'

export type ToolMarketStatus = PluginMaturity

export interface ToolPermissionTag {
  id: string
  label: string
}

export interface ToolEntity {
  id: string
  name: string
  description: string
  category?: string
  status: ToolMarketStatus
  version?: string
  tags?: string[]
  permissions?: ToolPermissionTag[]
  isInstalled?: boolean
  isPinned?: boolean
}

export interface MarketFilterOption {
  id: string
  label: string
  count?: number
}
