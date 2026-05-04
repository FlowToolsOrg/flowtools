import { RunLogList } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('RunLogList', () => {
  it('renders log messages', async () => {
    const { getByText } = await render(
      <RunLogList
        entries={[
          {
            id: 'l1',
            level: 'info',
            message: 'Task started',
            timestamp: '10:20:00',
          },
        ]}
      />
    )

    await expect.element(getByText('Task started')).toBeInTheDocument()
    await expect.element(getByText('info')).toBeInTheDocument()
  })

  it('calls onSelect when entry clicked', async () => {
    const user = userEvent.setup()
    const onSelect = vi.fn()
    const { getByRole } = await render(
      <RunLogList
        entries={[
          {
            id: 'l2',
            level: 'warn',
            message: 'Slow response',
            timestamp: '10:21:00',
          },
        ]}
        onSelect={onSelect}
      />
    )

    await user.click(
      getByRole('button', { name: 'Slow response 10:21:00 warn' })
    )

    expect(onSelect).toHaveBeenCalledWith('l2')
  })
})
