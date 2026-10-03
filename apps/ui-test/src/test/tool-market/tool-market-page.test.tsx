import { ToolMarketPage } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('ToolMarketPage', () => {
  it('renders header and content slots', async () => {
    const { getByRole, getByText } = await render(
      <ToolMarketPage>
        <ToolMarketPage.Header>
          <h1>Tool Market</h1>
        </ToolMarketPage.Header>
        <ToolMarketPage.Content>
          <p>Market body</p>
        </ToolMarketPage.Content>
      </ToolMarketPage>
    )

    await expect
      .element(getByRole('heading', { level: 1, name: 'Tool Market' }))
      .toBeInTheDocument()
    await expect.element(getByText('Market body')).toBeInTheDocument()
  })

  it('supports interactions inside header area', async () => {
    const user = userEvent.setup()
    const onOpen = vi.fn()
    const { getByRole } = await render(
      <ToolMarketPage>
        <ToolMarketPage.Header>
          <button type="button" onClick={onOpen}>
            Create Tool
          </button>
        </ToolMarketPage.Header>
        <ToolMarketPage.Content />
      </ToolMarketPage>
    )

    await user.click(getByRole('button', { name: 'Create Tool' }))

    expect(onOpen).toHaveBeenCalledTimes(1)
  })
})
