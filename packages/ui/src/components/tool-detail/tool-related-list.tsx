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
        'w-full rounded-(--radius) border border-(--border) bg-(--surface) p-4',
        className
      )}
      {...props}
    >
      <ul className="space-y-2">
        {items.map(item => (
          <li
            key={item.id}
            className="flex items-center justify-between gap-3 rounded-(--radius) border border-border bg-(--surface-secondary) px-3 py-2"
          >
            <div className="min-w-0">
              <p className="truncate text-sm font-medium text-(--foreground)">
                {item.name}
              </p>
              {item.description ? (
                <p className="truncate text-xs text-(--muted)">
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
