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
        'w-full rounded-3xl border border-slate-200/80 bg-white/90 p-4',
        className
      )}
      {...props}
    >
      <ol className="space-y-3">
        {records.map(record => (
          <li key={record.id}>
            <button
              className="w-full rounded-2xl border border-slate-200/80 bg-white px-4 py-3 text-left transition hover:bg-slate-50"
              onClick={() => onSelect?.(record.id)}
              type="button"
            >
              <div className="flex items-center justify-between gap-2">
                <p className="text-sm font-medium text-slate-900">
                  {record.version}
                </p>
                <Chip size="sm" variant="tertiary">
                  {record.date}
                </Chip>
              </div>
              {record.notes ? (
                <p className="mt-1 text-xs text-slate-500">{record.notes}</p>
              ) : null}
            </button>
          </li>
        ))}
      </ol>
    </section>
  )
}
