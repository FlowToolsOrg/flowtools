import { SettingsGroupCard } from '@flow-tool/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('SettingsGroupCard', () => {
  it('renders title and description slots', async () => {
    const { getByRole, getByText } = await render(
      <SettingsGroupCard>
        <SettingsGroupCard.Header>
          <SettingsGroupCard.Title>General</SettingsGroupCard.Title>
          <SettingsGroupCard.Description>
            Core preferences
          </SettingsGroupCard.Description>
        </SettingsGroupCard.Header>
        <SettingsGroupCard.Content>
          <p>Body</p>
        </SettingsGroupCard.Content>
      </SettingsGroupCard>
    )

    await expect
      .element(getByRole('heading', { level: 3, name: 'General' }))
      .toBeInTheDocument()
    await expect.element(getByText('Core preferences')).toBeInTheDocument()
  })

  it('supports interactions in content area', async () => {
    const user = userEvent.setup()
    const onApply = vi.fn()
    const { getByRole } = await render(
      <SettingsGroupCard>
        <SettingsGroupCard.Content>
          <button onClick={onApply} type="button">
            Apply
          </button>
        </SettingsGroupCard.Content>
      </SettingsGroupCard>
    )

    await user.click(getByRole('button', { name: 'Apply' }))

    expect(onApply).toHaveBeenCalledTimes(1)
  })
})
