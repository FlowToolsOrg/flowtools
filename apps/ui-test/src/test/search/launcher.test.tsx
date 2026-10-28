import { expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { LauncherView } from '../../../../desktop/src/App'
import { LauncherInputProvider } from '../../../../desktop/src/runtime/launcher-input'
import { launcherPreferencesKey } from '../../../../desktop/src/settings/launcher-preferences'

const navigate = vi.hoisted(() => vi.fn())
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate,
  Outlet: () => null,
  useParams: () => ({}),
}))
vi.mock('../../../../desktop/src/utils/bindings', () => ({
  commands: { getPlugins: vi.fn(async () => ({ status: 'ok', data: [] })) },
}))
it('actual launcher searches pinyin, explains ranking and ignores IME Enter', async () => {
  localStorage.removeItem(launcherPreferencesKey)
  const view = await render(
    <LauncherInputProvider>
      <LauncherView />
    </LauncherInputProvider>
  )
  const input = view.getByRole('searchbox')
  await input.fill('shezhi')
  await expect.element(view.getByText(/匹配原因：.*拼音/)).toBeVisible()
  input.element().dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'Enter',
      isComposing: true,
      bubbles: true,
    })
  )
  expect(navigate).not.toHaveBeenCalled()
  await userEvent.setup().keyboard('{Enter}')
  expect(navigate).toHaveBeenCalledWith({ to: '/settings' })
  expect(localStorage.getItem(launcherPreferencesKey)).not.toContain('shezhi')
  expect(localStorage.getItem(launcherPreferencesKey)).toContain(
    'flowtools:settings'
  )
})
