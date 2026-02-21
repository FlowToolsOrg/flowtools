'use client'

import { Surface } from '@heroui/react'
import { type ComponentPropsWithoutRef, type ReactNode } from 'react'

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
        'relative min-h-screen overflow-hidden bg-gradient-to-b from-amber-50 via-white to-slate-100 text-slate-900',
        className
      )}
      {...props}
    >
      <div className="pointer-events-none absolute inset-x-0 top-0 h-64 bg-[radial-gradient(circle_at_top,rgba(251,191,36,0.2),transparent_70%)]" />
      <div className="relative mx-auto grid w-full max-w-7xl gap-6 px-4 py-8 lg:grid-cols-[minmax(0,1fr)_22rem] lg:px-8">
        {children}
      </div>
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
          'flex flex-col gap-4 rounded-3xl border border-slate-200/80 bg-white/90 p-4 shadow-sm backdrop-blur',
          isSticky && 'lg:sticky lg:top-8'
        )}
        variant="secondary"
      >
        {children}
      </Surface>
    </aside>
  )
}
