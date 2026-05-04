import { ToolDetailPage } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { Button } from '@heroui/react'

describe('ToolDetailPage', () => {
  it('renders header and content slots', async () => {
    const { getByRole, getByText } = await render(
      <ToolDetailPage>
        <ToolDetailPage.Header>
          <h1>Tool Detail</h1>
        </ToolDetailPage.Header>
        <ToolDetailPage.Content>
          <div>Main</div>
          <aside>Side</aside>
        </ToolDetailPage.Content>
      </ToolDetailPage>
    )

    await expect
      .element(getByRole('heading', { level: 1, name: 'Tool Detail' }))
      .toBeInTheDocument()
    await expect.element(getByText('Main')).toBeInTheDocument()
  })

  it('supports interactions in header', async () => {
    const user = userEvent.setup()
    const onInstall = vi.fn()
    const { getByRole } = await render(
      <ToolDetailPage>
        <ToolDetailPage.Header>
          <Button onPress={onInstall}>Install</Button>
        </ToolDetailPage.Header>
        <ToolDetailPage.Content />
      </ToolDetailPage>
    )

    await user.click(getByRole('button', { name: 'Install' }))

    expect(onInstall).toHaveBeenCalledTimes(1)
  })
})
