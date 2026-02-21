import { describe, expect, it } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { App } from '../app'

describe('ui package dashboard smoke tests', () => {
  it('renders core wrapper components', async () => {
    const { getByRole } = await render(<App />)

    await expect
      .element(getByRole('heading', { name: 'Flow Tool Console' }))
      .toBeInTheDocument()
    await expect
      .element(getByRole('heading', { name: 'Available Tools' }))
      .toBeInTheDocument()
    await expect
      .element(getByRole('heading', { name: 'Settings' }))
      .toBeInTheDocument()
  })

  it('updates active tool and telemetry state through interactions', async () => {
    const user = userEvent.setup()
    const { getByRole, getByText, getByTestId } = await render(<App />)

    await user.click(getByRole('option', { name: 'Hash Generator' }))

    await expect
      .element(getByTestId('active-tool'))
      .toHaveTextContent('hash-generator')

    await user.click(getByText('Enable telemetry'))

    await expect
      .element(getByTestId('settings-summary'))
      .toHaveTextContent('Telemetry: on')
  })
})
