'use client'

import {
  type ComponentPropsWithoutRef,
  type HTMLInputTypeAttribute,
} from 'react'

import { Description, Input, Label, TextField } from '@heroui/react'

import { cn } from '../../utils/class-name'

export interface SettingsInputFieldProps extends Omit<
  ComponentPropsWithoutRef<'div'>,
  'onChange'
> {
  label: string
  value: string
  description?: string
  placeholder?: string
  type?: HTMLInputTypeAttribute
  isDisabled?: boolean
  onChange?: (value: string) => void
}

export const SettingsInputField = ({
  label,
  value,
  description,
  placeholder,
  type = 'text',
  isDisabled,
  onChange,
  className,
  ...props
}: SettingsInputFieldProps) => {
  return (
    <div
      className={cn(
        'rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface)] px-4 py-3',
        className
      )}
      {...props}
    >
      <TextField
        isDisabled={isDisabled}
        onChange={onChange}
        type={type}
        value={value}
      >
        <Label>{label}</Label>
        <Input placeholder={placeholder} variant="secondary" />
        {description ? (
          <Description className="text-xs text-[var(--muted)]">
            {description}
          </Description>
        ) : null}
      </TextField>
    </div>
  )
}
