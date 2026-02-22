import { useState } from 'react'

import {
  RunHistoryPanel,
  RunInputPanel,
  RunLogList,
  RunPanel,
  RunResultPanel,
  RunStatusStrip,
  ToolLayout,
  ToolLayoutMain,
  ToolLayoutSidebar,
  type RunHistoryEntry,
  type RunLogEntry,
  type RunResultPayload,
  type RunStatus,
} from '@flow-tool/ui'
import { createFileRoute } from '@tanstack/react-router'

import { Button } from '@heroui/react'

export const Route = createFileRoute('/run')({
  component: RunPage,
})

function RunPage() {
  const [input, setInput] = useState('')
  const [status, setStatus] = useState<RunStatus>('idle')
  const [statusMessage, setStatusMessage] = useState('Waiting for input')
  const [result, setResult] = useState<RunResultPayload>({
    title: 'No Result Yet',
    summary: 'Run a tool to generate output.',
  })
  const [logs, setLogs] = useState<RunLogEntry[]>([])
  const [history, setHistory] = useState<RunHistoryEntry[]>([])
  const [selectedHistoryId, setSelectedHistoryId] = useState<string>('')

  const appendLog = (entry: RunLogEntry) => {
    setLogs(current => [entry, ...current].slice(0, 8))
  }

  const handleRun = () => {
    const now = new Date()
    const timestamp = now.toLocaleTimeString()

    if (!input.trim()) {
      setStatus('error')
      setStatusMessage('Input cannot be empty')
      appendLog({
        id: `${Date.now()}-error`,
        level: 'error',
        message: 'Run rejected because input is empty',
        timestamp,
      })
      return
    }

    const output = input.toUpperCase()

    setStatus('success')
    setStatusMessage('Execution completed')
    setResult({
      title: 'Execution Result',
      summary: `Processed ${input.length} characters`,
      raw: output,
    })

    appendLog({
      id: `${Date.now()}-success`,
      level: 'info',
      message: `Ran input: ${input}`,
      timestamp,
    })

    setHistory(current => [
      {
        id: `${Date.now()}`,
        title: `Run: ${input.slice(0, 18)}`,
        startedAt: now.toLocaleString(),
        status: 'success',
      },
      ...current,
    ])
  }

  return (
    <ToolLayout>
      <ToolLayoutMain>
        <RunPanel>
          <RunPanel.Header>
            <h1 className="text-2xl font-semibold text-slate-900">Run Panel</h1>
            <p className="text-sm text-slate-500">
              Run tools and inspect result, logs, status and history.
            </p>
          </RunPanel.Header>
          <RunPanel.Content>
            <RunStatusStrip
              duration={status === 'success' ? '24ms' : undefined}
              message={statusMessage}
              onReset={() => {
                setStatus('idle')
                setStatusMessage('Waiting for input')
                setResult({
                  title: 'No Result Yet',
                  summary: 'Run a tool to generate output.',
                })
              }}
              status={status}
            />
            <RunInputPanel
              label="Tool Input"
              onChange={setInput}
              onRun={handleRun}
              value={input}
            />
            <RunResultPanel
              onCopy={() => {
                appendLog({
                  id: `${Date.now()}-copy`,
                  level: 'warn',
                  message: 'Result copied action triggered',
                  timestamp: new Date().toLocaleTimeString(),
                })
              }}
              result={result}
            />
            <RunLogList
              entries={logs}
              onSelect={entryId => {
                setStatus('running')
                setStatusMessage(`Inspecting log: ${entryId}`)
              }}
            />
          </RunPanel.Content>
          <RunPanel.Footer>
            <Button
              onPress={() => setInput('demo-input')}
              size="sm"
              variant="ghost"
            >
              Fill Demo Input
            </Button>
          </RunPanel.Footer>
        </RunPanel>
      </ToolLayoutMain>
      <ToolLayoutSidebar>
        <h3 className="text-sm font-semibold text-slate-900">Run History</h3>
        <RunHistoryPanel entries={history} onSelect={setSelectedHistoryId} />
        <div className="rounded-2xl border border-slate-200/80 bg-white p-3 text-xs text-slate-600">
          Selected history id: <strong>{selectedHistoryId || '-'}</strong>
        </div>
      </ToolLayoutSidebar>
    </ToolLayout>
  )
}
