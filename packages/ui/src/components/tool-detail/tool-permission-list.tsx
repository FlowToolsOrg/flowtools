'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Chip } from '@heroui/react'

import { LockIcon, ShieldCheckIcon } from '../../icons'
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
        'flex items-start justify-between gap-3 rounded-(--radius) border border-(--border) bg-(--surface) px-4 py-3',
        onPress && 'cursor-pointer transition hover:bg-(--surface-secondary)',
        className
      )}
      onClick={onPress}
      role={onPress ? 'button' : undefined}
      tabIndex={onPress ? 0 : undefined}
      data-permission-id={permissionId}
      {...props}
    >
      <div className="space-y-1">
        <p className="text-sm font-medium text-(--foreground)">{name}</p>
        {description ? (
          <p className="text-xs text-(--muted)">{description}</p>
        ) : null}
      </div>
      {required ? (
        <Chip color="warning" size="sm" variant="soft">
          <span className="flex items-center gap-1">
            <ShieldCheckIcon size={12} />
            Required
          </span>
        </Chip>
      ) : (
        <Chip size="sm" variant="tertiary">
          <span className="flex items-center gap-1">
            <LockIcon size={12} />
            Optional
          </span>
        </Chip>
      )}
    </li>
  )
}

export const ToolPermissionList = Object.assign(ToolPermissionListRoot, {
  Root: ToolPermissionListRoot,
  Item: ToolPermissionListItem,
})
