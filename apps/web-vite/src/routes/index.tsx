import { extractMeta } from '@flow-tool/sdk'
import { ToolCard, ToolGrid } from '@flow-tool/ui'
import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { Chip } from '@heroui/react'

import runHello from '@plugins/plugin-example-run-hello'

import appPlugins, { pluginCategories } from '@/plugin/app'

const allPlugins = [...appPlugins, runHello]
const tools = extractMeta(allPlugins).map(meta => ({
  id: meta.id,
  name: meta.name,
  description: meta.description ?? '',
  version: meta.version,
  status: meta.status ?? 'stable',
  category: meta.category,
  tags: meta.tags,
  isInstalled: true,
}))

export const Route = createFileRoute('/')({
  component: Dashboard,
})

function Dashboard() {
  const navigate = useNavigate()

  const getCategoryTools = (pluginIds: readonly string[]) =>
    tools.filter(t => pluginIds.includes(t.id))

  return (
    <div className="flex flex-col gap-6 p-6 lg:p-8">
      <header className="space-y-1">
        <h1 className="text-2xl font-semibold text-[var(--foreground)]">
          Dashboard
        </h1>
        <p className="text-sm text-[var(--muted)]">
          Your smart toolbox — quick access to all tools.
        </p>
      </header>

      <section className="space-y-2">
        <div className="flex items-center gap-3">
          <h2 className="text-xs font-semibold tracking-wide text-[var(--muted)] uppercase">
            Pinned
          </h2>
          <div className="h-px flex-1 bg-[var(--separator)]" />
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

        return (
          <section key={category.id} className="space-y-2">
            <div className="flex items-center gap-3">
              <h2 className="text-xs font-semibold tracking-wide text-[var(--muted)] uppercase">
                {category.label}
              </h2>
              <Chip size="sm" variant="tertiary">
                {categoryTools.length}
              </Chip>
              <div className="h-px flex-1 bg-[var(--separator)]" />
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
