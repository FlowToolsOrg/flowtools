import { CommandPalette } from '@flowtools/ui'
import { expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

const items = [
  { id: 'reminder', title: '本地提醒', source: '办公', aliases: ['alarm'] },
  { id: 'json', title: 'JSON Formatter', source: '工具' },
]
it('pinyin and aliases search the real shared index and disabled items disappear', async () => {
  const view = await render(
    <CommandPalette open items={items} onSelect={() => {}} onClose={() => {}} />
  )
  const input = view.getByRole('textbox')
  await input.fill('bdtx')
  await expect
    .element(view.getByRole('button', { name: /本地提醒/ }))
    .toBeVisible()
  await expect
    .element(view.getByRole('button', { name: /JSON Formatter/ }))
    .not.toBeInTheDocument()
  await input.fill('alarm')
  await expect
    .element(view.getByRole('button', { name: /本地提醒/ }))
    .toBeVisible()
  await view.rerender(
    <CommandPalette
      open
      items={[items[1]!]}
      onSelect={() => {}}
      onClose={() => {}}
    />
  )
  await expect
    .element(view.getByRole('button', { name: /本地提醒/ }))
    .not.toBeInTheDocument()
})
it('invalid regex is visible and IME Enter does not select a command', async () => {
  const select = vi.fn()
  const view = await render(
    <CommandPalette open items={items} onSelect={select} onClose={() => {}} />
  )
  const input = view.getByRole('textbox')
  input.element().dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'Enter',
      isComposing: true,
      bubbles: true,
    })
  )
  expect(select).not.toHaveBeenCalled()
  const user = userEvent.setup()
  await user.selectOptions(
    view.getByRole('combobox', { name: '搜索方式' }),
    'regex'
  )
  await input.fill('[')
  await expect.element(view.getByRole('status')).toHaveTextContent('无效')
  await input.fill('^json')
  await user.keyboard('{Enter}')
  expect(select).toHaveBeenCalledWith(items[1])
})
