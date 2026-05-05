'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { cn } from '../../utils/class-name'

export interface ToolMarketPageRootProps extends ComponentPropsWithoutRef<'section'> {}

export interface ToolMarketPageHeaderProps extends ComponentPropsWithoutRef<'header'> {}

export interface ToolMarketPageContentProps extends ComponentPropsWithoutRef<'div'> {}

const ToolMarketPageRoot = ({
  children,
  className,
  ...props
}: ToolMarketPageRootProps) => {
  return (
    <section className={cn('space-y-4', className)} {...props}>
      {children}
    </section>
  )
}

const ToolMarketPageHeader = ({
  children,
  className,
  ...props
}: ToolMarketPageHeaderProps) => {
  return (
    <header
      className={cn(
        'space-y-2 rounded-(--radius) border border-border bg-surface p-5',
        className
      )}
      {...props}
    >
      {children}
    </header>
  )
}

const ToolMarketPageContent = ({
  children,
  className,
  ...props
}: ToolMarketPageContentProps) => {
  return (
    <div className={cn('space-y-4', className)} {...props}>
      {children}
    </div>
  )
}

export const ToolMarketPage = Object.assign(ToolMarketPageRoot, {
  Root: ToolMarketPageRoot,
  Header: ToolMarketPageHeader,
  Content: ToolMarketPageContent,
})
