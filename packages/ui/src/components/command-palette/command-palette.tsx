import type { CommandPaletteItem, CommandPaletteRootProps } from './types'

import { useCallback, useEffect, useRef, useState } from 'react'

function filterItems(
  items: CommandPaletteItem[],
  query: string
): CommandPaletteItem[] {
  const trimmed = query.trim().toLowerCase()

  if (trimmed.length === 0) {
    return items
  }

  return items.filter(item => {
    const titleMatch = item.title.toLowerCase().includes(trimmed)
    const descMatch = item.description?.toLowerCase().includes(trimmed)
    const sourceMatch = item.source?.toLowerCase().includes(trimmed)

    return titleMatch || descMatch || sourceMatch
  })
}

function CommandPaletteRoot({
  open,
  onClose,
  items,
  onSelect,
  placeholder = 'Search commands...',
}: CommandPaletteRootProps) {
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  const filtered = filterItems(items, query)

  useEffect(() => {
    if (open) {
      setQuery('')
      setSelectedIndex(0)

      requestAnimationFrame(() => {
        inputRef.current?.focus()
      })
    }
  }, [open])

  useEffect(() => {
    setSelectedIndex(0)
  }, [query])

  const scrollToIndex = useCallback((index: number) => {
    const list = listRef.current
    if (!list) return

    const item = list.children[index] as HTMLElement | undefined
    if (item) {
      item.scrollIntoView({ block: 'nearest' })
    }
  }, [])

  const handleSelect = useCallback(
    (item: CommandPaletteItem) => {
      onSelect(item)
      onClose()
    },
    [onSelect, onClose]
  )

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown': {
          e.preventDefault()
          const next = Math.min(selectedIndex + 1, filtered.length - 1)
          setSelectedIndex(next)
          scrollToIndex(next)
          break
        }
        case 'ArrowUp': {
          e.preventDefault()
          const prev = Math.max(selectedIndex - 1, 0)
          setSelectedIndex(prev)
          scrollToIndex(prev)
          break
        }
        case 'Enter': {
          e.preventDefault()
          const selected = filtered[selectedIndex]
          if (selected) {
            handleSelect(selected)
          }
          break
        }
        case 'Escape': {
          e.preventDefault()
          onClose()
          break
        }
      }
    },
    [selectedIndex, filtered, handleSelect, onClose, scrollToIndex]
  )

  if (!open) {
    return null
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]"
      onKeyDown={handleKeyDown}
    >
      <div
        className="fixed inset-0 bg-black/50 backdrop-blur-sm"
        onClick={onClose}
      />
      <div className="relative z-10 w-full max-w-150 overflow-hidden rounded-xl border border-border bg-surface shadow-2xl">
        <div className="flex items-center gap-3 border-b border-border px-4 py-3">
          <svg
            className="size-5 shrink-0 text-muted"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            viewBox="0 0 24 24"
          >
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.3-4.3" />
          </svg>
          <input
            ref={inputRef}
            className="flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted"
            onChange={e => setQuery(e.target.value)}
            placeholder={placeholder}
            type="text"
            value={query}
          />
          <kbd className="rounded border border-border px-1.5 py-0.5 text-xs text-muted">
            ESC
          </kbd>
        </div>

        <div ref={listRef} className="max-h-80 overflow-y-auto p-1">
          {filtered.length === 0 ? (
            <div className="flex flex-col items-center gap-2 py-8 text-center">
              <p className="text-sm text-muted">No commands found</p>
              <p className="text-xs text-muted">Try a different search term</p>
            </div>
          ) : (
            filtered.map((item, index) => (
              <button
                key={item.id}
                className={`flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left transition ${
                  index === selectedIndex
                    ? 'bg-[var(--accent)]/10 text-[var(--accent)]'
                    : 'text-(--foreground) hover:bg-(--surface-secondary)'
                }`}
                onClick={() => handleSelect(item)}
                onMouseEnter={() => setSelectedIndex(index)}
                type="button"
              >
                <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="truncate text-sm font-medium">
                    {item.title}
                  </span>
                  {item.description ? (
                    <span className="truncate text-xs text-muted">
                      {item.description}
                    </span>
                  ) : null}
                </div>
                {item.source ? (
                  <span className="shrink-0 text-xs text-muted">
                    {item.source}
                  </span>
                ) : null}
                {item.shortcut ? (
                  <kbd className="shrink-0 rounded border border-border px-1.5 py-0.5 text-xs text-muted">
                    {item.shortcut}
                  </kbd>
                ) : null}
              </button>
            ))
          )}
        </div>

        <div className="flex items-center justify-between border-t border-border px-4 py-2 text-xs text-muted">
          <span>
            {filtered.length} command{filtered.length !== 1 ? 's' : ''}
          </span>
          <div className="flex items-center gap-2">
            <span>
              <kbd className="rounded border border-border px-1">↑↓</kbd>{' '}
              navigate
            </span>
            <span>
              <kbd className="rounded border border-border px-1">↵</kbd> select
            </span>
            <span>
              <kbd className="rounded border border-border px-1">esc</kbd> close
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}

export { CommandPaletteRoot as CommandPalette }
export type { CommandPaletteItem, CommandPaletteRootProps }
