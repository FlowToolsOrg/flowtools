'use client'

import { type ComponentProps, type ComponentPropsWithoutRef } from 'react'

import { Card, Chip } from '@heroui/react'

import { cn } from '../../utils/class-name'

import { type ToolMarketStatus } from './types'

export interface ToolCardRootProps extends Omit<
  ComponentProps<typeof Card>,
  'onClick'
> {
  onPress?: () => void
}

export interface ToolCardHeaderProps extends ComponentPropsWithoutRef<'header'> {}

export interface ToolCardTitleProps extends ComponentPropsWithoutRef<'h3'> {}

export interface ToolCardDescriptionProps extends ComponentPropsWithoutRef<'p'> {}

export interface ToolCardMetaProps extends ComponentPropsWithoutRef<'div'> {
  status?: ToolMarketStatus
  version?: string
}

export interface ToolCardTagsProps extends ComponentPropsWithoutRef<'div'> {}

export interface ToolCardActionsProps extends ComponentPropsWithoutRef<'div'> {}

const statusTextMap: Record<ToolMarketStatus, string> = {
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

const ToolCardRoot = ({
  children,
  className,
  onPress,
  ...props
}: ToolCardRootProps) => {
  return (
    <Card
      className={cn(
        'rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface)] p-4',
        onPress &&
          'cursor-pointer transition hover:-translate-y-0.5 hover:border-[var(--accent)]/30 hover:shadow-md',
        className
      )}
      onClick={onPress}
      onKeyDown={event => {
        if (!onPress) {
          return
        }

        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault()
          onPress()
        }
      }}
      role={onPress ? 'button' : undefined}
      tabIndex={onPress ? 0 : undefined}
      variant="secondary"
      {...props}
    >
      {children}
    </Card>
  )
}

const ToolCardHeader = ({
  children,
  className,
  ...props
}: ToolCardHeaderProps) => {
  return (
    <header className={cn('space-y-1', className)} {...props}>
      {children}
    </header>
  )
}

const ToolCardTitle = ({
  children,
  className,
  ...props
}: ToolCardTitleProps) => {
  return (
    <h3
      className={cn(
        'text-base font-semibold text-[var(--foreground)]',
        className
      )}
      {...props}
    >
      {children}
    </h3>
  )
}

const ToolCardDescription = ({
  children,
  className,
  ...props
}: ToolCardDescriptionProps) => {
  return (
    <p className={cn('text-sm text-[var(--muted)]', className)} {...props}>
      {children}
    </p>
  )
}

const ToolCardMeta = ({
  status,
  version,
  children,
  className,
  ...props
}: ToolCardMetaProps) => {
  return (
    <div
      className={cn('mt-3 flex flex-wrap items-center gap-2', className)}
      {...props}
    >
      {status ? (
        <Chip color={statusColorMap[status]} size="sm" variant="soft">
          {statusTextMap[status]}
        </Chip>
      ) : null}
      {version ? (
        <Chip size="sm" variant="secondary">
          v{version}
        </Chip>
      ) : null}
      {children}
    </div>
  )
}

const ToolCardTags = ({ children, className, ...props }: ToolCardTagsProps) => {
  return (
    <div className={cn('mt-3 flex flex-wrap gap-2', className)} {...props}>
      {children}
    </div>
  )
}

const ToolCardActions = ({
  children,
  className,
  ...props
}: ToolCardActionsProps) => {
  return (
    <div
      className={cn('mt-4 flex flex-wrap items-center gap-2', className)}
      {...props}
    >
      {children}
    </div>
  )
}

export const ToolCard = Object.assign(ToolCardRoot, {
  Root: ToolCardRoot,
  Header: ToolCardHeader,
  Title: ToolCardTitle,
  Description: ToolCardDescription,
  Meta: ToolCardMeta,
  Tags: ToolCardTags,
  Actions: ToolCardActions,
})
