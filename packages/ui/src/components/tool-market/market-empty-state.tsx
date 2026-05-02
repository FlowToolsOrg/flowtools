'use client'

import { type ComponentPropsWithoutRef, type ReactNode } from 'react'

import { Card } from '@heroui/react'

import { SearchIcon } from '../../icons'
import { cn } from '../../utils/class-name'

export interface MarketEmptyStateProps extends Omit<
  ComponentPropsWithoutRef<'section'>,
  'title'
> {
  title: string
  description?: string
  action?: ReactNode
}

export const MarketEmptyState = ({
  title,
  description,
  action,
  className,
  ...props
}: MarketEmptyStateProps) => {
  return (
    <section className={cn('w-full', className)} {...props}>
      <Card
        className="rounded-[var(--radius)] border border-dashed border-[var(--border)] bg-[var(--surface-secondary)] p-6 text-center"
        variant="secondary"
      >
        <Card.Header className="flex flex-col items-center gap-2">
          <SearchIcon className="text-[var(--muted)]" size={40} />
          <Card.Title className="text-lg text-[var(--foreground)]">
            {title}
          </Card.Title>
          {description ? (
            <Card.Description className="max-w-md text-sm text-[var(--muted)]">
              {description}
            </Card.Description>
          ) : null}
        </Card.Header>
        {action ? (
          <Card.Footer className="justify-center">{action}</Card.Footer>
        ) : null}
      </Card>
    </section>
  )
}
