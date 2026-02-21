import { ToolCard } from '@flow-tool/ui'
import { Button, Chip } from '@heroui/react'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('ToolCard', () => {
  it('renders title, description, meta and tags', async () => {
    const { getByRole, getByText } = await render(
      <ToolCard>
        <ToolCard.Header>
          <ToolCard.Title>Hash Generator</ToolCard.Title>
          <ToolCard.Description>
            Generate SHA and MD5 hashes quickly.
          </ToolCard.Description>
        </ToolCard.Header>
        <ToolCard.Meta status="beta" version="0.9.1" />
        <ToolCard.Tags>
          <Chip size="sm" variant="tertiary">
            security
          </Chip>
        </ToolCard.Tags>
        <ToolCard.Actions>
          <Button size="sm">Install</Button>
        </ToolCard.Actions>
      </ToolCard>
    )

    await expect
      .element(getByRole('heading', { level: 3, name: 'Hash Generator' }))
      .toBeInTheDocument()
    await expect
      .element(getByText('Generate SHA and MD5 hashes quickly.'))
      .toBeInTheDocument()
    await expect.element(getByText('Beta')).toBeInTheDocument()
    await expect.element(getByText('v0.9.1')).toBeInTheDocument()
  })

  it('calls onPress when root card clicked', async () => {
    const user = userEvent.setup()
    const onPress = vi.fn()
    const { getByRole } = await render(
      <ToolCard onPress={onPress}>
        <ToolCard.Title>Clipboard History</ToolCard.Title>
      </ToolCard>
    )

    await user.click(getByRole('button', { name: 'Clipboard History' }))

    expect(onPress).toHaveBeenCalledTimes(1)
  })
})
