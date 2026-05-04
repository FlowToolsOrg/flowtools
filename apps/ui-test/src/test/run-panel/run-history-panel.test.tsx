import { RunHistoryPanel } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('RunHistoryPanel', () => {
  it('renders history entries', async () => {
    const { getByText } = await render(
      <RunHistoryPanel
        entries={[
          {
            id: 'h1',
            title: 'Hash generation',
            startedAt: '2026-02-21 10:22',
            status: 'success',
          },
        ]}
      />
    )

    await expect.element(getByText('Hash generation')).toBeInTheDocument()
    await expect.element(getByText('success')).toBeInTheDocument()
  })

  it('calls onSelect when open clicked', async () => {
    const user = userEvent.setup()
    const onSelect = vi.fn()
    const { getByRole } = await render(
      <RunHistoryPanel
        entries={[
          {
            id: 'h2',
            title: 'Clipboard sync',
            startedAt: '2026-02-21 10:30',
            status: 'running',
          },
        ]}
        onSelect={onSelect}
      />
    )

    await user.click(getByRole('button', { name: 'Open' }))

    expect(onSelect).toHaveBeenCalledWith('h2')
  })
})
