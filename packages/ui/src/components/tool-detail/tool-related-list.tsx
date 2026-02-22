'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Button } from '@heroui/react'

import { cn } from '../../utils/class-name'

import { type ToolRelatedRecord } from './types'

export interface ToolRelatedListProps extends Omit<
  ComponentPropsWithoutRef<'section'>,
  'onSelect'
> {
  items: ToolRelatedRecord[]
  onSelect?: (toolId: string) => void
}

export const ToolRelatedList = ({
  items,
  onSelect,
  className,
  ...props
}: ToolRelatedListProps) => {
  return (
    <section
      className={cn(
        'w-full rounded-3xl border border-slate-200/80 bg-white/90 p-4',
        className
      )}
      {...props}
    >
      <ul className="space-y-2">
        {items.map(item => (
          <li
            key={item.id}
            className="flex items-center justify-between gap-3 rounded-2xl border border-slate-200/80 bg-white px-3 py-2"
          >
            <div className="min-w-0">
              <p className="truncate text-sm font-medium text-slate-900">
                {item.name}
              </p>
              {item.description ? (
                <p className="truncate text-xs text-slate-500">
                  {item.description}
                </p>
              ) : null}
            </div>
            <Button
              onPress={() => onSelect?.(item.id)}
              size="sm"
              variant="ghost"
            >
              Open
            </Button>
          </li>
        ))}
      </ul>
    </section>
  )
}
