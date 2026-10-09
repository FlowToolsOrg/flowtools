import {
  ExtensionContributionRegistry,
  type AppearanceDefaults,
  type AppearanceTokens,
} from '@flowtools/sdk/extensions'

const rgb = (red: number, green = red, blue = red) => ({
  space: 'srgb' as const,
  red,
  green,
  blue,
})

function baseline(dark: boolean): AppearanceTokens {
  const canvas = dark ? rgb(20, 24, 32) : rgb(246, 247, 249)
  const panel = dark ? rgb(30, 36, 46) : rgb(255)
  const text = dark ? rgb(237, 241, 247) : rgb(25, 33, 46)
  const muted = dark ? rgb(166, 179, 196) : rgb(92, 107, 128)
  const accent = dark ? rgb(146, 174, 255) : rgb(54, 82, 184)
  const border = dark ? rgb(70, 82, 100) : rgb(206, 214, 226)
  return {
    colors: {
      canvas,
      text,
      panel,
      panelText: text,
      overlay: panel,
      overlayText: text,
      mutedText: muted,
      control: canvas,
      controlText: text,
      field: panel,
      fieldText: text,
      fieldPlaceholder: muted,
      accent,
      accentText: dark ? rgb(20, 24, 32) : rgb(255),
      border,
      separator: border,
      focus: accent,
      success: rgb(18, 120, 83),
      successText: rgb(255),
      warning: rgb(245, 196, 83),
      warningText: rgb(40, 32, 15),
      danger: rgb(187, 47, 64),
      dangerText: rgb(255),
    },
    radii: { panelRem: 0.75, controlRem: 0.5 },
    borders: { panelPx: 1, fieldPx: 1 },
    typography: { fontFamily: 'system', fontSizePx: 16, lineHeight: 1.5 },
    shadows: { panel: 'soft', overlay: 'raised', field: 'none' },
    motion: 'system',
  }
}

export const previewDefaults: AppearanceDefaults = {
  formatVersion: 1,
  title: '默认外观',
  modes: { light: baseline(false), dark: baseline(true) },
}

// Trusted compiled fixtures; no install, external loader or new CLI inventory.
const contributions = {
  formatVersion: 1,
  contributions: [
    {
      id: 'bay',
      kind: 'theme',
      value: {
        formatVersion: 1,
        title: '海湾',
        common: {
          radii: { panelRem: 1.5, controlRem: 1 },
          borders: { panelPx: 2, fieldPx: 2 },
          typography: { fontFamily: 'serif', fontSizePx: 18, lineHeight: 1.6 },
          shadows: { panel: 'none' },
        },
        modes: {
          light: {
            colors: {
              canvas: rgb(234, 245, 245),
              panel: rgb(248, 253, 251),
              field: rgb(248, 253, 251),
              accent: rgb(18, 106, 114),
              focus: rgb(18, 106, 114),
              border: rgb(164, 197, 196),
            },
          },
          dark: {
            colors: {
              canvas: rgb(14, 36, 40),
              panel: rgb(22, 49, 52),
              field: rgb(22, 49, 52),
              accent: rgb(111, 216, 207),
              focus: rgb(111, 216, 207),
              border: rgb(58, 105, 108),
            },
          },
        },
      },
    },
    {
      id: 'ink',
      kind: 'theme',
      value: {
        formatVersion: 1,
        title: '墨线',
        common: {
          radii: { panelRem: 0.25, controlRem: 0.125 },
          typography: { fontFamily: 'mono', fontSizePx: 16 },
          colors: {
            accent: { space: 'oklch', lightness: 0.52, chroma: 0.16, hue: 295 },
            accentText: rgb(255),
          },
          motion: 'reduced',
        },
      },
    },
    {
      id: 'broken',
      kind: 'theme',
      value: {
        formatVersion: 1,
        title: '损坏示例',
        common: { css: 'invalid' },
      },
    },
  ],
}

export function createPreviewRegistry() {
  const registry = new ExtensionContributionRegistry()
  let owner = registry.createOwner('theme-lab', '1.0.0')
  registry.replace(owner, contributions)
  return {
    registry,
    disable: () => registry.revoke(owner),
    enable: () => {
      owner = registry.createOwner('theme-lab', '1.0.0')
      registry.replace(owner, contributions)
    },
  }
}
