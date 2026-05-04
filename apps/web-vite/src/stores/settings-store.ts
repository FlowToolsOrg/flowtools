import { persist } from 'zustand/middleware'
import { createStore } from 'zustand/vanilla'

interface SettingsState {
  autoUpdate: boolean
  telemetry: boolean
  startupMode: string
  workspaceName: string
  commandPaletteShortcut: string
}

interface SettingsActions {
  setAutoUpdate: (value: boolean) => void
  setTelemetry: (value: boolean) => void
  setStartupMode: (value: string) => void
  setWorkspaceName: (value: string) => void
}

export type SettingsStore = SettingsState & SettingsActions

export const settingsStore = createStore<SettingsState & SettingsActions>()(
  persist(
    set => ({
      autoUpdate: true,
      telemetry: false,
      startupMode: 'workspace',
      workspaceName: 'Flow Workspace',
      commandPaletteShortcut: 'mod+k',

      setAutoUpdate(value: boolean) {
        set({ autoUpdate: value })
      },

      setTelemetry(value: boolean) {
        set({ telemetry: value })
      },

      setStartupMode(value: string) {
        set({ startupMode: value })
      },

      setWorkspaceName(value: string) {
        set({ workspaceName: value })
      },
    }),
    {
      name: 'flowtools-settings',
    }
  )
)
