'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { cn } from '../../utils/class-name'

export interface RunPanelRootProps extends ComponentPropsWithoutRef<'section'> {}

export interface RunPanelHeaderProps extends ComponentPropsWithoutRef<'header'> {}

export interface RunPanelContentProps extends ComponentPropsWithoutRef<'div'> {}

export interface RunPanelFooterProps extends ComponentPropsWithoutRef<'footer'> {}

const RunPanelRoot = ({ children, className, ...props }: RunPanelRootProps) => {
  return (
    <section
      className={cn(
        'space-y-4 rounded-(--radius) border border-border bg-surface p-4',
        className
      )}
      {...props}
    >
      {children}
    </section>
  )
}

const RunPanelHeader = ({
  children,
  className,
  ...props
}: RunPanelHeaderProps) => {
  return (
    <header className={cn('space-y-1', className)} {...props}>
      {children}
    </header>
  )
}

const RunPanelContent = ({
  children,
  className,
  ...props
}: RunPanelContentProps) => {
  return (
    <div className={cn('space-y-3', className)} {...props}>
      {children}
    </div>
  )
}

const RunPanelFooter = ({
  children,
  className,
  ...props
}: RunPanelFooterProps) => {
  return (
    <footer
      className={cn('flex flex-wrap items-center gap-2', className)}
      {...props}
    >
      {children}
    </footer>
  )
}

export const RunPanel = Object.assign(RunPanelRoot, {
  Root: RunPanelRoot,
  Header: RunPanelHeader,
  Content: RunPanelContent,
  Footer: RunPanelFooter,
})
