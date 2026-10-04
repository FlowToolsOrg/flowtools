'use client'

import { type ComponentPropsWithoutRef, type ReactNode } from 'react'

import { Card, Chip } from '@heroui/react'

import { cn } from '../../utils/class-name'
import { PluginMaturityBadge } from '../plugin-status'
import { type ToolMarketStatus } from '../tool-market'

export interface ToolSummaryCardProps extends Omit<
  ComponentPropsWithoutRef<'section'>,
  'title'
> {
  title: string
  description?: string
  category?: string
  status?: ToolMarketStatus
  version?: string
  actions?: ReactNode
}

export const ToolSummaryCard = ({
  title,
  description,
  category,
  status,
  version,
  actions,
  className,
  ...props
}: ToolSummaryCardProps) => {
  return (
    <section className={cn('w-full', className)} {...props}>
      <Card
        className="rounded-(--radius) border border-border bg-surface"
        variant="secondary"
      >
        <Card.Header className="flex flex-col gap-2">
          <Card.Title className="text-foreground">{title}</Card.Title>
          {description ? (
            <Card.Description className="text-sm text-muted">
              {description}
            </Card.Description>
          ) : null}
          <div className="flex flex-wrap items-center gap-2">
            {category ? (
              <Chip size="sm" variant="tertiary">
                {category}
              </Chip>
            ) : null}
            <PluginMaturityBadge maturity={status} />
            {version ? (
              <Chip size="sm" variant="secondary">
                v{version}
              </Chip>
            ) : null}
          </div>
        </Card.Header>
        {actions ? (
          <Card.Footer className="flex flex-wrap items-center gap-2">
            {actions}
          </Card.Footer>
        ) : null}
      </Card>
    </section>
  )
}
