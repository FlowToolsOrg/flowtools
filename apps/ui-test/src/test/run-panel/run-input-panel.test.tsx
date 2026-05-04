import { RunInputPanel } from '@flowtools/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('RunInputPanel', () => {
  it('renders input and run button', async () => {
    const { getByRole, getByText } = await render(
      <RunInputPanel label="Arguments" onChange={() => {}} value="" />
    )

    await expect.element(getByText('Arguments')).toBeInTheDocument()
    await expect
      .element(getByRole('button', { name: 'Run' }))
      .toBeInTheDocument()
  })

  it('calls onChange and onRun through interaction', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    const onRun = vi.fn()
    const { getByRole } = await render(
      <RunInputPanel onChange={onChange} onRun={onRun} value="" />
    )

    await user.type(getByRole('textbox', { name: 'Input' }), 'abc')
    await user.click(getByRole('button', { name: 'Run' }))

    expect(onChange).toHaveBeenCalled()
    expect(onRun).toHaveBeenCalledTimes(1)
  })
})
