'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Description, Label, Switch } from '@heroui/react'

import { cn } from '../../utils/class-name'

export interface SettingsSwitchFieldProps extends Omit<
  ComponentPropsWithoutRef<'div'>,
  'onChange'
> {
  label: string
  description?: string
  isSelected: boolean
  isDisabled?: boolean
  onChange?: (value: boolean) => void
}

export const SettingsSwitchField = ({
  label,
  description,
  isSelected,
  isDisabled,
  onChange,
  className,
  ...props
}: SettingsSwitchFieldProps) => {
  return (
    <div
      className={cn(
        'rounded-(--radius) border border-border bg-surface px-4 py-3',
        className
      )}
      {...props}
    >
      <Switch
        isDisabled={isDisabled}
        isSelected={isSelected}
        onChange={onChange}
      >
        <Switch.Control>
          <Switch.Thumb />
        </Switch.Control>
        <Switch.Content>
          <Label className="text-sm font-medium text-foreground">{label}</Label>
          {description ? (
            <Description className="text-xs text-muted">
              {description}
            </Description>
          ) : null}
        </Switch.Content>
      </Switch>
    </div>
  )
}
