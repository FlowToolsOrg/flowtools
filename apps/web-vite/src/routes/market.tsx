import { useState } from 'react'

import { extractMeta } from '@flow-tool/sdk'
import {
  MarketEmptyState,
  MarketToolbar,
  ToolCard,
  ToolGrid,
  ToolLayout,
  ToolLayoutMain,
  ToolLayoutSidebar,
  ToolMarketPage,
  ToolSummaryCard,
  type ToolEntity,
} from '@flow-tool/ui'
import { createFileRoute } from '@tanstack/react-router'

import { Button, Chip } from '@heroui/react'

import runHello from '@plugins/plugin-example-run-hello'

import appPlugins from '@/plugin/app'

const allPlugins = [...appPlugins, runHello]

const pathMap: Record<string, string> = {
  'plugin-example-hello-world': '/plugins/hello-world',
  'plugin-todo-list': '/plugins/todo-list',
  'plugin-example-run-hello': '/plugins/run-hello',
}

const tools: ToolEntity[] = extractMeta(allPlugins, pathMap).map(meta => ({
  id: meta.id,
  name: meta.name,
  description: meta.description ?? '',
  version: meta.version,
  status: meta.status ?? 'stable',
  category: meta.category,
  tags: meta.tags,
  permissions: meta.permissions?.map(p => ({ id: p, label: p })),
  isInstalled: true,
}))

type FilterMode = 'all' | 'installed' | 'beta'

export const Route = createFileRoute('/market')({
  component: MarketPage,
})

function MarketPage() {
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<FilterMode>('all')
  const [selectedToolId, setSelectedToolId] = useState(tools[0]?.id ?? '')

  const filteredTools = tools.filter(tool => {
    const matchesQuery =
      query.trim().length === 0 ||
      tool.name.toLowerCase().includes(query.toLowerCase()) ||
      tool.description.toLowerCase().includes(query.toLowerCase())

    const matchesFilter =
      filter === 'all'
        ? true
        : filter === 'installed'
          ? !!tool.isInstalled
          : tool.status === 'beta'

    return matchesQuery && matchesFilter
  })

  const selectedTool =
    filteredTools.find(tool => tool.id === selectedToolId) ??
    filteredTools[0] ??
    null

  return (
    <ToolLayout>
      <ToolLayoutMain>
        <ToolMarketPage>
          <ToolMarketPage.Header>
            <h1 className="text-2xl font-semibold text-slate-900">
              Tool Market
            </h1>
            <p className="text-sm text-slate-500">
              Browse available tools and preview metadata from @flow-tool/ui
              market components.
            </p>
          </ToolMarketPage.Header>
          <ToolMarketPage.Content>
            <MarketToolbar>
              <MarketToolbar.Search
                ariaLabel="Search market tools"
                onChange={setQuery}
                value={query}
              />
              <MarketToolbar.Filters>
                <Button
                  onPress={() => setFilter('all')}
                  size="sm"
                  variant={filter === 'all' ? 'primary' : 'ghost'}
                >
                  All
                </Button>
                <Button
                  onPress={() => setFilter('installed')}
                  size="sm"
                  variant={filter === 'installed' ? 'primary' : 'ghost'}
                >
                  Installed
                </Button>
                <Button
                  onPress={() => setFilter('beta')}
                  size="sm"
                  variant={filter === 'beta' ? 'primary' : 'ghost'}
                >
                  Beta
                </Button>
              </MarketToolbar.Filters>
              <MarketToolbar.Actions>
                <Chip size="sm" variant="tertiary">
                  {filteredTools.length} tools
                </Chip>
              </MarketToolbar.Actions>
            </MarketToolbar>

            {filteredTools.length === 0 ? (
              <MarketEmptyState
                title="No tools matched"
                description="Try clearing the search text or switching filter mode."
                action={
                  <Button
                    onPress={() => setFilter('all')}
                    size="sm"
                    variant="outline"
                  >
                    Reset Filters
                  </Button>
                }
              />
            ) : (
              <ToolGrid>
                {filteredTools.map(tool => (
                  <ToolGrid.Item key={tool.id}>
                    <ToolCard onPress={() => setSelectedToolId(tool.id)}>
                      <ToolCard.Header>
                        <ToolCard.Title>{tool.name}</ToolCard.Title>
                        <ToolCard.Description>
                          {tool.description}
                        </ToolCard.Description>
                      </ToolCard.Header>
                      <ToolCard.Meta
                        status={tool.status}
                        version={tool.version}
                      >
                        {tool.category ? (
                          <Chip size="sm" variant="tertiary">
                            {tool.category}
                          </Chip>
                        ) : null}
                      </ToolCard.Meta>
                      <ToolCard.Tags>
                        {tool.tags?.map(tag => (
                          <Chip key={tag} size="sm" variant="secondary">
                            {tag}
                          </Chip>
                        ))}
                      </ToolCard.Tags>
                    </ToolCard>
                  </ToolGrid.Item>
                ))}
              </ToolGrid>
            )}
          </ToolMarketPage.Content>
        </ToolMarketPage>
      </ToolLayoutMain>
      <ToolLayoutSidebar>
        {selectedTool ? (
          <ToolSummaryCard
            category={selectedTool.category}
            description={selectedTool.description}
            status={selectedTool.status}
            title={selectedTool.name}
            version={selectedTool.version}
            actions={
              <>
                <Button size="sm">Install</Button>
                <Button size="sm" variant="outline">
                  Details
                </Button>
              </>
            }
          />
        ) : null}
      </ToolLayoutSidebar>
    </ToolLayout>
  )
}
