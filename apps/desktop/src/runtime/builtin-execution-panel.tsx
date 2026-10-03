import type { FlowToolPlugin } from '@flowtools/sdk/types'

import { describeInputSchema } from '@flowtools/sdk/execution'
import { ExecutionPanel } from '@flowtools/ui'

import { desktopExecutionHistory } from './execution-history'
import { runDesktopPlugin } from './plugin-execution'

export function BuiltinExecutionPanel({ plugin }: { plugin: FlowToolPlugin }) {
  return (
    <ExecutionPanel
      meta={plugin.meta}
      history={desktopExecutionHistory}
      inputSchema={describeInputSchema(plugin.inputSchema)}
      execute={(input, signal) => runDesktopPlugin(plugin, input, { signal })}
    />
  )
}
