import { RunPanel } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { Button } from '@heroui/react'

describe('RunPanel', () => {
  it('renders header/content/footer slots', async () => {
    const { getByRole, getByText } = await render(
      <RunPanel>
        <RunPanel.Header>
          <h2>Run Tool</h2>
        </RunPanel.Header>
        <RunPanel.Content>
          <p>Body</p>
        </RunPanel.Content>
        <RunPanel.Footer>
          <span>Footer</span>
        </RunPanel.Footer>
      </RunPanel>
    )

    await expect
      .element(getByRole('heading', { level: 2, name: 'Run Tool' }))
      .toBeInTheDocument()
    await expect.element(getByText('Body')).toBeInTheDocument()
  })

  it('supports interactions in footer', async () => {
    const user = userEvent.setup()
    const onCancel = vi.fn()
    const { getByRole } = await render(
      <RunPanel>
        <RunPanel.Footer>
          <Button onPress={onCancel}>Cancel</Button>
        </RunPanel.Footer>
      </RunPanel>
    )

    await user.click(getByRole('button', { name: 'Cancel' }))

    expect(onCancel).toHaveBeenCalledTimes(1)
  })
})
