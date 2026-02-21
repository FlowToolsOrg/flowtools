import { SettingsSelectField } from '@flow-tool/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('SettingsSelectField', () => {
  it('renders label and helper description', async () => {
    const { getByRole, getByText } = await render(
      <SettingsSelectField
        description="Select runtime mode"
        label="Mode"
        onChange={() => {}}
        options={[
          { key: 'default', label: 'Default' },
          { key: 'safe', label: 'Safe' },
        ]}
        value={'default'}
      />
    )

    await expect
      .element(getByRole('button', { name: /mode/i }))
      .toBeInTheDocument()
    await expect.element(getByText('Select runtime mode')).toBeInTheDocument()
  })

  it('calls onChange when option selected', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    const { getByRole } = await render(
      <SettingsSelectField
        label="Theme"
        onChange={onChange}
        options={[
          { key: 'light', label: 'Light' },
          { key: 'dark', label: 'Dark' },
        ]}
        value={'light'}
      />
    )

    await user.click(getByRole('button', { name: /theme/i }))
    await user.click(getByRole('option', { name: 'Dark' }))

    expect(onChange).toHaveBeenCalled()
  })
})
