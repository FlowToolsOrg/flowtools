import type { FlowToolPlugin } from '@flowtools/sdk/types'

import { describeInputSchema } from '@flowtools/sdk/execution'
import { ExecutionPanel } from '@flowtools/ui'

import { desktopExecutionHistory } from './execution-history'
import { useLauncherInputPreset } from './launcher-input'
import { runDesktopPlugin } from './plugin-execution'

export function BuiltinExecutionPanel({ plugin }: { plugin: FlowToolPlugin }) {
  const preset = useLauncherInputPreset(plugin.meta.id)
  return (
    <ExecutionPanel
      key={preset?.key ?? plugin.meta.id}
      initialInput={preset?.input}
      meta={plugin.meta}
      history={desktopExecutionHistory}
      inputSchema={describeInputSchema(plugin.inputSchema)}
      execute={(input, signal) => runDesktopPlugin(plugin, input, { signal })}
    />
  )
}
