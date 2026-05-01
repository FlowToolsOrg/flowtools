'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Chip } from '@heroui/react'

import { cn } from '../../utils/class-name'

import { type ToolVersionRecord } from './types'

export interface ToolVersionTimelineProps extends Omit<
  ComponentPropsWithoutRef<'section'>,
  'onSelect'
> {
  records: ToolVersionRecord[]
  onSelect?: (recordId: string) => void
}

export const ToolVersionTimeline = ({
  records,
  onSelect,
  className,
  ...props
}: ToolVersionTimelineProps) => {
  return (
    <section
      className={cn(
        'w-full rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface)] p-4',
        className
      )}
      {...props}
    >
      <ol className="space-y-3">
        {records.map(record => (
          <li key={record.id}>
            <button
              className="w-full rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface-secondary)] px-4 py-3 text-left transition hover:bg-[var(--surface-tertiary)]"
              onClick={() => onSelect?.(record.id)}
              type="button"
            >
              <div className="flex items-center justify-between gap-2">
                <p className="text-sm font-medium text-[var(--foreground)]">
                  {record.version}
                </p>
                <Chip size="sm" variant="tertiary">
                  {record.date}
                </Chip>
              </div>
              {record.notes ? (
                <p className="mt-1 text-xs text-[var(--muted)]">
                  {record.notes}
                </p>
              ) : null}
            </button>
          </li>
        ))}
      </ol>
    </section>
  )
}
