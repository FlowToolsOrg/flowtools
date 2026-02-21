import { Button, Code } from '@flow-tool/ui'
import { vi, describe, it, expect } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('ui package smoke tests', () => {
  it('renders code component content', async () => {
    const { getByText } = await render(<Code className="x">bun run test</Code>)

    const code = getByText('bun run test')
    await expect.element(code).toBeInTheDocument()
    await expect.element(code).toHaveClass('x')
  })

  it('triggers alert in button click handler', async () => {
    const user = userEvent.setup()
    const alertSpy = vi.spyOn(window, 'alert').mockImplementation(() => {})
    const { getByRole } = await render(<Button appName="ui-test">Run</Button>)

    await user.click(getByRole('button', { name: 'Run' }))

    expect(alertSpy).toHaveBeenCalledWith('Hello from your ui-test app!')
    alertSpy.mockRestore()
  })
})
