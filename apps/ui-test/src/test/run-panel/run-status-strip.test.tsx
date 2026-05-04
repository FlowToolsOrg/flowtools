import { RunStatusStrip } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('RunStatusStrip', () => {
  it('renders status and message', async () => {
    const { getByText } = await render(
      <RunStatusStrip
        duration="34ms"
        message="Run completed"
        status="success"
      />
    )

    await expect.element(getByText('success')).toBeInTheDocument()
    await expect.element(getByText('Run completed')).toBeInTheDocument()
  })

  it('calls onReset when reset button clicked', async () => {
    const user = userEvent.setup()
    const onReset = vi.fn()
    const { getByRole } = await render(
      <RunStatusStrip onReset={onReset} status="error" />
    )

    await user.click(getByRole('button', { name: 'Reset' }))

    expect(onReset).toHaveBeenCalledTimes(1)
  })
})
