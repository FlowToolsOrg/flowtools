import { useState } from 'react'

import { resolvePluginMaturity } from '@flowtools/sdk/types'
import {
  MarketEmptyState,
  MarketToolbar,
  ToolCard,
  ToolGrid,
  ToolSummaryCard,
  type ToolEntity,
} from '@flowtools/ui'
import {
  BlocksIcon,
  CircleCheckIcon,
  FlaskIcon,
  LayersIcon,
} from '@flowtools/ui/icons'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useStore } from 'zustand'

import { Button, Chip } from '@heroui/react'

import { pluginRegistryStore } from '@/stores/plugin-registry-store'

type FilterMode = 'all' | 'installed' | 'beta'

export const Route = createFileRoute('/tools/')({
  component: ToolsPage,
})

function ToolsPage() {
  const navigate = useNavigate()
  const { plugins } = useStore(pluginRegistryStore)
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<FilterMode>('all')
  const [category, setCategory] = useState('all')

  const tools: ToolEntity[] = plugins
    .filter(p => p.state === 'enabled')
    .map(p => ({
      id: p.id,
      name: p.manifest.name,
      description: p.manifest.description ?? '',
      version: p.manifest.version,
      status: resolvePluginMaturity(p.manifest.maturity),
      category: p.manifest.category,
      tags: p.manifest.tags,
      permissions: p.manifest.permissions?.map(perm => ({
        id: perm,
        label: perm,
      })),
      isInstalled: true,
    }))

  const [selectedToolId, setSelectedToolId] = useState(tools[0]?.id ?? '')

  const categories = [
    'all',
    ...new Set(tools.map(t => t.category).filter(Boolean)),
  ] as string[]

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

    const matchesCategory = category === 'all' || tool.category === category

    return matchesQuery && matchesFilter && matchesCategory
  })

  const selectedTool =
    filteredTools.find(tool => tool.id === selectedToolId) ??
    filteredTools[0] ??
    null

  return (
    <div className="grid min-h-0 flex-1 gap-6 p-6 lg:grid-cols-[minmax(0,1fr)_20rem] lg:p-8">
      <div className="flex min-w-0 flex-col gap-4">
        <header className="flex items-center gap-2.5 space-y-0">
          <BlocksIcon className="text-accent" size={22} />
          <div>
            <h1 className="text-2xl font-semibold text-foreground">Tools</h1>
            <p className="text-sm text-muted">
              Browse and run available tools.
            </p>
          </div>
        </header>

        <MarketToolbar>
          <MarketToolbar.Search
            ariaLabel="Search tools"
            onChange={setQuery}
            value={query}
          />
          <MarketToolbar.Filters>
            <Button
              onPress={() => setFilter('all')}
              size="sm"
              variant={filter === 'all' ? 'primary' : 'ghost'}
            >
              <LayersIcon size={14} />
              All
            </Button>
            <Button
              onPress={() => setFilter('installed')}
              size="sm"
              variant={filter === 'installed' ? 'primary' : 'ghost'}
            >
              <CircleCheckIcon size={14} />
              Installed
            </Button>
            <Button
              onPress={() => setFilter('beta')}
              size="sm"
              variant={filter === 'beta' ? 'primary' : 'ghost'}
            >
              <FlaskIcon size={14} />
              Beta
            </Button>
          </MarketToolbar.Filters>
          <MarketToolbar.Actions>
            <Chip size="sm" variant="tertiary">
              {filteredTools.length} tools
            </Chip>
          </MarketToolbar.Actions>
        </MarketToolbar>

        <div className="flex flex-wrap gap-2">
          {categories.map(cat => (
            <Button
              key={cat}
              onPress={() => setCategory(cat)}
              size="sm"
              variant={category === cat ? 'primary' : 'ghost'}
            >
              {cat === 'all' ? 'All Categories' : cat}
            </Button>
          ))}
        </div>

        {filteredTools.length === 0 ? (
          <MarketEmptyState
            title="No tools matched"
            description="Try clearing the search text or switching filter mode."
            action={
              <Button
                onPress={() => {
                  setFilter('all')
                  setCategory('all')
                  setQuery('')
                }}
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
                <ToolCard
                  onPress={() => {
                    setSelectedToolId(tool.id)
                    void navigate({ to: `/tools/${tool.id}` })
                  }}
                >
                  <ToolCard.Header>
                    <ToolCard.Title>{tool.name}</ToolCard.Title>
                    <ToolCard.Description>
                      {tool.description}
                    </ToolCard.Description>
                  </ToolCard.Header>
                  <ToolCard.Meta status={tool.status} version={tool.version}>
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
      </div>

      <aside className="hidden min-w-0 lg:block">
        <div className="sticky top-6">
          {selectedTool ? (
            <ToolSummaryCard
              category={selectedTool.category}
              description={selectedTool.description}
              status={selectedTool.status}
              title={selectedTool.name}
              version={selectedTool.version}
              actions={
                <>
                  <Button
                    onPress={() => {
                      void navigate({ to: `/tools/${selectedTool.id}` })
                    }}
                    size="sm"
                  >
                    Open
                  </Button>
                  <Button size="sm" variant="outline">
                    Install
                  </Button>
                </>
              }
            />
          ) : null}
        </div>
      </aside>
    </div>
  )
}
