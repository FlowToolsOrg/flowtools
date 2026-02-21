// TODO(review): waiting code review

export interface ToolVersionRecord {
  id: string
  version: string
  date: string
  notes?: string
}

export interface ToolPermissionRecord {
  id: string
  name: string
  description?: string
  required?: boolean
}

export interface ToolRelatedRecord {
  id: string
  name: string
  description?: string
  category?: string
}
