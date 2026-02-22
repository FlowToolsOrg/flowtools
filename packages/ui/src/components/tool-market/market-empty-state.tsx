'use client'

import { type ComponentPropsWithoutRef, type ReactNode } from 'react'

import { Card } from '@heroui/react'

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
        className="rounded-3xl border border-dashed border-slate-300/80 bg-white/70 p-6 text-center"
        variant="secondary"
      >
        <Card.Header className="flex flex-col items-center gap-2">
          <Card.Title className="text-lg">{title}</Card.Title>
          {description ? (
            <Card.Description className="max-w-md text-sm text-slate-500">
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
