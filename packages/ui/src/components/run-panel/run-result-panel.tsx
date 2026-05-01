'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Button } from '@heroui/react'

import { cn } from '../../utils/class-name'

import { type RunResultPayload } from './types'

export interface RunResultPanelProps extends ComponentPropsWithoutRef<'section'> {
  result: RunResultPayload
  onCopy?: () => void
}

export const RunResultPanel = ({
  result,
  onCopy,
  className,
  ...props
}: RunResultPanelProps) => {
  return (
    <section
      className={cn(
        'rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface)] p-4',
        className
      )}
      {...props}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1">
          <h3 className="text-sm font-semibold text-[var(--foreground)]">
            {result.title}
          </h3>
          {result.summary ? (
            <p className="text-xs text-[var(--muted)]">{result.summary}</p>
          ) : null}
        </div>
        {onCopy ? (
          <Button onPress={onCopy} size="sm" variant="ghost">
            Copy
          </Button>
        ) : null}
      </div>
      {result.raw ? (
        <pre className="mt-3 overflow-x-auto rounded-[var(--radius)] bg-[var(--surface-tertiary)] p-3 text-xs text-[var(--foreground)]">
          {result.raw}
        </pre>
      ) : null}
    </section>
  )
}
