import { ToolVersionTimeline } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('ToolVersionTimeline', () => {
  it('renders version records', async () => {
    const { getByText } = await render(
      <ToolVersionTimeline
        records={[
          {
            id: 'v1',
            version: '1.0.0',
            date: '2026-02-01',
            notes: 'Initial release',
          },
        ]}
      />
    )

    await expect.element(getByText('1.0.0')).toBeInTheDocument()
    await expect.element(getByText('2026-02-01')).toBeInTheDocument()
  })

  it('calls onSelect when version clicked', async () => {
    const user = userEvent.setup()
    const onSelect = vi.fn()
    const { getByRole } = await render(
      <ToolVersionTimeline
        onSelect={onSelect}
        records={[
          {
            id: 'v2',
            version: '2.0.0',
            date: '2026-02-20',
          },
        ]}
      />
    )

    await user.click(getByRole('button', { name: '2.0.0 2026-02-20' }))

    expect(onSelect).toHaveBeenCalledWith('v2')
  })
})
