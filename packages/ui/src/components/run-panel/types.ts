// TODO(review): waiting code review

export type RunStatus = 'idle' | 'running' | 'success' | 'error'

export interface RunLogEntry {
  id: string
  level: 'info' | 'warn' | 'error'
  message: string
  timestamp: string
}

export interface RunHistoryEntry {
  id: string
  title: string
  startedAt: string
  status: RunStatus
}

export interface RunResultPayload {
  title: string
  summary?: string
  raw?: string
}
