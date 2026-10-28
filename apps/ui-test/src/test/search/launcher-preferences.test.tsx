import { expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import {
  launcherPreferencesKey,
  recordLauncherOpen,
  useLauncherPreferences,
} from '../../../../desktop/src/settings/launcher-preferences'

function Preferences() {
  const { preferences, preferencesError, save, clear } =
    useLauncherPreferences()
  return (
    <>
      <button onClick={() => save(recordLauncherOpen(preferences, 'p:run'))}>
        打开记录
      </button>
      <button onClick={clear}>清空</button>
      <p role="status">{preferencesError ? '存储失败' : '已保存'}</p>
    </>
  )
}
it('unreadable records survive automatic updates until explicit reset', async () => {
  localStorage.setItem(launcherPreferencesKey, 'old unreadable record')
  const view = await render(<Preferences />),
    user = userEvent.setup()
  await user.click(view.getByRole('button', { name: '打开记录' }))
  expect(localStorage.getItem(launcherPreferencesKey)).toBe(
    'old unreadable record'
  )
  await expect.element(view.getByRole('status')).toHaveTextContent('存储失败')
  await user.click(view.getByRole('button', { name: '清空', exact: true }))
  await expect.element(view.getByRole('status')).toHaveTextContent('已保存')
  expect(localStorage.getItem(launcherPreferencesKey)).toContain(
    '"recentIds":[]'
  )
})
it('write failure remains visible with usable controls', async () => {
  localStorage.removeItem(launcherPreferencesKey)
  const view = await render(<Preferences />),
    user = userEvent.setup()
  const write = vi
    .spyOn(Storage.prototype, 'setItem')
    .mockImplementation(() => {
      throw new DOMException('denied', 'QuotaExceededError')
    })
  try {
    await user.click(view.getByRole('button', { name: '打开记录' }))
    await expect.element(view.getByRole('status')).toHaveTextContent('存储失败')
    await expect
      .element(view.getByRole('button', { name: '清空', exact: true }))
      .toBeEnabled()
  } finally {
    write.mockRestore()
  }
})
