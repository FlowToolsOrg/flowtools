import { SettingsInputField } from '@flow-tool/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('SettingsInputField', () => {
  it('renders label and description', async () => {
    const { getByText } = await render(
      <SettingsInputField
        description="Used for command prefixes"
        label="Workspace name"
        onChange={() => {}}
        value=""
      />
    )

    await expect.element(getByText('Workspace name')).toBeInTheDocument()
    await expect
      .element(getByText('Used for command prefixes'))
      .toBeInTheDocument()
  })

  it('calls onChange when user types', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    const { getByRole } = await render(
      <SettingsInputField label="Workspace" onChange={onChange} value="" />
    )

    await user.type(getByRole('textbox', { name: 'Workspace' }), 'abc')

    expect(onChange).toHaveBeenCalled()
    expect(onChange).toHaveBeenLastCalledWith('c')
  })
})
