'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { cn } from '../../utils/class-name'

export interface SettingsItemRootProps extends Omit<
  ComponentPropsWithoutRef<'div'>,
  'onClick'
> {
  onPress?: () => void
}

export interface SettingsItemLabelProps extends ComponentPropsWithoutRef<'p'> {}

export interface SettingsItemDescriptionProps extends ComponentPropsWithoutRef<'p'> {}

export interface SettingsItemControlProps extends ComponentPropsWithoutRef<'div'> {}

const SettingsItemRoot = ({
  children,
  className,
  onPress,
  ...props
}: SettingsItemRootProps) => {
  const classes = cn(
    'flex w-full items-center justify-between gap-4 rounded-2xl border border-slate-200/80 bg-white px-4 py-3 text-left',
    onPress && 'transition hover:bg-slate-50',
    className
  )

  if (onPress) {
    return (
      <div
        className={classes}
        onClick={onPress}
        onKeyDown={event => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault()
            onPress()
          }
        }}
        role="button"
        tabIndex={0}
        {...props}
      >
        {children}
      </div>
    )
  }

  return (
    <div className={classes} {...props}>
      {children}
    </div>
  )
}

const SettingsItemLabel = ({
  children,
  className,
  ...props
}: SettingsItemLabelProps) => {
  return (
    <p
      className={cn('text-sm font-medium text-slate-900', className)}
      {...props}
    >
      {children}
    </p>
  )
}

const SettingsItemDescription = ({
  children,
  className,
  ...props
}: SettingsItemDescriptionProps) => {
  return (
    <p className={cn('text-xs text-slate-500', className)} {...props}>
      {children}
    </p>
  )
}

const SettingsItemControl = ({
  children,
  className,
  ...props
}: SettingsItemControlProps) => {
  return (
    <div className={cn('shrink-0', className)} {...props}>
      {children}
    </div>
  )
}

export const SettingsItem = Object.assign(SettingsItemRoot, {
  Root: SettingsItemRoot,
  Label: SettingsItemLabel,
  Description: SettingsItemDescription,
  Control: SettingsItemControl,
})
