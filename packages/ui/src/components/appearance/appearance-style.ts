import type { CSSProperties } from 'react'

import {
  AppearanceError,
  createAppearanceResolver,
  type AppearanceColor,
  type ResolvedAppearance,
} from '@flowtools/sdk/extensions'

const fonts = {
  system: 'system-ui, sans-serif',
  serif: 'ui-serif, Georgia, serif',
  mono: 'ui-monospace, monospace',
} as const

const shadows = {
  none: 'none',
  soft: '0 2px 8px rgb(0 0 0 / 0.08)',
  raised: '0 8px 24px rgb(0 0 0 / 0.16)',
} as const

function color(value: AppearanceColor): string {
  return value.space === 'srgb'
    ? `rgb(${value.red} ${value.green} ${value.blue} / ${value.alpha ?? 1})`
    : `oklch(${value.lightness} ${value.chroma} ${value.hue} / ${value.alpha ?? 1})`
}

/** Revalidate even typed callers; never copy caller CSS or property names. */
export function appearanceStyle(value: ResolvedAppearance): {
  style: CSSProperties
  mode: ResolvedAppearance['mode']
  reducedMotion: boolean
} {
  if (
    (value.mode !== 'light' && value.mode !== 'dark') ||
    typeof value.reducedMotion !== 'boolean'
  )
    throw new AppearanceError('INVALID_REQUEST')
  const parsed = createAppearanceResolver({
    formatVersion: 1,
    title: 'Appearance scope',
    modes: { light: value.tokens, dark: value.tokens },
  })({
    contributions: [],
    selectedKey: null,
    mode: value.mode,
    systemMode: value.mode,
    systemReducedMotion: value.reducedMotion,
  })
  const { colors, radii, borders, typography, shadows: shadow } = parsed.tokens
  const variables: Record<`--${string}`, string> = {
    '--background': color(colors.canvas),
    '--foreground': color(colors.text),
    '--surface': color(colors.panel),
    '--surface-foreground': color(colors.panelText),
    '--surface-secondary': color(colors.panel),
    '--surface-secondary-foreground': color(colors.panelText),
    '--surface-tertiary': color(colors.panel),
    '--surface-tertiary-foreground': color(colors.panelText),
    '--overlay': color(colors.overlay),
    '--overlay-foreground': color(colors.overlayText),
    '--muted': color(colors.mutedText),
    '--default': color(colors.control),
    '--default-foreground': color(colors.controlText),
    '--segment': color(colors.control),
    '--segment-foreground': color(colors.controlText),
    '--field-background': color(colors.field),
    '--field-foreground': color(colors.fieldText),
    '--field-placeholder': color(colors.fieldPlaceholder),
    '--field-border': color(colors.border),
    '--accent': color(colors.accent),
    '--accent-foreground': color(colors.accentText),
    '--border': color(colors.border),
    '--separator': color(colors.separator),
    '--focus': color(colors.focus),
    '--link': color(colors.accent),
    '--success': color(colors.success),
    '--success-foreground': color(colors.successText),
    '--warning': color(colors.warning),
    '--warning-foreground': color(colors.warningText),
    '--danger': color(colors.danger),
    '--danger-foreground': color(colors.dangerText),
    '--radius': `${radii.panelRem}rem`,
    '--ft-control-radius': `${radii.controlRem}rem`,
    '--field-radius': `${radii.controlRem}rem`,
    '--radius-field': `${radii.controlRem}rem`,
    '--border-width': `${borders.panelPx}px`,
    '--field-border-width': `${borders.fieldPx}px`,
    '--border-width-field': `${borders.fieldPx}px`,
    '--surface-shadow': shadows[shadow.panel],
    '--overlay-shadow': shadows[shadow.overlay],
    '--field-shadow': shadows[shadow.field],
    '--font-sans': fonts[typography.fontFamily],
    '--ft-font-size': `${typography.fontSizePx}px`,
    '--ft-line-height': String(typography.lineHeight),
  }
  // HeroUI component CSS also reads Tailwind aliases directly. Rebind them at
  // this scope so they cannot inherit values computed against the root theme.
  for (const name of [
    'background',
    'foreground',
    'surface',
    'surface-foreground',
    'surface-secondary',
    'surface-secondary-foreground',
    'surface-tertiary',
    'surface-tertiary-foreground',
    'overlay',
    'overlay-foreground',
    'muted',
    'default',
    'default-foreground',
    'segment',
    'segment-foreground',
    'accent',
    'accent-foreground',
    'border',
    'separator',
    'focus',
    'link',
    'success',
    'success-foreground',
    'warning',
    'warning-foreground',
    'danger',
    'danger-foreground',
  ])
    variables[`--color-${name}`] = variables[`--${name}`]!
  variables['--color-field'] = variables['--field-background']!
  variables['--color-field-foreground'] = variables['--field-foreground']!
  variables['--color-field-placeholder'] = variables['--field-placeholder']!
  variables['--color-field-border'] = variables['--field-border']!
  variables['--color-default-hover'] =
    'color-mix(in oklab, var(--default) 96%, var(--default-foreground) 4%)'
  variables['--color-surface-hover'] =
    'color-mix(in oklab, var(--surface) 92%, var(--surface-foreground) 8%)'
  for (const name of ['accent', 'success', 'warning', 'danger']) {
    variables[`--color-${name}-hover`] =
      `color-mix(in oklab, var(--${name}) 90%, var(--${name}-foreground) 10%)`
    variables[`--color-${name}-soft`] =
      `color-mix(in oklab, var(--${name}) 15%, transparent)`
    variables[`--color-${name}-soft-hover`] =
      `color-mix(in oklab, var(--${name}) 20%, transparent)`
    variables[`--color-${name}-soft-foreground`] = variables[`--${name}`]!
  }
  variables['--color-field-hover'] =
    'color-mix(in oklab, var(--field-background) 90%, var(--field-foreground) 2%)'
  variables['--color-field-focus'] = variables['--field-background']!
  variables['--color-field-border-hover'] =
    'color-mix(in oklab, var(--field-border) 88%, var(--field-foreground) 10%)'
  variables['--color-field-border-focus'] =
    'color-mix(in oklab, var(--field-border) 74%, var(--field-foreground) 22%)'
  // Tailwind tokens otherwise inherit the root's computed rem-based sizes.
  for (const [name, scale] of Object.entries({
    xs: 0.75,
    sm: 0.875,
    base: 1,
    lg: 1.125,
    xl: 1.25,
    '2xl': 1.5,
    '3xl': 1.875,
    '4xl': 2.25,
  })) {
    variables[`--text-${name}`] = `${typography.fontSizePx * scale}px`
    variables[`--text-${name}--line-height`] = String(typography.lineHeight)
  }
  for (const [name, scale] of Object.entries({
    xs: 0.25,
    sm: 0.5,
    md: 0.75,
    lg: 1,
    xl: 1.5,
    '2xl': 2,
    '3xl': 3,
    '4xl': 4,
  }))
    variables[`--radius-${name}`] = `${radii.panelRem * scale}rem`

  return {
    style: { ...variables, colorScheme: parsed.mode },
    mode: parsed.mode,
    reducedMotion: parsed.reducedMotion,
  }
}
