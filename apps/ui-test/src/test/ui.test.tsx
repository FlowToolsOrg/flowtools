import { Button, Code } from '@flow-tool/ui'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { vi } from 'vitest'

describe('ui package smoke tests', () => {
  it('renders code component content', () => {
    render(<Code className="x">bun run test</Code>)

    const code = screen.getByText('bun run test')
    expect(code).toBeInTheDocument()
    expect(code).toHaveClass('x')
  })

  it('triggers alert in button click handler', async () => {
    const user = userEvent.setup()
    const alertSpy = vi.spyOn(window, 'alert').mockImplementation(() => {})
    render(<Button appName="ui-test">Run</Button>)

    await user.click(screen.getByRole('button', { name: 'Run' }))

    expect(alertSpy).toHaveBeenCalledWith('Hello from your ui-test app!')
    alertSpy.mockRestore()
  })
})
