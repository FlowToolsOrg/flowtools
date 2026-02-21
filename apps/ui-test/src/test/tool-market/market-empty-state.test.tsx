import { MarketEmptyState } from '@flow-tool/ui'
import { Button } from '@heroui/react'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('MarketEmptyState', () => {
  it('renders title and description', async () => {
    const { getByRole, getByText } = await render(
      <MarketEmptyState
        description="Try adjusting filters or search query."
        title="No tools found"
      />
    )

    await expect
      .element(getByRole('heading', { level: 3, name: 'No tools found' }))
      .toBeInTheDocument()
    await expect
      .element(getByText('Try adjusting filters or search query.'))
      .toBeInTheDocument()
  })

  it('allows interaction through action slot', async () => {
    const user = userEvent.setup()
    const onReset = vi.fn()
    const { getByRole } = await render(
      <MarketEmptyState
        action={<Button onPress={onReset}>Reset filters</Button>}
        title="No data"
      />
    )

    await user.click(getByRole('button', { name: 'Reset filters' }))

    expect(onReset).toHaveBeenCalledTimes(1)
  })
})
