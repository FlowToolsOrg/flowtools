'use client'

import { type ComponentProps, type ComponentPropsWithoutRef } from 'react'

import { Card } from '@heroui/react'

import { cn } from '../../utils/class-name'

export interface SettingsGroupCardRootProps extends ComponentProps<
  typeof Card
> {}

export interface SettingsGroupCardHeaderProps extends ComponentPropsWithoutRef<'header'> {}

export interface SettingsGroupCardTitleProps extends ComponentPropsWithoutRef<'h3'> {}

export interface SettingsGroupCardDescriptionProps extends ComponentPropsWithoutRef<'p'> {}

export interface SettingsGroupCardContentProps extends ComponentPropsWithoutRef<'div'> {}

const SettingsGroupCardRoot = ({
  children,
  className,
  ...props
}: SettingsGroupCardRootProps) => {
  return (
    <Card
      className={cn(
        'rounded-3xl border border-slate-200/80 bg-white/90 shadow-sm',
        className
      )}
      variant="secondary"
      {...props}
    >
      {children}
    </Card>
  )
}

const SettingsGroupCardHeader = ({
  children,
  className,
  ...props
}: SettingsGroupCardHeaderProps) => {
  return (
    <header className={cn('space-y-1 px-5 pt-5', className)} {...props}>
      {children}
    </header>
  )
}

const SettingsGroupCardTitle = ({
  children,
  className,
  ...props
}: SettingsGroupCardTitleProps) => {
  return (
    <h3
      className={cn('text-base font-semibold text-slate-900', className)}
      {...props}
    >
      {children}
    </h3>
  )
}

const SettingsGroupCardDescription = ({
  children,
  className,
  ...props
}: SettingsGroupCardDescriptionProps) => {
  return (
    <p className={cn('text-sm text-slate-500', className)} {...props}>
      {children}
    </p>
  )
}

const SettingsGroupCardContent = ({
  children,
  className,
  ...props
}: SettingsGroupCardContentProps) => {
  return (
    <div className={cn('space-y-3 p-5 pt-4', className)} {...props}>
      {children}
    </div>
  )
}

export const SettingsGroupCard = Object.assign(SettingsGroupCardRoot, {
  Root: SettingsGroupCardRoot,
  Header: SettingsGroupCardHeader,
  Title: SettingsGroupCardTitle,
  Description: SettingsGroupCardDescription,
  Content: SettingsGroupCardContent,
})
