import { useState } from 'react'

import {
  RunInputPanel,
  RunLogList,
  RunPanel,
  RunResultPanel,
  RunStatusStrip,
  ToolDetailPage,
  ToolPermissionList,
  ToolSummaryCard,
  ToolVersionTimeline,
  type RunLogEntry,
  type RunResultPayload,
  type RunStatus,
  type ToolVersionRecord,
} from '@flowtools/ui'
import {
  ArrowLeftIcon,
  CircleCheckIcon,
  EyeIcon,
  PlayIcon,
  SparklesIcon,
} from '@flowtools/ui/icons'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useStore } from 'zustand'

import { Button, Chip, Tabs } from '@heroui/react'

import { renderWebAppPlugin } from '@/runtime'
import { pluginRegistryStore } from '@/stores/plugin-registry-store'

const versions: ToolVersionRecord[] = [
  {
    id: 'v1.0.0',
    version: '1.0.0',
    date: '2026-01-15',
    notes: 'Initial release',
  },
  {
    id: 'v1.1.0',
    version: '1.1.0',
    date: '2026-02-10',
    notes: 'Bug fixes and improvements',
  },
]

export const Route = createFileRoute('/tools/$toolId')({
  component: ToolDetailPage_,
})

function ToolDetailPage_() {
  const { toolId } = Route.useParams()
  const navigate = useNavigate()
  const [activeTab, setActiveTab] = useState('overview')
  const { plugins } = useStore(pluginRegistryStore)

  const registered = plugins.find(p => p.id === toolId)
  const plugin = registered?.plugin
  const meta = registered?.manifest

  const [input, setInput] = useState('')
  const [status, setStatus] = useState<RunStatus>('idle')
  const [statusMessage, setStatusMessage] = useState('Waiting for input')
  const [result, setResult] = useState<RunResultPayload>({
    title: 'No Result Yet',
    summary: 'Run the tool to see output.',
  })
  const [logs, setLogs] = useState<RunLogEntry[]>([])

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
        message: 'Run rejected: input is empty',
        timestamp,
      })
      return
    }

    setStatus('running')
    setStatusMessage('Executing...')

    setTimeout(() => {
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
        timestamp: new Date().toLocaleTimeString(),
      })
    }, 300)
  }

  if (!meta) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-4 p-8">
        <h1 className="text-xl font-semibold text-(--foreground)">
          Tool Not Found
        </h1>
        <p className="text-sm text-(--muted)">
          No tool with id &quot;{toolId}&quot; was found.
        </p>
        <Button onPress={() => navigate({ to: '/tools' })} size="sm">
          Back to Tools
        </Button>
      </div>
    )
  }

  const isAppPlugin = plugin?.type === 'app'
  const StatusIcon = CircleCheckIcon

  return (
    <div className="flex flex-col gap-6 p-6 lg:p-8">
      <header className="flex items-center gap-3">
        <Button
          onPress={() => navigate({ to: '/tools' })}
          size="sm"
          variant="ghost"
        >
          <ArrowLeftIcon size={16} />
          Back
        </Button>
        <div className="h-4 w-px bg-separator" />
        <div className="flex items-center gap-2">
          <h1 className="text-xl font-semibold text-(--foreground)">
            {meta.name}
          </h1>
          <Chip color="success" size="sm" variant="soft">
            <span className="flex items-center gap-1">
              <StatusIcon size={12} />
              stable
            </span>
          </Chip>
          <Chip size="sm" variant="secondary">
            v{meta.version}
          </Chip>
        </div>
      </header>

      <Tabs
        onSelectionChange={key => setActiveTab(String(key))}
        selectedKey={activeTab}
      >
        <Tabs.ListContainer>
          <Tabs.List aria-label="Tool sections">
            <Tabs.Tab id="overview">
              <span className="flex items-center gap-1.5">
                <EyeIcon size={14} />
                Overview
              </span>
              <Tabs.Indicator />
            </Tabs.Tab>
            <Tabs.Tab id="run">
              <span className="flex items-center gap-1.5">
                <PlayIcon size={14} />
                Run
              </span>
              <Tabs.Indicator />
            </Tabs.Tab>
          </Tabs.List>
        </Tabs.ListContainer>
        <Tabs.Panel id="overview">
          <ToolDetailPage>
            <ToolDetailPage.Content>
              <div className="space-y-4">
                <ToolSummaryCard
                  category={meta.category}
                  description={meta.description}
                  status="stable"
                  title={meta.name}
                  version={meta.version}
                />

                {meta.permissions && meta.permissions.length > 0 ? (
                  <section className="space-y-2">
                    <h2 className="text-sm font-semibold text-(--foreground)">
                      Permissions
                    </h2>
                    <ToolPermissionList>
                      {meta.permissions.map(perm => (
                        <ToolPermissionList.Item
                          key={perm}
                          name={perm}
                          permissionId={perm}
                        />
                      ))}
                    </ToolPermissionList>
                  </section>
                ) : null}

                <section className="space-y-2">
                  <h2 className="text-sm font-semibold text-(--foreground)">
                    Version History
                  </h2>
                  <ToolVersionTimeline records={versions} />
                </section>
              </div>

              <div className="space-y-4">
                {meta.tags && meta.tags.length > 0 ? (
                  <section className="space-y-2">
                    <h2 className="text-sm font-semibold text-(--foreground)">
                      Tags
                    </h2>
                    <div className="flex flex-wrap gap-2">
                      {meta.tags.map(tag => (
                        <Chip key={tag} size="sm" variant="tertiary">
                          {tag}
                        </Chip>
                      ))}
                    </div>
                  </section>
                ) : null}
              </div>
            </ToolDetailPage.Content>
          </ToolDetailPage>
        </Tabs.Panel>

        <Tabs.Panel id="run">
          {isAppPlugin && plugin ? (
            <div className="rounded-(--radius) border border-(--border) bg-(--surface) p-4">
              {renderWebAppPlugin(plugin)}
            </div>
          ) : (
            <RunPanel>
              <RunPanel.Content>
                <RunStatusStrip
                  duration={status === 'success' ? '24ms' : undefined}
                  message={statusMessage}
                  onReset={() => {
                    setStatus('idle')
                    setStatusMessage('Waiting for input')
                    setResult({
                      title: 'No Result Yet',
                      summary: 'Run the tool to see output.',
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
                <RunResultPanel result={result} />
                <RunLogList entries={logs} />
              </RunPanel.Content>
              <RunPanel.Footer>
                <Button
                  onPress={() => setInput('demo-input')}
                  size="sm"
                  variant="ghost"
                >
                  <SparklesIcon size={16} />
                  Fill Demo Input
                </Button>
              </RunPanel.Footer>
            </RunPanel>
          )}
        </Tabs.Panel>
      </Tabs>
    </div>
  )
}
