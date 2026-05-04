import { RunResultPanel } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('RunResultPanel', () => {
  it('renders result title and summary', async () => {
    const { getByText } = await render(
      <RunResultPanel
        result={{
          title: 'Execution Result',
          summary: 'Completed successfully',
          raw: '{"ok":true}',
        }}
      />
    )

    await expect.element(getByText('Execution Result')).toBeInTheDocument()
    await expect
      .element(getByText('Completed successfully'))
      .toBeInTheDocument()
  })

  it('calls onCopy when copy button clicked', async () => {
    const user = userEvent.setup()
    const onCopy = vi.fn()
    const { getByRole } = await render(
      <RunResultPanel onCopy={onCopy} result={{ title: 'Result' }} />
    )

    await user.click(getByRole('button', { name: 'Copy' }))

    expect(onCopy).toHaveBeenCalledTimes(1)
  })
})
