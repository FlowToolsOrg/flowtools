import { ToolRelatedList } from '@flow-tool/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

describe('ToolRelatedList', () => {
  it('renders related tool entries', async () => {
    const { getByText } = await render(
      <ToolRelatedList
        items={[
          {
            id: 'tool-a',
            name: 'JWT Decoder',
            description: 'Decode JSON web tokens',
          },
        ]}
      />
    )

    await expect.element(getByText('JWT Decoder')).toBeInTheDocument()
    await expect
      .element(getByText('Decode JSON web tokens'))
      .toBeInTheDocument()
  })

  it('calls onSelect when open button clicked', async () => {
    const user = userEvent.setup()
    const onSelect = vi.fn()
    const { getByRole } = await render(
      <ToolRelatedList
        items={[
          {
            id: 'tool-b',
            name: 'Base64 Converter',
          },
        ]}
        onSelect={onSelect}
      />
    )

    await user.click(getByRole('button', { name: 'Open' }))

    expect(onSelect).toHaveBeenCalledWith('tool-b')
  })
})
