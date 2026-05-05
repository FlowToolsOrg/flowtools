import { ToolCard, ToolGrid } from '@flowtools/ui'
import {
  EarthIcon,
  GalleryThumbnailsIcon,
  ScanTextIcon,
  WrenchIcon,
} from '@flowtools/ui/icons'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useStore } from 'zustand'

import { Chip } from '@heroui/react'

import { pluginCategories } from '@/plugin/manifests'
import { pluginRegistryStore } from '@/stores/plugin-registry-store'

const categoryIcons: Record<string, typeof ScanTextIcon> = {
  text: ScanTextIcon,
  image: GalleryThumbnailsIcon,
  network: EarthIcon,
  utility: WrenchIcon,
}

export const Route = createFileRoute('/')({
  component: Dashboard,
})

function Dashboard() {
  const navigate = useNavigate()
  const { plugins } = useStore(pluginRegistryStore)

  const enabledPlugins = plugins.filter(p => p.state === 'enabled')

  const tools = enabledPlugins.map(p => ({
    id: p.id,
    name: p.manifest.name,
    description: p.manifest.description ?? '',
    version: p.manifest.version,
    status: 'stable' as const,
    category: p.manifest.category,
    tags: p.manifest.tags,
    isInstalled: true,
  }))

  const getCategoryTools = (pluginIds: readonly string[]) =>
    tools.filter(t => pluginIds.includes(t.id))

  return (
    <div className="flex flex-col gap-6 p-6 lg:p-8">
      <header className="space-y-1">
        <h1 className="text-2xl font-semibold text-foreground">Dashboard</h1>
        <p className="text-sm text-muted">
          Your smart toolbox — quick access to all tools.
        </p>
      </header>

      <section className="space-y-2">
        <div className="flex items-center gap-3">
          <h2 className="text-xs font-semibold tracking-wide text-muted uppercase">
            Pinned
          </h2>
          <div className="h-px flex-1 bg-separator" />
        </div>
        <ToolGrid className="md:grid-cols-2 xl:grid-cols-4">
          {tools.slice(0, 4).map(tool => (
            <ToolGrid.Item key={tool.id}>
              <ToolCard onPress={() => navigate({ to: `/tools/${tool.id}` })}>
                <ToolCard.Header>
                  <ToolCard.Title>{tool.name}</ToolCard.Title>
                  <ToolCard.Description>
                    {tool.description}
                  </ToolCard.Description>
                </ToolCard.Header>
                <ToolCard.Meta status={tool.status} version={tool.version} />
              </ToolCard>
            </ToolGrid.Item>
          ))}
        </ToolGrid>
      </section>

      {pluginCategories.map(category => {
        const categoryTools = getCategoryTools([
          ...category.pluginIds,
        ] as string[])
        if (categoryTools.length === 0) return null

        const CategoryIcon = categoryIcons[category.id]

        return (
          <section key={category.id} className="space-y-2">
            <div className="flex items-center gap-3">
              {CategoryIcon ? (
                <CategoryIcon className="text-muted" size={14} />
              ) : null}
              <h2 className="text-xs font-semibold tracking-wide text-muted uppercase">
                {category.label}
              </h2>
              <Chip size="sm" variant="tertiary">
                {categoryTools.length}
              </Chip>
              <div className="h-px flex-1 bg-separator" />
            </div>
            <ToolGrid>
              {categoryTools.map(tool => (
                <ToolGrid.Item key={tool.id}>
                  <ToolCard
                    onPress={() => navigate({ to: `/tools/${tool.id}` })}
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
          </section>
        )
      })}
    </div>
  )
}
