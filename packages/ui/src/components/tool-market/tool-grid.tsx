'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { cn } from '../../utils/class-name'

export interface ToolGridRootProps extends ComponentPropsWithoutRef<'div'> {}

export interface ToolGridItemProps extends ComponentPropsWithoutRef<'article'> {}

const ToolGridRoot = ({ children, className, ...props }: ToolGridRootProps) => {
  return (
    <div
      className={cn(
        'grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3',
        className
      )}
      {...props}
    >
      {children}
    </div>
  )
}

const ToolGridItem = ({ children, className, ...props }: ToolGridItemProps) => {
  return (
    <article className={cn('min-w-0', className)} {...props}>
      {children}
    </article>
  )
}

export const ToolGrid = Object.assign(ToolGridRoot, {
  Root: ToolGridRoot,
  Item: ToolGridItem,
})
