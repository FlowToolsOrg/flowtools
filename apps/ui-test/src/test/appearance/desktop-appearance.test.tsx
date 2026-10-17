import { useState } from 'react'

import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { DesktopAppearanceProvider } from '../../../../desktop/src/settings/appearance'
import { DesktopAppearanceSettings } from '../../../../desktop/src/settings/appearance-settings'
import '../../../../desktop/src/App.css'

const key = 'flowtools.desktop.appearance'

beforeEach(() => localStorage.removeItem(key))
afterEach(() => {
  vi.restoreAllMocks()
  localStorage.removeItem(key)
})

function Host() {
  const [settings, showSettings] = useState(true)
  return (
    <DesktopAppearanceProvider>
      <button onClick={() => showSettings(value => !value)}>切换页面</button>
      {settings ? <DesktopAppearanceSettings /> : <p>启动器</p>}
    </DesktopAppearanceProvider>
  )
}

it('retains the preference across routes and restores it before settings opens', async () => {
  const user = userEvent.setup()
  const screen = await render(<Host />)
  await user.click(screen.getByRole('button', { name: '深色', exact: true }))
  await expect
    .element(document.documentElement)
    .toHaveAttribute('data-theme', 'dark')
  expect(getComputedStyle(document.documentElement).colorScheme).toBe('dark')
  await user.click(screen.getByRole('button', { name: '切换页面' }))
  await expect.element(screen.getByText('启动器')).toBeVisible()
  await expect
    .element(document.documentElement)
    .toHaveAttribute('data-theme', 'dark')
  await user.click(screen.getByRole('button', { name: '切换页面' }))
  await expect
    .element(screen.getByRole('button', { name: '深色', exact: true }))
    .toHaveAttribute('aria-pressed', 'true')
  await screen.unmount()

  const restarted = await render(
    <DesktopAppearanceProvider>
      <p>新窗口</p>
    </DesktopAppearanceProvider>
  )
  await expect.element(restarted.getByText('新窗口')).toBeVisible()
  await expect
    .element(document.documentElement)
    .toHaveAttribute('data-theme', 'dark')
})

it('follows live system changes while explicit light wins over a dark system', async () => {
  const original = window.matchMedia.bind(window)
  const media = original('(prefers-color-scheme: dark)')
  const dark = vi.spyOn(media, 'matches', 'get').mockReturnValue(false)
  vi.spyOn(window, 'matchMedia').mockImplementation(query =>
    query === media.media ? media : original(query)
  )
  const screen = await render(<Host />)
  await expect
    .element(document.documentElement)
    .toHaveAttribute('data-theme', 'light')
  expect(getComputedStyle(document.documentElement).colorScheme).toBe('light')
  dark.mockReturnValue(true)
  media.dispatchEvent(new Event('change'))
  await expect
    .element(document.documentElement)
    .toHaveAttribute('data-theme', 'dark')
  await userEvent.click(
    screen.getByRole('button', { name: '浅色', exact: true })
  )
  media.dispatchEvent(new Event('change'))
  await expect
    .element(document.documentElement)
    .toHaveAttribute('data-theme', 'light')
  expect(
    getComputedStyle(document.documentElement)
      .getPropertyValue('--foreground')
      .trim()
  ).toBe('#333333')
  await userEvent.click(screen.getByRole('button', { name: '重置外观' }))
  await expect
    .element(document.documentElement)
    .toHaveAttribute('data-theme', 'dark')
  expect(localStorage.getItem(key)).toBeNull()
})

it.each([
  '{',
  JSON.stringify({ formatVersion: 2, mode: 'dark' }),
  JSON.stringify({ formatVersion: 1, mode: ['dark'] }),
  JSON.stringify({ formatVersion: 1, mode: 'dark', grants: true }),
  'x'.repeat(129),
])('preserves unreadable records until the user resets: %s', async raw => {
  localStorage.setItem(key, raw)
  const screen = await render(<Host />)
  await expect
    .element(screen.getByRole('status'))
    .toHaveTextContent('无法读取外观设置')
  await expect
    .element(screen.getByRole('button', { name: '系统', exact: true }))
    .toHaveAttribute('aria-pressed', 'true')
  expect(localStorage.getItem(key)).toBe(raw)
  localStorage.setItem('unrelated-host-data', 'keep')
  try {
    await userEvent.click(screen.getByRole('button', { name: '重置外观' }))
    expect(localStorage.getItem(key)).toBeNull()
    expect(localStorage.getItem('unrelated-host-data')).toBe('keep')
    await expect
      .element(screen.getByRole('status'))
      .toHaveTextContent('已恢复系统外观')
  } finally {
    localStorage.removeItem('unrelated-host-data')
  }
})

it('keeps controls usable and reports read/write/reset failures without false success', async () => {
  vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
    throw new Error('read blocked')
  })
  const write = vi
    .spyOn(Storage.prototype, 'setItem')
    .mockImplementation(() => {
      throw new Error('quota')
    })
  const remove = vi
    .spyOn(Storage.prototype, 'removeItem')
    .mockImplementation(() => {
      throw new Error('blocked')
    })
  const screen = await render(<Host />)
  await expect
    .element(screen.getByRole('status'))
    .toHaveTextContent('无法读取外观设置')
  await userEvent.click(
    screen.getByRole('button', { name: '深色', exact: true })
  )
  await expect
    .element(document.documentElement)
    .toHaveAttribute('data-theme', 'dark')
  await expect.element(screen.getByRole('status')).toHaveTextContent('无法保存')
  await userEvent.click(screen.getByRole('button', { name: '重置外观' }))
  await expect.element(screen.getByRole('status')).toHaveTextContent('无法清除')
  remove.mockRestore()
  write.mockRestore()
  await userEvent.click(
    screen.getByRole('button', { name: '深色', exact: true })
  )
  await expect
    .element(screen.getByRole('status'))
    .toHaveTextContent('外观已保存')
})
