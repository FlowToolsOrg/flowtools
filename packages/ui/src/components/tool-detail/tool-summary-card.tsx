'use client'

import { Card, Chip } from '@heroui/react'
import { type ComponentPropsWithoutRef, type ReactNode } from 'react'

import { cn } from '../../utils/class-name'
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

const statusLabelMap: Record<ToolMarketStatus, string> = {
  stable: 'Stable',
  beta: 'Beta',
  experimental: 'Experimental',
  deprecated: 'Deprecated',
}

const statusColorMap: Record<
  ToolMarketStatus,
  'success' | 'accent' | 'warning' | 'danger'
> = {
  stable: 'success',
  beta: 'accent',
  experimental: 'warning',
  deprecated: 'danger',
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
        className="rounded-3xl border border-slate-200/80 bg-white/90 shadow-sm"
        variant="secondary"
      >
        <Card.Header className="flex flex-col gap-2">
          <Card.Title>{title}</Card.Title>
          {description ? (
            <Card.Description className="text-sm text-slate-500">
              {description}
            </Card.Description>
          ) : null}
          <div className="flex flex-wrap items-center gap-2">
            {category ? (
              <Chip size="sm" variant="tertiary">
                {category}
              </Chip>
            ) : null}
            {status ? (
              <Chip color={statusColorMap[status]} size="sm" variant="soft">
                {statusLabelMap[status]}
              </Chip>
            ) : null}
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
