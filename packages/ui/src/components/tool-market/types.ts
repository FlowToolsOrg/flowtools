// TODO(review): waiting code review

export type ToolMarketStatus = 'stable' | 'beta' | 'experimental' | 'deprecated'

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
