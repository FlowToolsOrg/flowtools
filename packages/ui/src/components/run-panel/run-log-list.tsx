'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Chip } from '@heroui/react'

import { BadgeAlertIcon, CircleHelpIcon, XIcon } from '../../icons'
import { cn } from '../../utils/class-name'

import { type RunLogEntry } from './types'

export interface RunLogListProps extends Omit<
  ComponentPropsWithoutRef<'section'>,
  'onSelect'
> {
  entries: RunLogEntry[]
  onSelect?: (entryId: string) => void
}

const levelColorMap: Record<
  RunLogEntry['level'],
  'default' | 'warning' | 'danger'
> = {
  info: 'default',
  warn: 'warning',
  error: 'danger',
}

const levelIcons: Record<RunLogEntry['level'], typeof CircleHelpIcon> = {
  info: CircleHelpIcon,
  warn: BadgeAlertIcon,
  error: XIcon,
}

export const RunLogList = ({
  entries,
  onSelect,
  className,
  ...props
}: RunLogListProps) => {
  return (
    <section
      className={cn(
        'rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface)] p-4',
        className
      )}
      {...props}
    >
      <ul className="space-y-2">
        {entries.map(entry => (
          <li key={entry.id}>
            <button
              className="flex w-full items-center justify-between gap-3 rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-2 text-left transition hover:bg-[var(--surface-tertiary)]"
              onClick={() => onSelect?.(entry.id)}
              type="button"
            >
              <div className="min-w-0 space-y-0.5">
                <p className="truncate text-sm text-[var(--foreground)]">
                  {entry.message}
                </p>
                <p className="text-xs text-[var(--muted)]">{entry.timestamp}</p>
              </div>
              <Chip color={levelColorMap[entry.level]} size="sm" variant="soft">
                <span className="flex items-center gap-1">
                  {(() => {
                    const LevelIcon = levelIcons[entry.level]
                    return <LevelIcon size={12} />
                  })()}
                  {entry.level}
                </span>
              </Chip>
            </button>
          </li>
        ))}
      </ul>
    </section>
  )
}
