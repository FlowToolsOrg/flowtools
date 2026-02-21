import { ToolPermissionList } from '@flow-tool/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('ToolPermissionList', () => {
  it('renders permission entries', async () => {
    const { getByText } = await render(
      <ToolPermissionList>
        <ToolPermissionList.Item
          description="Required for API calls"
          name="network"
          permissionId="network"
          required
        />
      </ToolPermissionList>
    )

    await expect.element(getByText('network')).toBeInTheDocument()
    await expect.element(getByText(/^Required$/)).toBeInTheDocument()
  })

  it('calls onPress when entry clicked', async () => {
    const user = userEvent.setup()
    const onPress = vi.fn()
    const { getByRole } = await render(
      <ToolPermissionList>
        <ToolPermissionList.Item
          name="storage"
          onPress={onPress}
          permissionId="storage"
        />
      </ToolPermissionList>
    )

    await user.click(getByRole('button', { name: 'storage Optional' }))

    expect(onPress).toHaveBeenCalledTimes(1)
  })
})
