'use client'

import { type ComponentPropsWithoutRef, type ReactNode } from 'react'

import { Surface } from '@heroui/react'

import { cn } from '../../utils/class-name'

export interface ToolLayoutProps extends ComponentPropsWithoutRef<'div'> {
  children: ReactNode
}

export interface ToolLayoutMainProps extends ComponentPropsWithoutRef<'main'> {
  children: ReactNode
}

export interface ToolLayoutSidebarProps extends ComponentPropsWithoutRef<'aside'> {
  children: ReactNode
  isSticky?: boolean
}

export const ToolLayout = ({
  children,
  className,
  ...props
}: ToolLayoutProps) => {
  return (
    <div
      className={cn(
        'min-h-[calc(100vh-3.5rem)] bg-background text-foreground',
        className
      )}
      {...props}
    >
      <div className="mx-auto w-full gap-6 px-5 py-6 lg:px-8">{children}</div>
    </div>
  )
}

export const ToolLayoutMain = ({
  children,
  className,
  ...props
}: ToolLayoutMainProps) => {
  return (
    <main className={cn('flex min-w-0 flex-col gap-6', className)} {...props}>
      {children}
    </main>
  )
}

export const ToolLayoutSidebar = ({
  children,
  className,
  isSticky = true,
  ...props
}: ToolLayoutSidebarProps) => {
  return (
    <aside className={cn('min-w-0', className)} {...props}>
      <Surface
        className={cn(
          'flex flex-col gap-4 rounded-(--radius) border border-border bg-surface p-4',
          isSticky && 'lg:sticky lg:top-20'
        )}
        variant="secondary"
      >
        {children}
      </Surface>
    </aside>
  )
}
