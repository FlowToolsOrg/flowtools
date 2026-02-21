import { SettingsCenterItem } from '@flow-tool/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('SettingsCenterItem', () => {
  it('renders label/description/control slots', async () => {
    const { getByText, getByRole } = await render(
      <SettingsCenterItem>
        <div>
          <SettingsCenterItem.Label>Telemetry</SettingsCenterItem.Label>
          <SettingsCenterItem.Description>
            Share anonymous diagnostics
          </SettingsCenterItem.Description>
        </div>
        <SettingsCenterItem.Control>
          <button type="button">Toggle</button>
        </SettingsCenterItem.Control>
      </SettingsCenterItem>
    )

    await expect.element(getByText('Telemetry')).toBeInTheDocument()
    await expect
      .element(getByRole('button', { name: 'Toggle' }))
      .toBeInTheDocument()
  })

  it('calls onPress when root item clicked', async () => {
    const user = userEvent.setup()
    const onPress = vi.fn()
    const { getByRole } = await render(
      <SettingsCenterItem onPress={onPress}>
        <SettingsCenterItem.Label>Clickable item</SettingsCenterItem.Label>
      </SettingsCenterItem>
    )

    await user.click(getByRole('button', { name: 'Clickable item' }))

    expect(onPress).toHaveBeenCalledTimes(1)
  })
})
