import { persist } from 'zustand/middleware'
import { createStore } from 'zustand/vanilla'

export interface RunHistoryEntry {
  id: string
  pluginId: string
  pluginName: string
  input: string
  output: string
  status: 'success' | 'error'
  error?: string
  startedAt: number
  durationMs: number
}

interface RunHistoryState {
  entries: RunHistoryEntry[]
}

interface RunHistoryActions {
  addEntry: (entry: RunHistoryEntry) => void
  getByPlugin: (pluginId: string) => RunHistoryEntry[]
  clear: (pluginId?: string) => void
}

export type RunHistoryStore = RunHistoryState & RunHistoryActions

export const runHistoryStore = createStore<
  RunHistoryState & RunHistoryActions
>()(
  persist(
    (set, get) => ({
      entries: [],

      addEntry(entry: RunHistoryEntry) {
        set(state => ({
          entries: [entry, ...state.entries].slice(0, 200),
        }))
      },

      getByPlugin(pluginId: string) {
        return get().entries.filter(e => e.pluginId === pluginId)
      },

      clear(pluginId?: string) {
        if (pluginId) {
          set(state => ({
            entries: state.entries.filter(e => e.pluginId !== pluginId),
          }))
        } else {
          set({ entries: [] })
        }
      },
    }),
    {
      name: 'flow-tool-run-history',
    }
  )
)
