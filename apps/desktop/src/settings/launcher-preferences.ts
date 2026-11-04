import { useRef, useState } from 'react'

export interface LauncherPreferences {
  formatVersion: 1
  historyEnabled: boolean
  pinnedIds: string[]
  recentIds: string[]
  frequencies: Record<string, number>
  aliases: Record<string, string[]>
}
export const launcherPreferencesKey = 'flowtools.desktop.launcher.v1'
export const defaultLauncherPreferences = (): LauncherPreferences => ({
  formatVersion: 1,
  historyEnabled: true,
  pinnedIds: ['flowtools:settings', 'flowtools:plugin-market'],
  recentIds: [],
  frequencies: {},
  aliases: {},
})
const object = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === 'object' && !Array.isArray(value)
const ids = (value: unknown): value is string[] =>
  Array.isArray(value) &&
  value.length <= 40 &&
  new Set(value).size === value.length &&
  value.every(id => typeof id === 'string' && id.length > 0 && id.length <= 256)
export function parseLauncherPreferences(text: string): LauncherPreferences {
  if (text.length > 65536) throw new Error('PREFERENCES_INVALID')
  const value: unknown = JSON.parse(text)
  if (
    !object(value) ||
    Object.keys(value).sort().join(',') !==
      'aliases,formatVersion,frequencies,historyEnabled,pinnedIds,recentIds' ||
    value.formatVersion !== 1 ||
    typeof value.historyEnabled !== 'boolean' ||
    !ids(value.pinnedIds) ||
    !ids(value.recentIds) ||
    !object(value.frequencies) ||
    !object(value.aliases) ||
    Object.keys(value.frequencies).length > 256 ||
    Object.keys(value.aliases).length > 256 ||
    Object.entries(value.frequencies).some(
      ([id, count]) =>
        !id ||
        id.length > 256 ||
        !Number.isInteger(count) ||
        Number(count) < 1 ||
        Number(count) > 1_000_000
    ) ||
    Object.entries(value.aliases).some(
      ([id, aliases]) =>
        !id ||
        id.length > 256 ||
        !Array.isArray(aliases) ||
        aliases.length > 8 ||
        aliases.some(
          alias =>
            typeof alias !== 'string' || !alias.trim() || alias.length > 128
        )
    ) ||
    (!value.historyEnabled &&
      (value.recentIds.length !== 0 ||
        Object.keys(value.frequencies).length !== 0))
  )
    throw new Error('PREFERENCES_INVALID')
  return value as unknown as LauncherPreferences
}
export function recordLauncherOpen(
  value: LauncherPreferences,
  id: string
): LauncherPreferences {
  if (!value.historyEnabled) return value
  const entries = Object.entries(value.frequencies)
    .filter(([key]) => key !== id)
    .slice(-255)
  return {
    ...value,
    recentIds: [id, ...value.recentIds.filter(item => item !== id)].slice(
      0,
      40
    ),
    frequencies: Object.fromEntries([
      ...entries,
      [
        id,
        Math.min(
          1_000_000,
          (Object.prototype.hasOwnProperty.call(value.frequencies, id)
            ? value.frequencies[id]!
            : 0) + 1
        ),
      ],
    ]),
  }
}
function load() {
  try {
    const raw = localStorage.getItem(launcherPreferencesKey)
    return {
      value: raw ? parseLauncherPreferences(raw) : defaultLauncherPreferences(),
      error: false,
    }
  } catch {
    return { value: defaultLauncherPreferences(), error: true }
  }
}
export function useLauncherPreferences() {
  const [state, setState] = useState(load)
  const current = useRef(state)
  const save = (value: LauncherPreferences, explicit = false) => {
    if (current.current.error && !explicit) return
    try {
      parseLauncherPreferences(JSON.stringify(value))
    } catch {
      current.current = { ...current.current, error: true }
      setState(current.current)
      return
    }
    try {
      localStorage.setItem(launcherPreferencesKey, JSON.stringify(value))
      current.current = { value, error: false }
    } catch {
      current.current = { value, error: true }
    }
    setState(current.current)
  }
  const clear = () => {
    const value = { ...current.current.value, recentIds: [], frequencies: {} }
    try {
      localStorage.removeItem('flowtools.desktop.recentCommandIds')
      localStorage.setItem(launcherPreferencesKey, JSON.stringify(value))
      current.current = { value, error: false }
    } catch {
      current.current = { value, error: true }
    }
    setState(current.current)
  }
  return {
    preferences: state.value,
    preferencesError: state.error,
    save,
    clear,
  }
}
