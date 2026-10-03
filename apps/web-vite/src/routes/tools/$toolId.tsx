import { useState } from 'react'

import { describeInputSchema } from '@flowtools/sdk/execution'
import {
  ExecutionPanel,
  ToolDetailPage,
  ToolPermissionList,
  ToolSummaryCard,
} from '@flowtools/ui'
import {
  ArrowLeftIcon,
  CircleCheckIcon,
  EyeIcon,
  PlayIcon,
} from '@flowtools/ui/icons'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useStore } from 'zustand'

import { Button, Chip, Tabs } from '@heroui/react'

import { renderWebAppPlugin, runWebPlugin } from '@/runtime'
import { pluginRegistryStore } from '@/stores/plugin-registry-store'
import { runHistoryStore } from '@/stores/run-history-store'

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

  if (!meta) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-4 p-8">
        <h1 className="text-xl font-semibold text-foreground">
          Tool Not Found
        </h1>
        <p className="text-sm text-muted">
          No tool with id &quot;{toolId}&quot; was found.
        </p>
        <Button onPress={() => navigate({ to: '/tools' })} size="sm">
          Back to Tools
        </Button>
      </div>
    )
  }

  if (registered.state !== 'enabled') {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-4 p-8">
        <h1 className="text-xl font-semibold text-foreground">
          Plugin Disabled
        </h1>
        <p className="text-sm text-muted">
          &quot;{meta.name}&quot; is currently {registered.state}. Enable it to
          use this tool.
        </p>
        <div className="flex gap-2">
          <Button
            onPress={() => navigate({ to: '/plugins' })}
            size="sm"
            variant="ghost"
          >
            Manage Plugins
          </Button>
          <Button
            onPress={async () => {
              await pluginRegistryStore.getState().enablePlugin(toolId)
            }}
            size="sm"
          >
            Enable Plugin
          </Button>
        </div>
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
          <h1 className="text-xl font-semibold text-foreground">{meta.name}</h1>
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
                    <h2 className="text-sm font-semibold text-foreground">
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
                  <h2 className="text-sm font-semibold text-foreground">
                    Version History
                  </h2>
                  <p className="text-sm text-muted">
                    Loaded v{meta.version}; release history is not provided.
                  </p>
                </section>
              </div>

              <div className="space-y-4">
                {meta.tags && meta.tags.length > 0 ? (
                  <section className="space-y-2">
                    <h2 className="text-sm font-semibold text-foreground">
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
          <div className="space-y-4">
            {isAppPlugin && plugin ? (
              <div className="rounded-(--radius) border border-border bg-surface p-4">
                {renderWebAppPlugin(plugin)}
              </div>
            ) : null}
            {plugin ? (
              <ExecutionPanel
                key={plugin.meta.id}
                meta={plugin.meta}
                inputSchema={describeInputSchema(plugin.inputSchema)}
                history={runHistoryStore}
                execute={(input, signal) =>
                  runWebPlugin(plugin, input, { signal })
                }
              />
            ) : (
              <p role="alert">
                Actual plugin code is not loaded; no execution is available.
              </p>
            )}
          </div>
        </Tabs.Panel>
      </Tabs>
    </div>
  )
}
