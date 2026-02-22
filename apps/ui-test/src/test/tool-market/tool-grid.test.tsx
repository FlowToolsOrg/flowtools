import { ToolCard, ToolGrid } from '@flow-tool/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { Button } from '@heroui/react'

describe('ToolGrid', () => {
  it('renders grid items', async () => {
    const { getByRole } = await render(
      <ToolGrid>
        <ToolGrid.Item>
          <ToolCard>
            <ToolCard.Title>Tool A</ToolCard.Title>
          </ToolCard>
        </ToolGrid.Item>
        <ToolGrid.Item>
          <ToolCard>
            <ToolCard.Title>Tool B</ToolCard.Title>
          </ToolCard>
        </ToolGrid.Item>
      </ToolGrid>
    )

    await expect
      .element(getByRole('heading', { level: 3, name: 'Tool A' }))
      .toBeInTheDocument()
    await expect
      .element(getByRole('heading', { level: 3, name: 'Tool B' }))
      .toBeInTheDocument()
  })

  it('keeps child interactions working inside items', async () => {
    const user = userEvent.setup()
    const onInstall = vi.fn()
    const { getByRole } = await render(
      <ToolGrid>
        <ToolGrid.Item>
          <ToolCard>
            <ToolCard.Title>Clipboard</ToolCard.Title>
            <ToolCard.Actions>
              <Button onPress={onInstall} size="sm">
                Install
              </Button>
            </ToolCard.Actions>
          </ToolCard>
        </ToolGrid.Item>
      </ToolGrid>
    )

    await user.click(getByRole('button', { name: 'Install' }))

    expect(onInstall).toHaveBeenCalledTimes(1)
  })
})
