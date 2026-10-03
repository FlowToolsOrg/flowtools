import { createBrowserExecutionHistory } from '@flowtools/sdk/execution'

// The old flowtools-run-history key remains untouched and is not verified data.
export const runHistoryStore = createBrowserExecutionHistory(
  'flowtools-web-run-history-v1'
)
