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
        'rounded-3xl border border-slate-200/80 bg-white p-4',
        className
      )}
      {...props}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1">
          <h3 className="text-sm font-semibold text-slate-900">
            {result.title}
          </h3>
          {result.summary ? (
            <p className="text-xs text-slate-500">{result.summary}</p>
          ) : null}
        </div>
        {onCopy ? (
          <Button onPress={onCopy} size="sm" variant="ghost">
            Copy
          </Button>
        ) : null}
      </div>
      {result.raw ? (
        <pre className="mt-3 overflow-x-auto rounded-2xl bg-slate-900 p-3 text-xs text-slate-100">
          {result.raw}
        </pre>
      ) : null}
    </section>
  )
}
