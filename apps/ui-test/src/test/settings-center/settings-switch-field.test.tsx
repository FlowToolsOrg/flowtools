import { SettingsSwitchField } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('SettingsSwitchField', () => {
  it('renders label and description', async () => {
    const { getByText } = await render(
      <SettingsSwitchField
        description="Enable metrics"
        isSelected={false}
        label="Telemetry"
      />
    )

    await expect.element(getByText('Telemetry')).toBeInTheDocument()
    await expect.element(getByText('Enable metrics')).toBeInTheDocument()
  })

  it('calls onChange when toggled', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    const { getByText } = await render(
      <SettingsSwitchField
        isSelected={false}
        label="Auto update"
        onChange={onChange}
      />
    )

    await user.click(getByText('Auto update'))

    expect(onChange).toHaveBeenCalled()
  })
})
