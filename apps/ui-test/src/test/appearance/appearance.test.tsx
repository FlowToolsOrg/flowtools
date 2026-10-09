import { createAppearanceResolver } from '@flowtools/sdk/extensions'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { appearanceStyle } from '../../../../../packages/ui/src/components/appearance/appearance-style'
import { AppearanceValidation } from '../../appearance/appearance-validation'
import { previewDefaults } from '../../appearance/fixtures'

function element(selector: string): HTMLElement {
  const found = document.querySelector(selector)
  if (!(found instanceof HTMLElement)) throw new Error(`Missing ${selector}`)
  return found
}

const styles = (selector: string) => getComputedStyle(element(selector))
const card = '.flowtools-appearance .card'
const button = '.flowtools-appearance .button--primary'
const field = '.flowtools-appearance .input'

describe('Host-owned appearance preview', () => {
  it('reacts to media changes and keeps system reduced motion above theme choice', async () => {
    const original = window.matchMedia.bind(window)
    const colorMedia = original('(prefers-color-scheme: dark)')
    const motionMedia = original('(prefers-reduced-motion: reduce)')
    const dark = vi.spyOn(colorMedia, 'matches', 'get').mockReturnValue(false)
    const reduce = vi
      .spyOn(motionMedia, 'matches', 'get')
      .mockReturnValue(false)
    vi.spyOn(window, 'matchMedia').mockImplementation(query => {
      if (query === colorMedia.media) return colorMedia
      if (query === motionMedia.media) return motionMedia
      return original(query)
    })
    try {
      const screen = await render(<AppearanceValidation />)
      const scope = screen.getByRole('region', { name: '主题预览' })
      await expect
        .element(scope)
        .toHaveAttribute('data-appearance-mode', 'light')
      dark.mockReturnValue(true)
      colorMedia.dispatchEvent(new Event('change'))
      reduce.mockReturnValue(true)
      motionMedia.dispatchEvent(new Event('change'))
      await expect
        .element(scope)
        .toHaveAttribute('data-appearance-mode', 'dark')
      await expect.element(scope).toHaveAttribute('data-reduced-motion', 'true')
      const user = userEvent.setup()
      await user.selectOptions(screen.getByLabelText('显示模式'), 'light')
      await expect
        .element(scope)
        .toHaveAttribute('data-appearance-mode', 'light')
      await expect.element(scope).toHaveAttribute('data-reduced-motion', 'true')
    } finally {
      vi.restoreAllMocks()
    }
  })

  it('themes real HeroUI and shared wrappers, keeping recovery controls isolated', async () => {
    const user = userEvent.setup()
    const screen = await render(<AppearanceValidation />)
    const outside = styles('.appearance-lab__apply').borderRadius
    await user.selectOptions(screen.getByLabelText('显示模式'), 'light')
    await user.selectOptions(
      screen.getByLabelText('主题', { exact: true }),
      'theme-lab:bay'
    )

    expect(styles('.flowtools-appearance').backgroundColor).toBe(
      'rgb(234, 245, 245)'
    )
    expect(styles(card).borderRadius).toBe('24px')
    expect(styles(card).borderWidth).toBe('2px')
    expect(styles(card).boxShadow).toBe('none')
    expect(styles(button).borderRadius).toBe('16px')
    await expect
      .poll(() => styles(button).backgroundColor)
      .toBe('rgb(18, 106, 114)')
    expect(styles(field).borderRadius).toBe('16px')
    expect(styles(field).borderWidth).toBe('2px')
    expect(styles('.flowtools-appearance .card__title').fontSize).toBe(
      '15.75px'
    )
    expect(styles('.flowtools-appearance').fontFamily).toContain('ui-serif')
    expect(
      styles('[data-testid="appearance-settings-field"]').borderRadius
    ).toBe('24px')
    expect(
      styles('[data-testid="appearance-settings-field"]').paddingLeft
    ).toBe('16px')
    expect(styles('.appearance-lab__apply').borderRadius).toBe(outside)

    await user.selectOptions(screen.getByLabelText('显示模式'), 'dark')
    expect(styles('.flowtools-appearance').colorScheme).toBe('dark')
    expect(styles('.flowtools-appearance').backgroundColor).toBe(
      'rgb(14, 36, 40)'
    )
    await expect
      .poll(() => styles(button).backgroundColor)
      .toBe('rgb(111, 216, 207)')
    expect(styles(card).borderRadius).toBe('24px')
  })

  it('applies, cancels and resets previews; keyboard radius overrides win', async () => {
    const user = userEvent.setup()
    const screen = await render(<AppearanceValidation />)
    await user.selectOptions(
      screen.getByLabelText('主题', { exact: true }),
      'theme-lab:bay'
    )
    await user.click(screen.getByRole('button', { name: '应用主题' }))
    await expect.element(screen.getByText('已应用：海湾')).toBeVisible()
    await user.selectOptions(
      screen.getByLabelText('主题', { exact: true }),
      'theme-lab:ink'
    )
    expect(styles(card).borderRadius).toBe('4px')
    expect(styles(button).transitionDuration).toBe('0s')
    await user.click(screen.getByRole('button', { name: '取消预览' }))
    expect(styles(card).borderRadius).toBe('24px')
    await user.click(screen.getByLabelText('自定义圆角'))
    element('input[type="range"]').focus()
    await user.keyboard('{Home}')
    expect(styles(card).borderRadius).toBe('0px')
    expect(styles(button).borderRadius).toBe('0px')
    await user.click(screen.getByRole('button', { name: '取消预览' }))
    expect(styles(card).borderRadius).toBe('24px')
    await user.click(screen.getByRole('button', { name: '恢复默认' }))
    expect(styles(card).borderRadius).toBe('12px')
    await expect.element(screen.getByText('已应用：默认外观')).toBeVisible()
  })

  it('withdraws live contributions, falls back and recovers the retained selection', async () => {
    const user = userEvent.setup()
    const screen = await render(<AppearanceValidation />)
    await user.selectOptions(
      screen.getByLabelText('主题', { exact: true }),
      'theme-lab:bay'
    )
    await user.click(screen.getByRole('button', { name: '应用主题' }))
    await user.click(screen.getByRole('button', { name: '停用主题包' }))
    expect(styles(card).borderRadius).toBe('12px')
    await expect
      .element(
        screen.getByText('主题包已停用，正在使用默认外观。原选择已保留。')
      )
      .toBeVisible()
    await expect.element(screen.getByText('已应用：海湾')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '启用主题包' }))
    expect(styles(card).borderRadius).toBe('24px')
    await user.selectOptions(
      screen.getByLabelText('主题', { exact: true }),
      'theme-lab:broken'
    )
    expect(styles(card).borderRadius).toBe('12px')
    await expect
      .element(screen.getByText('主题数据无效，已回到默认外观。'))
      .toBeVisible()
    await user.click(screen.getByRole('button', { name: '恢复默认' }))
    await expect.element(screen.getByText('已恢复默认外观。')).toBeVisible()
  })

  it('keeps shared inputs interactive through a theme change', async () => {
    const user = userEvent.setup()
    const screen = await render(<AppearanceValidation />)
    await user.clear(screen.getByRole('textbox', { name: '工作空间名称' }))
    await user.type(
      screen.getByRole('textbox', { name: '工作空间名称' }),
      '设计工作台'
    )
    await user.selectOptions(
      screen.getByLabelText('主题', { exact: true }),
      'theme-lab:ink'
    )
    await expect
      .element(screen.getByRole('heading', { name: '设计工作台' }))
      .toBeVisible()
    await expect
      .element(screen.getByRole('textbox', { name: '工作空间名称' }))
      .toHaveValue('设计工作台')
    await expect.poll(() => styles(button).backgroundColor).toMatch(/^oklch\(/)
    await expect.element(screen.getByText('已减少动态效果')).toBeVisible()
  })

  it('revalidates forged typed CSS and invalid numeric values before mapping', () => {
    const value = createAppearanceResolver(previewDefaults)({
      contributions: [],
      selectedKey: null,
      mode: 'light',
      systemMode: 'light',
    })
    expect(() =>
      appearanceStyle({
        ...value,
        tokens: {
          ...value.tokens,
          colors: {
            ...value.tokens.colors,
            accent: 'url(https://invalid.example)' as never,
          },
        },
      })
    ).toThrow('INVALID_DEFAULTS')
    expect(() =>
      appearanceStyle({
        ...value,
        tokens: {
          ...value.tokens,
          radii: { panelRem: Infinity, controlRem: 1 },
        },
      })
    ).toThrow('INVALID_DEFAULTS')
    expect(() =>
      appearanceStyle({ ...value, mode: 'external' as never })
    ).toThrow('INVALID_REQUEST')
  })
})
