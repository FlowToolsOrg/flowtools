'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Button, Chip } from '@heroui/react'

import { cn } from '../../utils/class-name'

import { type RunStatus } from './types'

export interface RunStatusStripProps extends ComponentPropsWithoutRef<'section'> {
  status: RunStatus
  message?: string
  duration?: string
  onReset?: () => void
}

const statusColorMap: Record<
  RunStatus,
  'default' | 'accent' | 'success' | 'danger'
> = {
  idle: 'default',
  running: 'accent',
  success: 'success',
  error: 'danger',
}

export const RunStatusStrip = ({
  status,
  message,
  duration,
  onReset,
  className,
  ...props
}: RunStatusStripProps) => {
  return (
    <section
      className={cn(
        'flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-slate-200/80 bg-white px-4 py-2',
        className
      )}
      {...props}
    >
      <div className="flex items-center gap-2">
        <Chip color={statusColorMap[status]} size="sm" variant="soft">
          {status}
        </Chip>
        {message ? <p className="text-sm text-slate-700">{message}</p> : null}
        {duration ? <p className="text-xs text-slate-500">{duration}</p> : null}
      </div>
      {onReset ? (
        <Button onPress={onReset} size="sm" variant="ghost">
          Reset
        </Button>
      ) : null}
    </section>
  )
}
