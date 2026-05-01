'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { cn } from '../../utils/class-name'

export interface ToolDetailPageRootProps extends ComponentPropsWithoutRef<'section'> {}

export interface ToolDetailPageHeaderProps extends ComponentPropsWithoutRef<'header'> {}

export interface ToolDetailPageContentProps extends ComponentPropsWithoutRef<'div'> {}

const ToolDetailPageRoot = ({
  children,
  className,
  ...props
}: ToolDetailPageRootProps) => {
  return (
    <section className={cn('space-y-4', className)} {...props}>
      {children}
    </section>
  )
}

const ToolDetailPageHeader = ({
  children,
  className,
  ...props
}: ToolDetailPageHeaderProps) => {
  return (
    <header
      className={cn(
        'space-y-2 rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface)] p-5',
        className
      )}
      {...props}
    >
      {children}
    </header>
  )
}

const ToolDetailPageContent = ({
  children,
  className,
  ...props
}: ToolDetailPageContentProps) => {
  return (
    <div
      className={cn('grid gap-4 xl:grid-cols-[minmax(0,1fr)_20rem]', className)}
      {...props}
    >
      {children}
    </div>
  )
}

export const ToolDetailPage = Object.assign(ToolDetailPageRoot, {
  Root: ToolDetailPageRoot,
  Header: ToolDetailPageHeader,
  Content: ToolDetailPageContent,
})
