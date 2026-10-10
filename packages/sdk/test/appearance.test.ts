import { expect, test } from 'bun:test'

import {
  AppearanceError,
  createAppearanceResolver,
  ExtensionContributionRegistry,
  parseAppearanceOverrides,
  parseAppearanceTheme,
  projectPluginContributions,
  type AppearanceRequest,
} from '../src/extensions'
import { PluginLoader } from '../src/registry/plugin-loader'
import { PluginRegistry } from '../src/registry/plugin-registry'

import { appearanceDefaults, oceanTheme, rgb } from './appearance-fixtures'

const initial: AppearanceRequest = {
  contributions: [],
  selectedKey: null,
  mode: 'system',
  systemMode: 'light',
}
const document = (value: unknown, kind = 'theme') => ({
  formatVersion: 1,
  contributions: [{ id: 'ocean', kind, value }],
})

function setup(value: unknown = oceanTheme, kind = 'theme') {
  const registry = new ExtensionContributionRegistry()
  const owner = registry.createOwner('themes', '1.0.0')
  const dispose = registry.replace(owner, document(value, kind))
  const resolve = createAppearanceResolver(appearanceDefaults)
  const selected = (extra: Partial<AppearanceRequest> = {}) =>
    resolve({
      ...initial,
      contributions: registry.getSnapshot(),
      selectedKey: 'themes:ocean',
      ...extra,
    })
  return { registry, owner, dispose, resolve, selected }
}

test('theme and overrides are strict versioned data, cloned and deeply frozen', () => {
  const input = structuredClone(oceanTheme)
  const parsed = parseAppearanceTheme(input)
  input.common.radii.panelRem = 2
  expect(parsed.common?.radii?.panelRem).toBe(1.25)
  expect(Object.isFrozen(parsed.modes?.light?.colors?.accent)).toBe(true)
  expect(Reflect.set(parsed.common!.radii!, 'panelRem', 2)).toBe(false)
  expect(parseAppearanceOverrides({ formatVersion: 1 }).formatVersion).toBe(1)
  for (const value of [
    null,
    {},
    { ...oceanTheme, formatVersion: 2 },
    { ...oceanTheme, css: 'body{}' },
    { ...oceanTheme, title: '  ' },
    { ...oceanTheme, title: 'Bad\nTitle' },
    { ...oceanTheme, title: 'Bad\u0085Title' },
  ]) {
    expect(() => parseAppearanceTheme(value)).toThrow(AppearanceError)
  }
  expect(() => parseAppearanceOverrides({ ...oceanTheme })).toThrow(
    AppearanceError
  )
})

test('colors and token dimensions reject CSS, resource URLs and out-of-range values', () => {
  for (const common of [
    { colors: { accent: 'red' } },
    { colors: { accent: 'url(https://fixture.invalid/)' } },
    { colors: { accent: { ...rgb(0), red: 256 } } },
    { colors: { accent: { ...rgb(0), alpha: -0.1 } } },
    {
      colors: {
        accent: { space: 'oklch', lightness: 1, chroma: 0.5, hue: 400 },
      },
    },
    { radii: { panelRem: -1 } },
    { radii: { controlRem: 4 } },
    { borders: { fieldPx: 5 } },
    { typography: { fontSizePx: 11 } },
    { typography: { lineHeight: 3 } },
    { typography: { fontFamily: 'url(https://fixture.invalid/font)' } },
    { shadows: { overlay: '0 0 0 red' } },
    { layout: { sidebarWidth: 400 } },
  ])
    expect(() =>
      parseAppearanceTheme({ formatVersion: 1, title: 'Invalid', common })
    ).toThrow(AppearanceError)
  const parsed = parseAppearanceTheme({
    formatVersion: 1,
    title: 'OKLCH',
    common: {
      colors: {
        accent: {
          space: 'oklch',
          lightness: 0.7,
          chroma: 0.2,
          hue: 270,
          alpha: 0.5,
        },
      },
    },
  })
  expect(parsed.common?.colors?.accent?.space).toBe('oklch')
})

test('accessors, executable arrays and oversized documents are refused before reads/serialization', () => {
  let calls = 0
  const accessor = Object.defineProperty({}, 'formatVersion', {
    enumerable: true,
    get() {
      calls++
      return 1
    },
  })
  expect(() => parseAppearanceTheme(accessor)).toThrow(AppearanceError)
  class ExecutableArray extends Array {
    toJSON() {
      calls++
      return []
    }
  }
  expect(() =>
    parseAppearanceTheme({
      formatVersion: 1,
      title: 'Bad',
      common: new ExecutableArray(),
    })
  ).toThrow(AppearanceError)
  expect(calls).toBe(0)
  try {
    parseAppearanceTheme({
      formatVersion: 1,
      title: 'Bad',
      extra: 'x'.repeat(70_000),
    })
    throw new Error('Expected budget rejection')
  } catch (error) {
    expect(error).toBeInstanceOf(AppearanceError)
    expect((error as AppearanceError).code).toBe('BUDGET_EXCEEDED')
  }
})

test('Host supplies complete validated defaults; its input cannot change future resolution', () => {
  const defaults = structuredClone(appearanceDefaults)
  const resolve = createAppearanceResolver(defaults)
  expect(() =>
    createAppearanceResolver({ ...defaults, modes: { light: {} } })
  ).toThrow(AppearanceError)
  // Mutate a separate mutable view to exercise the caller-owned object boundary.
  const canvas = defaults.modes.light.colors.canvas as { red: number }
  canvas.red = 0
  expect(resolve(initial).tokens.colors.canvas).toEqual(rgb(248))
  expect(resolve(initial).fallback).toBe('default')
})

test('merges defaults, theme common/mode and user common/mode field by field', () => {
  const state = setup()
  const result = state.selected({
    overrides: {
      formatVersion: 1,
      common: { radii: { panelRem: 2 }, typography: { fontSizePx: 18 } },
      modes: {
        light: { radii: { panelRem: 0.5 }, shadows: { panel: 'none' } },
      },
    },
  })
  expect(result.effectiveKey).toBe('themes:ocean')
  expect(result.fallback).toBeNull()
  expect(result.tokens.radii).toEqual({ panelRem: 0.5, controlRem: 0.625 })
  expect(result.tokens.typography).toEqual({
    fontFamily: 'serif',
    fontSizePx: 18,
    lineHeight: 1.5,
  })
  expect(result.tokens.borders).toEqual({ panelPx: 1, fieldPx: 2 })
  expect(result.tokens.colors.accent).toEqual(rgb(10, 100, 160))
  expect(result.tokens.shadows).toEqual({
    panel: 'none',
    overlay: 'raised',
    field: 'none',
  })
  expect(result.tokens.colors.canvas).toEqual(rgb(248))
})

test('system and explicit modes select separate palettes without carrying old mode fields', () => {
  const state = setup()
  const dark = state.selected({ systemMode: 'dark' })
  expect(dark.mode).toBe('dark')
  expect(dark.tokens.colors.canvas).toEqual(rgb(24))
  expect(dark.tokens.colors.accent).toEqual(rgb(80, 180, 240))
  expect(dark.tokens.radii.controlRem).toBe(0.75)
  expect(dark.tokens.borders.fieldPx).toBe(1)
  const light = state.selected({ mode: 'light', systemMode: 'dark' })
  expect(light.mode).toBe('light')
  expect(light.tokens.radii.controlRem).toBe(0.625)
})

test('single-mode themes inherit the other Host palette', () => {
  const state = setup({
    formatVersion: 1,
    title: 'Light only',
    modes: { light: { radii: { panelRem: 2 } } },
  })
  const dark = state.selected({ mode: 'dark' })
  expect(dark.fallback).toBeNull()
  expect(dark.tokens).toEqual(appearanceDefaults.modes.dark)
})

test('registration does not select a theme and plugin namespaces remain independent', () => {
  const state = setup()
  const otherOwner = state.registry.createOwner('other', '1.0.0')
  state.registry.replace(
    otherOwner,
    document({
      formatVersion: 1,
      title: 'Other',
      common: { radii: { panelRem: 2.5 } },
    })
  )
  expect(
    state.resolve({ ...initial, contributions: state.registry.getSnapshot() })
      .effectiveKey
  ).toBeNull()
  expect(state.selected().tokens.radii.panelRem).toBe(1.25)
  expect(
    state.selected({ selectedKey: 'other:ocean' }).tokens.radii.panelRem
  ).toBe(2.5)
})

test('broken or wrong-kind themes recover to defaults while user overrides still apply', () => {
  for (const state of [
    setup({ formatVersion: 2, title: 'Bad' }),
    setup(oceanTheme, 'settings'),
  ]) {
    const result = state.selected({
      overrides: { formatVersion: 1, common: { radii: { panelRem: 1 } } },
    })
    expect(result.requestedKey).toBe('themes:ocean')
    expect(result.effectiveKey).toBeNull()
    expect(result.fallback).toBe('invalid-theme')
    expect(result.owner).toBeNull()
    expect(result.tokens.radii.panelRem).toBe(1)
    expect(result.tokens.colors.canvas).toEqual(rgb(248))
  }
})

test('invalid user overrides are reported and ignored atomically', () => {
  const state = setup()
  const result = state.selected({
    overrides: {
      formatVersion: 1,
      common: { radii: { panelRem: 1, controlRem: 99 } },
    },
  })
  expect(result.overrideError).toBe('INVALID_OVERRIDES')
  expect(result.tokens.radii).toEqual({ panelRem: 1.25, controlRem: 0.625 })
})

test('reduced motion from the operating system cannot be undone by a theme or user override', () => {
  const state = setup()
  expect(
    state.selected({
      systemReducedMotion: true,
      overrides: { formatVersion: 1, common: { motion: 'system' } },
    }).reducedMotion
  ).toBe(true)
  expect(
    state.selected({
      overrides: { formatVersion: 1, common: { motion: 'reduced' } },
    }).reducedMotion
  ).toBe(true)
  expect(state.selected().reducedMotion).toBe(false)
})

test('unchanged results are stable and frozen; preview resolution does not alter the registry', () => {
  const state = setup()
  const snapshot = state.registry.getSnapshot()
  const result = state.selected()
  expect(state.selected()).toBe(result)
  expect(Object.isFrozen(result.tokens.colors.accent)).toBe(true)
  expect(Object.isFrozen(result.owner)).toBe(true)
  state.selected({ selectedKey: 'missing:ocean' })
  expect(state.registry.getSnapshot()).toBe(snapshot)
  expect(state.selected().effectiveKey).toBe('themes:ocean')
})

test('actual plugin disable withdraws the theme, preserving selection and recovering on re-enable', async () => {
  const plugins = new PluginRegistry()
  const registry = new ExtensionContributionRegistry()
  const loader = new PluginLoader(plugins)
  plugins.register({
    id: 'themes',
    name: 'Theme fixture',
    type: 'tool',
    version: '1.0.0',
    loader: async () => ({
      default: {
        type: 'tool',
        meta: { id: 'themes', name: 'Theme fixture', version: '1.0.0' },
        run: () => undefined,
      },
    }),
  })
  const dispose = projectPluginContributions(plugins, registry, () =>
    document(oceanTheme)
  )
  const resolve = createAppearanceResolver(appearanceDefaults)
  const selected = () =>
    resolve({
      ...initial,
      contributions: registry.getSnapshot(),
      selectedKey: 'themes:ocean',
    })
  await loader.load('themes')
  await loader.enable('themes')
  const epoch = selected().owner!.generation
  await loader.disable('themes')
  expect(selected().fallback).toBe('missing-theme')
  expect(selected().requestedKey).toBe('themes:ocean')
  expect(selected().tokens).toEqual(appearanceDefaults.modes.light)
  await loader.enable('themes')
  expect(selected().effectiveKey).toBe('themes:ocean')
  expect(selected().owner!.generation).toBeGreaterThan(epoch)
  dispose()
  expect(selected().fallback).toBe('missing-theme')
})

test('corrupt Host mode and oversized selections fail with a stable error', () => {
  const state = setup()
  for (const request of [
    { ...initial, mode: 'unknown' },
    { ...initial, selectedKey: 'x'.repeat(258) },
  ]) {
    expect(() => state.resolve(request as AppearanceRequest)).toThrow(
      'INVALID_REQUEST'
    )
  }
})
