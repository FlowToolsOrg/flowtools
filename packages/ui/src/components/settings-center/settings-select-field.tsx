'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Description, Label, ListBox, Select } from '@heroui/react'

import { cn } from '../../utils/class-name'

import { type SettingsOption, type SettingsValueKey } from './types'

export interface SettingsSelectFieldProps extends Omit<
  ComponentPropsWithoutRef<'div'>,
  'onChange'
> {
  label: string
  description?: string
  value: SettingsValueKey | null
  options: SettingsOption[]
  placeholder?: string
  isDisabled?: boolean
  onChange?: (value: SettingsValueKey | null) => void
}

const normalizeSelectValue = (
  value: SettingsValueKey | SettingsValueKey[] | null
): SettingsValueKey | null => {
  if (Array.isArray(value)) {
    return value[0] ?? null
  }

  return value
}

export const SettingsSelectField = ({
  label,
  description,
  value,
  options,
  placeholder,
  isDisabled,
  onChange,
  className,
  ...props
}: SettingsSelectFieldProps) => {
  return (
    <div
      className={cn(
        'rounded-2xl border border-slate-200/80 bg-white px-4 py-3',
        className
      )}
      {...props}
    >
      <Select
        className="w-full"
        isDisabled={isDisabled}
        onChange={next => onChange?.(normalizeSelectValue(next))}
        placeholder={placeholder ?? 'Select one'}
        value={value}
      >
        <Label>{label}</Label>
        <Select.Trigger>
          <Select.Value />
          <Select.Indicator />
        </Select.Trigger>
        <Select.Popover>
          <ListBox>
            {options.map(option => (
              <ListBox.Item
                key={option.key}
                id={option.key}
                isDisabled={option.isDisabled}
                textValue={option.label}
              >
                <div className="flex flex-col gap-0.5">
                  <Label>{option.label}</Label>
                  {option.description ? (
                    <Description className="text-xs text-slate-500">
                      {option.description}
                    </Description>
                  ) : null}
                </div>
                <ListBox.ItemIndicator />
              </ListBox.Item>
            ))}
          </ListBox>
        </Select.Popover>
        {description ? (
          <Description className="text-xs text-slate-500">
            {description}
          </Description>
        ) : null}
      </Select>
    </div>
  )
}
