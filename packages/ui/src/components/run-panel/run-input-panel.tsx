'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Button, Input, Label, TextField } from '@heroui/react'

import { PlayIcon } from '../../icons'
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
        'rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface)] p-4',
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
        <Button onPress={onRun}>
          <PlayIcon size={16} />
          {runButtonText}
        </Button>
      </div>
    </section>
  )
}
