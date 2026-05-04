import { useCallback, useEffect, useState } from 'react'

import { CommandPalette } from '@flowtools/ui'
import {
  BlocksIcon,
  HomeIcon,
  SearchIcon,
  SettingsIcon,
} from '@flowtools/ui/icons'
import {
  createRootRoute,
  Link,
  Outlet,
  useMatchRoute,
} from '@tanstack/react-router'
import { TanStackRouterDevtools } from '@tanstack/react-router-devtools'
import { useStore } from 'zustand'

import { executeCommand, getCommandPaletteItems } from '@/stores/command-store'
import { pluginRegistryStore } from '@/stores/plugin-registry-store'

const navItems = [
  { to: '/', label: 'Dashboard', icon: HomeIcon },
  { to: '/tools', label: 'Tools', icon: BlocksIcon },
  { to: '/settings', label: 'Settings', icon: SettingsIcon },
] as const

const RootLayout = () => {
  const matchRoute = useMatchRoute()
  const [paletteOpen, setPaletteOpen] = useState(false)
  const { commands } = useStore(pluginRegistryStore)

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        setPaletteOpen(prev => !prev)
      }
    }

    window.addEventListener('keydown', handleKeyDown)

    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  const handleSelect = useCallback(
    async (item: { id: string }) => {
      const cmd = commands.find(c => c.id === item.id)
      if (cmd) {
        await executeCommand(cmd.id)
      }
    },
    [commands]
  )

  const items = getCommandPaletteItems()

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <aside className="hidden w-56 shrink-0 border-r border-(--border) bg-(--surface) lg:flex lg:flex-col">
        <div className="flex items-center gap-2.5 border-b border-(--border) px-4 py-4">
          <div className="flex size-8 items-center justify-center rounded-(--radius) bg-(--accent) text-sm font-bold text-(--accent-foreground)">
            F
          </div>
          <div>
            <p className="text-sm font-semibold text-(--foreground)">
              Flow Tool
            </p>
            <p className="text-xs text-(--muted)">Smart Toolbox</p>
          </div>
        </div>
        <nav className="flex-1 overflow-y-auto px-3 py-3">
          <ul className="flex flex-col gap-0.5">
            {navItems.map(item => {
              const isActive =
                item.to === '/'
                  ? matchRoute({ to: '/', fuzzy: false })
                  : matchRoute({ to: item.to, fuzzy: true })

              return (
                <li key={item.to}>
                  <Link
                    className={`flex items-center gap-3 rounded-(--radius) px-3 py-2 text-sm font-medium transition ${
                      isActive
                        ? 'bg-[var(--accent)]/10 text-[var(--accent)]'
                        : 'text-[var(--foreground)] hover:bg-[var(--surface-secondary)]'
                    }`}
                    to={item.to}
                  >
                    <item.icon
                      className={
                        isActive ? 'text-(--accent)' : 'text-(--muted)'
                      }
                      size={18}
                    />
                    {item.label}
                  </Link>
                </li>
              )
            })}
          </ul>
        </nav>
        <div className="border-t border-(--border) px-3 py-3">
          <button
            className="flex w-full items-center gap-2 rounded-(--radius) border border-(--border) px-3 py-1.5 text-xs text-(--muted) transition hover:bg-(--surface-secondary)"
            onClick={() => setPaletteOpen(true)}
            type="button"
          >
            <SearchIcon size={14} />
            <span className="flex-1 text-left">Search commands...</span>
            <kbd className="rounded border border-(--border) px-1 py-0.5">
              ⌘K
            </kbd>
          </button>
        </div>
        <div className="border-t border-(--border) px-4 py-3">
          <p className="text-xs text-(--muted)">v0.1.0</p>
        </div>
      </aside>
      <main className="flex min-w-0 flex-1 flex-col overflow-y-auto">
        <Outlet />
      </main>
      <CommandPalette
        items={items}
        onClose={() => setPaletteOpen(false)}
        onSelect={handleSelect}
        open={paletteOpen}
      />
      <TanStackRouterDevtools />
    </div>
  )
}

export const Route = createRootRoute({ component: RootLayout })
