import {
  PluginCompatibilityBadge,
  PluginMaturityBadge,
  ToolCard,
  ToolList,
  ToolSummaryCard,
} from '@flowtools/ui'
import { describe, expect, it } from 'vitest'
import { render } from 'vitest-browser-react'

describe('plugin maturity and independent compatibility evidence', () => {
  it.each(['prototype', 'experimental', 'beta', 'production'] as const)(
    'renders %s from the SDK vocabulary',
    async maturity => {
      const { getByText } = await render(
        <PluginMaturityBadge maturity={maturity} />
      )
      await expect
        .element(getByText(maturity[0]!.toUpperCase() + maturity.slice(1)))
        .toBeInTheDocument()
    }
  )
  it.each(['card', 'summary', 'list'] as const)(
    'defaults missing %s maturity to Prototype',
    async kind => {
      const { getByText } = await render(
        kind === 'card' ? (
          <ToolCard>
            <ToolCard.Meta />
          </ToolCard>
        ) : kind === 'summary' ? (
          <ToolSummaryCard title="Default fixture" />
        ) : (
          <ToolList items={[{ id: 'fixture', name: 'Default fixture' }]} />
        )
      )
      await expect.element(getByText('Prototype')).toBeInTheDocument()
    }
  )
  it.each(['indexed', 'entry-resolved'] as const)(
    'keeps %s evidence separate without upgrading maturity',
    async evidence => {
      const { getByText } = await render(
        <div>
          <PluginMaturityBadge />
          <PluginCompatibilityBadge evidence={evidence} />
        </div>
      )
      await expect.element(getByText('Prototype')).toBeInTheDocument()
      await expect.element(getByText(evidence)).toBeInTheDocument()
    }
  )
})
