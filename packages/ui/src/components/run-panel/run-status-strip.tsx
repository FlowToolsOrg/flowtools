'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Button, Chip } from '@heroui/react'

import {
  CircleCheckIcon,
  ClockIcon,
  LoaderPinwheelIcon,
  RefreshCWIcon,
  XIcon,
} from '../../icons'
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

const statusIcons: Record<RunStatus, typeof ClockIcon> = {
  idle: ClockIcon,
  running: LoaderPinwheelIcon,
  success: CircleCheckIcon,
  error: XIcon,
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
        'flex flex-wrap items-center justify-between gap-3 rounded-(--radius) border border-(--border) bg-(--surface) px-4 py-2',
        className
      )}
      {...props}
    >
      <div className="flex items-center gap-2">
        <Chip color={statusColorMap[status]} size="sm" variant="soft">
          <span className="flex items-center gap-1">
            {(() => {
              const StatusIcon = statusIcons[status]
              return <StatusIcon size={12} />
            })()}
            {status}
          </span>
        </Chip>
        {message ? (
          <p className="text-sm text-(--foreground)">{message}</p>
        ) : null}
        {duration ? <p className="text-xs text-(--muted)">{duration}</p> : null}
      </div>
      {onReset ? (
        <Button onPress={onReset} size="sm" variant="ghost">
          <RefreshCWIcon size={14} />
          Reset
        </Button>
      ) : null}
    </section>
  )
}
