'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Button, Chip } from '@heroui/react'

import { cn } from '../../utils/class-name'

import { type RunHistoryEntry } from './types'

export interface RunHistoryPanelProps extends Omit<
  ComponentPropsWithoutRef<'section'>,
  'onSelect'
> {
  entries: RunHistoryEntry[]
  onSelect?: (entryId: string) => void
}

const statusColorMap: Record<
  RunHistoryEntry['status'],
  'default' | 'accent' | 'success' | 'danger'
> = {
  idle: 'default',
  running: 'accent',
  success: 'success',
  error: 'danger',
}

export const RunHistoryPanel = ({
  entries,
  onSelect,
  className,
  ...props
}: RunHistoryPanelProps) => {
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
          <li
            key={entry.id}
            className="flex items-center justify-between gap-2 rounded-2xl border border-slate-200/80 bg-white px-3 py-2"
          >
            <div className="min-w-0">
              <p className="truncate text-sm font-medium text-slate-900">
                {entry.title}
              </p>
              <p className="text-xs text-slate-500">{entry.startedAt}</p>
            </div>
            <div className="flex items-center gap-2">
              <Chip
                color={statusColorMap[entry.status]}
                size="sm"
                variant="soft"
              >
                {entry.status}
              </Chip>
              <Button
                onPress={() => onSelect?.(entry.id)}
                size="sm"
                variant="ghost"
              >
                Open
              </Button>
            </div>
          </li>
        ))}
      </ul>
    </section>
  )
}
