'use client'

import { Button, Input, Label, TextField } from '@heroui/react'
import { type ComponentPropsWithoutRef } from 'react'

import { cn } from '../../utils/class-name'

export interface RunInputPanelProps extends Omit<
  ComponentPropsWithoutRef<'section'>,
  'onChange'
> {
  label?: string
  value: string
  placeholder?: string
  onChange?: (value: string) => void
  onRun?: () => void
  runButtonText?: string
}

export const RunInputPanel = ({
  label = 'Input',
  value,
  placeholder,
  onChange,
  onRun,
  runButtonText = 'Run',
  className,
  ...props
}: RunInputPanelProps) => {
  return (
    <section
      className={cn(
        'rounded-3xl border border-slate-200/80 bg-white p-4',
        className
      )}
      {...props}
    >
      <div className="flex flex-col gap-3 md:flex-row md:items-end">
        <TextField className="flex-1" onChange={onChange} value={value}>
          <Label>{label}</Label>
          <Input
            placeholder={placeholder ?? 'Type command input...'}
            variant="secondary"
          />
        </TextField>
        <Button onPress={onRun}>{runButtonText}</Button>
      </div>
    </section>
  )
}
