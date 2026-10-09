import type { AppearanceDefaults, AppearanceTokens } from '../src/extensions'

export const rgb = (red: number, green = red, blue = red) => ({
  space: 'srgb' as const,
  red,
  green,
  blue,
})

function baseline(dark: boolean): AppearanceTokens {
  const paper = rgb(dark ? 24 : 248)
  const ink = rgb(dark ? 240 : 24)
  const accent = rgb(80, 60, 200)
  return {
    colors: {
      canvas: paper,
      text: ink,
      panel: paper,
      panelText: ink,
      overlay: paper,
      overlayText: ink,
      mutedText: rgb(128),
      control: paper,
      controlText: ink,
      field: paper,
      fieldText: ink,
      fieldPlaceholder: rgb(128),
      accent,
      accentText: rgb(255),
      border: rgb(160),
      separator: rgb(160),
      focus: accent,
      success: rgb(20, 160, 80),
      successText: ink,
      warning: rgb(240, 180, 20),
      warningText: ink,
      danger: rgb(200, 40, 40),
      dangerText: rgb(255),
    },
    radii: { panelRem: 0.625, controlRem: 0.625 },
    borders: { panelPx: 1, fieldPx: 1 },
    typography: { fontFamily: 'system', fontSizePx: 16, lineHeight: 1.5 },
    shadows: { panel: 'soft', overlay: 'raised', field: 'none' },
    motion: 'system',
  }
}

export const appearanceDefaults: AppearanceDefaults = {
  formatVersion: 1,
  title: 'Host default',
  modes: { light: baseline(false), dark: baseline(true) },
}

export const oceanTheme = {
  formatVersion: 1,
  title: 'Ocean',
  common: { radii: { panelRem: 1.25 }, typography: { fontFamily: 'serif' } },
  modes: {
    light: { colors: { accent: rgb(10, 100, 160) }, borders: { fieldPx: 2 } },
    dark: {
      colors: { accent: rgb(80, 180, 240) },
      radii: { controlRem: 0.75 },
    },
  },
}
