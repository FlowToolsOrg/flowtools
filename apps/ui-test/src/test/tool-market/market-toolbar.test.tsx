import { MarketToolbar } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('MarketToolbar', () => {
  it('renders search, filters and actions slots', async () => {
    const { getByRole, getByText } = await render(
      <MarketToolbar>
        <MarketToolbar.Search onChange={() => {}} value="" />
        <MarketToolbar.Filters>
          <span>Installed</span>
        </MarketToolbar.Filters>
        <MarketToolbar.Actions>
          <button type="button">Refresh</button>
        </MarketToolbar.Actions>
      </MarketToolbar>
    )

    await expect.element(getByRole('textbox')).toBeInTheDocument()
    await expect.element(getByText('Installed')).toBeInTheDocument()
    await expect
      .element(getByRole('button', { name: 'Refresh' }))
      .toBeInTheDocument()
  })

  it('calls onChange when typing in search input', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    const { getByRole } = await render(
      <MarketToolbar>
        <MarketToolbar.Search onChange={onChange} value="" />
      </MarketToolbar>
    )

    await user.type(getByRole('textbox'), 'hash')

    expect(onChange).toHaveBeenCalled()
  })
})
