'use client'

import { Chip } from '@heroui/react'
import { type ComponentPropsWithoutRef } from 'react'

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

export const RunLogList = ({
  entries,
  onSelect,
  className,
  ...props
}: RunLogListProps) => {
  return (
    <section
      className={cn(
        'rounded-3xl border border-slate-200/80 bg-white p-4',
        className
      )}
      {...props}
    >
      <ul className="space-y-2">
        {entries.map(entry => (
          <li key={entry.id}>
            <button
              className="flex w-full items-center justify-between gap-3 rounded-2xl border border-slate-200/80 bg-white px-3 py-2 text-left transition hover:bg-slate-50"
              onClick={() => onSelect?.(entry.id)}
              type="button"
            >
              <div className="min-w-0 space-y-0.5">
                <p className="truncate text-sm text-slate-800">
                  {entry.message}
                </p>
                <p className="text-xs text-slate-500">{entry.timestamp}</p>
              </div>
              <Chip color={levelColorMap[entry.level]} size="sm" variant="soft">
                {entry.level}
              </Chip>
            </button>
          </li>
        ))}
      </ul>
    </section>
  )
}
