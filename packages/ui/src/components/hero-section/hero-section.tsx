'use client'

import { type ComponentPropsWithoutRef, type ReactNode } from 'react'

import { Card, Chip } from '@heroui/react'

import { cn } from '../../utils/class-name'

export interface HeroStat {
  id: string
  label: string
  value: string | number
}

export interface HeroSectionProps extends Omit<
  ComponentPropsWithoutRef<'section'>,
  'title'
> {
  title: string
  description?: string
  eyebrow?: string
  badges?: string[]
  stats?: HeroStat[]
  actions?: ReactNode
  tone?: 'default' | 'secondary' | 'tertiary'
}

export const HeroSection = ({
  title,
  description,
  eyebrow,
  badges,
  stats,
  actions,
  tone = 'default',
  className,
  ...props
}: HeroSectionProps) => {
  return (
    <section className={cn('w-full', className)} {...props}>
      <Card
        className="overflow-hidden rounded-(--radius) border border-border bg-surface"
        variant={tone}
      >
        <Card.Header className="flex flex-col gap-4">
          {eyebrow ? (
            <p className="text-xs font-semibold tracking-[0.18em] text-accent uppercase">
              {eyebrow}
            </p>
          ) : null}
          <div className="flex flex-col gap-2">
            <Card.Title className="text-2xl leading-tight font-semibold text-foreground md:text-3xl">
              {title}
            </Card.Title>
            {description ? (
              <Card.Description className="max-w-3xl text-sm leading-6 text-muted md:text-base">
                {description}
              </Card.Description>
            ) : null}
          </div>
          {badges && badges.length > 0 ? (
            <div className="flex flex-wrap gap-2">
              {badges.map(badge => (
                <Chip key={badge} color="accent" size="sm" variant="soft">
                  {badge}
                </Chip>
              ))}
            </div>
          ) : null}
        </Card.Header>
        {stats && stats.length > 0 ? (
          <Card.Content>
            <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
              {stats.map(stat => (
                <div
                  key={stat.id}
                  className="rounded-(--radius) border border-border bg-surface-secondary px-4 py-3"
                >
                  <p className="text-xs font-medium tracking-wide text-muted uppercase">
                    {stat.label}
                  </p>
                  <p className="mt-1 text-lg font-semibold text-foreground">
                    {stat.value}
                  </p>
                </div>
              ))}
            </div>
          </Card.Content>
        ) : null}
        {actions ? (
          <Card.Footer className="flex flex-wrap items-center gap-3">
            {actions}
          </Card.Footer>
        ) : null}
      </Card>
    </section>
  )
}
