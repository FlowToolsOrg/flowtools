import { ToolSummaryCard } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { Button } from '@heroui/react'

describe('ToolSummaryCard', () => {
  it('renders summary metadata', async () => {
    const { getByRole, getByText } = await render(
      <ToolSummaryCard
        category="Security"
        description="Generate secure hashes quickly"
        status="stable"
        title="Hash Generator"
        version="1.2.0"
      />
    )

    await expect
      .element(getByRole('heading', { level: 3, name: 'Hash Generator' }))
      .toBeInTheDocument()
    await expect.element(getByText('Security')).toBeInTheDocument()
    await expect.element(getByText('Stable')).toBeInTheDocument()
  })

  it('supports interactions through actions slot', async () => {
    const user = userEvent.setup()
    const onRun = vi.fn()
    const { getByRole } = await render(
      <ToolSummaryCard
        actions={<Button onPress={onRun}>Run</Button>}
        title="Clipboard"
      />
    )

    await user.click(getByRole('button', { name: 'Run' }))

    expect(onRun).toHaveBeenCalledTimes(1)
  })
})
