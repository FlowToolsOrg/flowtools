'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Chip } from '@heroui/react'

import { cn } from '../../utils/class-name'

export interface ToolPermissionListRootProps extends ComponentPropsWithoutRef<'ul'> {}

export interface ToolPermissionListItemProps extends Omit<
  ComponentPropsWithoutRef<'li'>,
  'onClick'
> {
  permissionId: string
  name: string
  description?: string
  required?: boolean
  onPress?: () => void
}

const ToolPermissionListRoot = ({
  children,
  className,
  ...props
}: ToolPermissionListRootProps) => {
  return (
    <ul className={cn('space-y-2', className)} {...props}>
      {children}
    </ul>
  )
}

const ToolPermissionListItem = ({
  permissionId,
  name,
  description,
  required,
  onPress,
  className,
  ...props
}: ToolPermissionListItemProps) => {
  return (
    <li
      className={cn(
        'flex items-start justify-between gap-3 rounded-2xl border border-slate-200/80 bg-white px-4 py-3',
        onPress && 'cursor-pointer transition hover:bg-slate-50',
        className
      )}
      onClick={onPress}
      role={onPress ? 'button' : undefined}
      tabIndex={onPress ? 0 : undefined}
      data-permission-id={permissionId}
      {...props}
    >
      <div className="space-y-1">
        <p className="text-sm font-medium text-slate-900">{name}</p>
        {description ? (
          <p className="text-xs text-slate-500">{description}</p>
        ) : null}
      </div>
      {required ? (
        <Chip color="warning" size="sm" variant="soft">
          Required
        </Chip>
      ) : (
        <Chip size="sm" variant="tertiary">
          Optional
        </Chip>
      )}
    </li>
  )
}

export const ToolPermissionList = Object.assign(ToolPermissionListRoot, {
  Root: ToolPermissionListRoot,
  Item: ToolPermissionListItem,
})
