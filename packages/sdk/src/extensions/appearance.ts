import type {
  ExtensionContributionOwner,
  ExtensionContributionSnapshot,
} from './types'

import { z } from 'zod'

import { isJsonValue } from '../manifest/json-schema'

import { boundedJsonBytes } from './json-budget'
import { extensionContributionLimits, parseContributionOwner } from './schema'

const unit = z.number().min(0).max(1)
const channel = z.number().int().min(0).max(255)
const color = z.discriminatedUnion('space', [
  z.strictObject({
    space: z.literal('srgb'),
    red: channel,
    green: channel,
    blue: channel,
    alpha: unit.optional(),
  }),
  z.strictObject({
    space: z.literal('oklch'),
    lightness: unit,
    chroma: z.number().min(0).max(0.4),
    hue: z.number().min(0).max(360),
    alpha: unit.optional(),
  }),
])

// FlowTools semantic roles, independent of any component library's CSS names.
const colors = z.strictObject({
  canvas: color,
  text: color,
  panel: color,
  panelText: color,
  overlay: color,
  overlayText: color,
  mutedText: color,
  control: color,
  controlText: color,
  field: color,
  fieldText: color,
  fieldPlaceholder: color,
  accent: color,
  accentText: color,
  border: color,
  separator: color,
  focus: color,
  success: color,
  successText: color,
  warning: color,
  warningText: color,
  danger: color,
  dangerText: color,
})
const radius = z.number().min(0).max(3)
const radii = z.strictObject({ panelRem: radius, controlRem: radius })
const border = z.number().min(0).max(4)
const borders = z.strictObject({ panelPx: border, fieldPx: border })
const typography = z.strictObject({
  fontFamily: z.enum(['system', 'serif', 'mono']),
  fontSizePx: z.number().min(12).max(24),
  lineHeight: z.number().min(1).max(2),
})
const shadow = z.enum(['none', 'soft', 'raised'])
const shadows = z.strictObject({
  panel: shadow,
  overlay: shadow,
  field: shadow,
})
const motion = z.enum(['system', 'reduced'])
const tokens = z.strictObject({
  colors,
  radii,
  borders,
  typography,
  shadows,
  motion,
})
const patch = z.strictObject({
  colors: colors.partial().optional(),
  radii: radii.partial().optional(),
  borders: borders.partial().optional(),
  typography: typography.partial().optional(),
  shadows: shadows.partial().optional(),
  motion: motion.optional(),
})
const modes = z.strictObject({
  light: patch.optional(),
  dark: patch.optional(),
})
const title = z
  .string()
  .trim()
  .min(1)
  .max(80)
  .refine(value => !/\p{Cc}/u.test(value))
const themeSchema = z.strictObject({
  formatVersion: z.literal(1),
  title,
  common: patch.optional(),
  modes: modes.optional(),
})
const overridesSchema = z.strictObject({
  formatVersion: z.literal(1),
  common: patch.optional(),
  modes: modes.optional(),
})
const defaultsSchema = z.strictObject({
  formatVersion: z.literal(1),
  title,
  modes: z.strictObject({ light: tokens, dark: tokens }),
})

type Immutable<T> = T extends object
  ? { readonly [Key in keyof T]: Immutable<T[Key]> }
  : T

export type AppearanceMode = 'light' | 'dark'
export type AppearanceColor =
  | {
      readonly space: 'srgb'
      readonly red: number
      readonly green: number
      readonly blue: number
      readonly alpha?: number
    }
  | {
      readonly space: 'oklch'
      readonly lightness: number
      readonly chroma: number
      readonly hue: number
      readonly alpha?: number
    }
export type AppearanceColorRole =
  | 'canvas'
  | 'text'
  | 'panel'
  | 'panelText'
  | 'overlay'
  | 'overlayText'
  | 'mutedText'
  | 'control'
  | 'controlText'
  | 'field'
  | 'fieldText'
  | 'fieldPlaceholder'
  | 'accent'
  | 'accentText'
  | 'border'
  | 'separator'
  | 'focus'
  | 'success'
  | 'successText'
  | 'warning'
  | 'warningText'
  | 'danger'
  | 'dangerText'
export type AppearanceShadow = 'none' | 'soft' | 'raised'
export interface AppearanceTokens {
  readonly colors: Readonly<Record<AppearanceColorRole, AppearanceColor>>
  readonly radii: { readonly panelRem: number; readonly controlRem: number }
  readonly borders: { readonly panelPx: number; readonly fieldPx: number }
  readonly typography: {
    readonly fontFamily: 'system' | 'serif' | 'mono'
    readonly fontSizePx: number
    readonly lineHeight: number
  }
  readonly shadows: {
    readonly panel: AppearanceShadow
    readonly overlay: AppearanceShadow
    readonly field: AppearanceShadow
  }
  readonly motion: 'system' | 'reduced'
}
export type AppearanceTokenPatch = {
  readonly [Key in keyof AppearanceTokens]?: AppearanceTokens[Key] extends object
    ? Partial<AppearanceTokens[Key]>
    : AppearanceTokens[Key]
}
export interface AppearanceOverrides {
  readonly formatVersion: 1
  readonly common?: AppearanceTokenPatch
  readonly modes?: Readonly<
    Partial<Record<AppearanceMode, AppearanceTokenPatch>>
  >
}
export interface AppearanceTheme extends AppearanceOverrides {
  readonly title: string
}
export interface AppearanceDefaults {
  readonly formatVersion: 1
  readonly title: string
  readonly modes: Readonly<Record<AppearanceMode, AppearanceTokens>>
}
export type AppearanceErrorCode =
  | 'INVALID_THEME'
  | 'INVALID_OVERRIDES'
  | 'INVALID_DEFAULTS'
  | 'INVALID_REQUEST'
  | 'BUDGET_EXCEEDED'

export class AppearanceError extends Error {
  constructor(readonly code: AppearanceErrorCode) {
    super(code)
    this.name = 'AppearanceError'
  }
}

function freeze<T>(value: T): Immutable<T> {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value)) freeze(child)
    Object.freeze(value)
  }
  return value as Immutable<T>
}

function parse<T>(
  schema: z.ZodType<T>,
  value: unknown,
  code: AppearanceErrorCode
): Immutable<T> {
  if (!isJsonValue(value)) throw new AppearanceError(code)
  try {
    boundedJsonBytes(value, extensionContributionLimits.maxValueBytes)
  } catch {
    throw new AppearanceError('BUDGET_EXCEEDED')
  }
  const result = schema.safeParse(value)
  if (!result.success) throw new AppearanceError(code)
  // The guard and schema have excluded accessors, executable and lossy data.
  return freeze(JSON.parse(JSON.stringify(result.data)) as T)
}

export function parseAppearanceTheme(value: unknown): AppearanceTheme {
  return parse(themeSchema, value, 'INVALID_THEME')
}

export function parseAppearanceOverrides(value: unknown): AppearanceOverrides {
  return parse(overridesSchema, value, 'INVALID_OVERRIDES')
}

export interface AppearanceRequest {
  /** A Host-owned E01 snapshot, never a caller-supplied package identity. */
  contributions: ExtensionContributionSnapshot
  selectedKey: string | null
  mode: AppearanceMode | 'system'
  systemMode: AppearanceMode
  systemReducedMotion?: boolean
  overrides?: unknown
}

export interface ResolvedAppearance {
  readonly requestedKey: string | null
  readonly effectiveKey: string | null
  readonly owner: ExtensionContributionOwner | null
  readonly title: string
  readonly mode: AppearanceMode
  readonly tokens: AppearanceTokens
  readonly reducedMotion: boolean
  readonly fallback: 'default' | 'missing-theme' | 'invalid-theme' | null
  readonly overrideError: AppearanceErrorCode | null
}

function merge(
  base: AppearanceTokens,
  value?: AppearanceTokenPatch
): AppearanceTokens {
  return {
    colors: { ...base.colors, ...value?.colors },
    radii: { ...base.radii, ...value?.radii },
    borders: { ...base.borders, ...value?.borders },
    typography: { ...base.typography, ...value?.typography },
    shadows: { ...base.shadows, ...value?.shadows },
    motion: value?.motion ?? base.motion,
  }
}

/**
 * Cooperative T1, pure resolution. No CSS, IO, persistence or plugin execution.
 * The Host supplies its complete light/dark defaults and keeps control of choice.
 */
export function createAppearanceResolver(defaultsValue: unknown) {
  const defaults = parse(defaultsSchema, defaultsValue, 'INVALID_DEFAULTS')
  let previous: ResolvedAppearance | undefined
  let previousJson: string | undefined
  return (request: AppearanceRequest): ResolvedAppearance => {
    if (
      !['light', 'dark', 'system'].includes(request.mode) ||
      !['light', 'dark'].includes(request.systemMode) ||
      (request.selectedKey !== null &&
        typeof request.selectedKey !== 'string') ||
      (typeof request.selectedKey === 'string' &&
        (request.selectedKey.length > 257 ||
          !/^[a-z0-9]+(?:-[a-z0-9]+)*:[a-z0-9]+(?:-[a-z0-9]+)*$/.test(
            request.selectedKey
          ))) ||
      (request.systemReducedMotion !== undefined &&
        typeof request.systemReducedMotion !== 'boolean')
    )
      throw new AppearanceError('INVALID_REQUEST')
    const mode = request.mode === 'system' ? request.systemMode : request.mode
    let theme: AppearanceTheme | undefined
    let owner: ExtensionContributionOwner | null = null
    let effectiveKey: string | null = null
    let fallback: ResolvedAppearance['fallback'] =
      request.selectedKey === null ? 'default' : 'missing-theme'
    if (request.selectedKey !== null) {
      const entry = request.contributions.find(
        item => item.key === request.selectedKey
      )
      if (entry) {
        fallback = 'invalid-theme'
        if (entry.kind === 'theme') {
          try {
            theme = parseAppearanceTheme(entry.value)
            owner = parseContributionOwner(entry.owner)
            effectiveKey = entry.key
            fallback = null
          } catch {
            // A broken selected contribution must not break the recovery UI.
            theme = undefined
          }
        }
      }
    }
    let overrides: AppearanceOverrides | undefined
    let overrideError: AppearanceErrorCode | null = null
    if (request.overrides !== undefined) {
      try {
        overrides = parseAppearanceOverrides(request.overrides)
      } catch (error) {
        overrideError =
          error instanceof AppearanceError ? error.code : 'INVALID_OVERRIDES'
      }
    }
    let resolved = defaults.modes[mode]
    for (const layer of [
      theme?.common,
      theme?.modes?.[mode],
      overrides?.common,
      overrides?.modes?.[mode],
    ]) {
      resolved = merge(resolved, layer)
    }
    const next = freeze({
      requestedKey: request.selectedKey,
      effectiveKey,
      owner,
      title: theme?.title ?? defaults.title,
      mode,
      tokens: resolved,
      reducedMotion:
        request.systemReducedMotion === true || resolved.motion === 'reduced',
      fallback,
      overrideError,
    })
    const nextJson = JSON.stringify(next)
    if (previous && previousJson === nextJson) return previous
    previous = next
    previousJson = nextJson
    return next
  }
}
