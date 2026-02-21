'use client'

import { Description, Label, Switch } from '@heroui/react'
import { type ComponentPropsWithoutRef } from 'react'

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
        'rounded-2xl border border-slate-200/80 bg-white px-4 py-3',
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
          <Label className="text-sm font-medium text-slate-900">{label}</Label>
          {description ? (
            <Description className="text-xs text-slate-500">
              {description}
            </Description>
          ) : null}
        </Switch.Content>
      </Switch>
    </div>
  )
}
