import type { ReactNode } from 'react'

import { createContext, useContext, useLayoutEffect, useState } from 'react'

export type DesktopAppearance = 'system' | 'light' | 'dark'

const storageKey = 'flowtools.desktop.appearance'

interface AppearanceState {
  mode: DesktopAppearance
  notice: string | null
}

function readAppearance(): AppearanceState {
  try {
    const raw = window.localStorage.getItem(storageKey)
    if (raw === null) return { mode: 'system', notice: null }
    if (raw.length > 128) throw new Error('Invalid appearance record')
    const record: unknown = JSON.parse(raw)
    if (
      !record ||
      typeof record !== 'object' ||
      Array.isArray(record) ||
      Object.keys(record).length !== 2 ||
      !('formatVersion' in record) ||
      record.formatVersion !== 1 ||
      !('mode' in record) ||
      (record.mode !== 'system' &&
        record.mode !== 'light' &&
        record.mode !== 'dark')
    ) {
      throw new Error('Invalid appearance record')
    }
    return { mode: record.mode, notice: null }
  } catch {
    // Keep unreadable records intact until an explicit selection or reset.
    return {
      mode: 'system',
      notice: '无法读取外观设置，暂时跟随系统。请选择外观或重置后重试。',
    }
  }
}

interface AppearanceContext extends AppearanceState {
  select: (mode: DesktopAppearance) => void
  reset: () => void
}

const AppearanceContext = createContext<AppearanceContext | null>(null)

/** Host UI preference only; never a plugin resource, grant or Runtime store. */
export function DesktopAppearanceProvider({
  children,
}: {
  children: ReactNode
}) {
  const [state, setState] = useState(readAppearance)

  useLayoutEffect(() => {
    const root = document.documentElement
    const previous = root.getAttribute('data-theme')
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const apply = () => {
      root.dataset.theme =
        state.mode === 'system'
          ? media.matches
            ? 'dark'
            : 'light'
          : state.mode
    }
    apply()
    media.addEventListener('change', apply)
    return () => {
      media.removeEventListener('change', apply)
      if (previous === null) root.removeAttribute('data-theme')
      else root.setAttribute('data-theme', previous)
    }
  }, [state.mode])

  const select = (mode: DesktopAppearance) => {
    let notice = '外观已保存。'
    try {
      window.localStorage.setItem(
        storageKey,
        JSON.stringify({ formatVersion: 1, mode })
      )
    } catch {
      notice =
        '外观已在本次窗口生效，但无法保存。重启后可能恢复原设置，请重试。'
    }
    setState({ mode, notice })
  }

  const reset = () => {
    let notice = '已恢复系统外观。'
    try {
      window.localStorage.removeItem(storageKey)
    } catch {
      notice =
        '本次窗口已跟随系统，但无法清除已保存的外观。重启后可能恢复原设置，请重试。'
    }
    setState({ mode: 'system', notice })
  }

  return (
    <AppearanceContext.Provider value={{ ...state, select, reset }}>
      {children}
    </AppearanceContext.Provider>
  )
}

export function useDesktopAppearance() {
  const context = useContext(AppearanceContext)
  if (!context) throw new Error('DesktopAppearanceProvider required')
  return context
}
