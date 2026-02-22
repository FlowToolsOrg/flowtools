'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Input, TextField } from '@heroui/react'

import { cn } from '../../utils/class-name'

export interface MarketToolbarRootProps extends ComponentPropsWithoutRef<'section'> {}

export interface MarketToolbarSearchProps extends Omit<
  ComponentPropsWithoutRef<'div'>,
  'onChange'
> {
  value: string
  onChange?: (value: string) => void
  placeholder?: string
  ariaLabel?: string
}

export interface MarketToolbarFiltersProps extends ComponentPropsWithoutRef<'div'> {}

export interface MarketToolbarActionsProps extends ComponentPropsWithoutRef<'div'> {}

const MarketToolbarRoot = ({
  children,
  className,
  ...props
}: MarketToolbarRootProps) => {
  return (
    <section
      className={cn(
        'flex flex-col gap-3 rounded-3xl border border-slate-200/80 bg-white/90 p-4 shadow-sm md:flex-row md:items-end md:justify-between',
        className
      )}
      {...props}
    >
      {children}
    </section>
  )
}

const MarketToolbarSearch = ({
  value,
  onChange,
  placeholder,
  ariaLabel = 'Search tools',
  className,
  ...props
}: MarketToolbarSearchProps) => {
  return (
    <div className={cn('w-full md:max-w-sm', className)} {...props}>
      <TextField aria-label={ariaLabel} onChange={onChange} value={value}>
        <Input placeholder={placeholder ?? 'Search tools'} />
      </TextField>
    </div>
  )
}

const MarketToolbarFilters = ({
  children,
  className,
  ...props
}: MarketToolbarFiltersProps) => {
  return (
    <div
      className={cn('flex flex-wrap items-center gap-2', className)}
      {...props}
    >
      {children}
    </div>
  )
}

const MarketToolbarActions = ({
  children,
  className,
  ...props
}: MarketToolbarActionsProps) => {
  return (
    <div
      className={cn('flex flex-wrap items-center gap-2', className)}
      {...props}
    >
      {children}
    </div>
  )
}

export const MarketToolbar = Object.assign(MarketToolbarRoot, {
  Root: MarketToolbarRoot,
  Search: MarketToolbarSearch,
  Filters: MarketToolbarFilters,
  Actions: MarketToolbarActions,
})
