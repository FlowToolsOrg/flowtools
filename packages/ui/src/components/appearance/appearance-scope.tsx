'use client'

import type { ResolvedAppearance } from '@flowtools/sdk/extensions'

import { useMemo, type ReactNode } from 'react'

import { cn } from '../../utils/class-name'

import { appearanceStyle } from './appearance-style'

export interface AppearanceScopeProps {
  appearance: ResolvedAppearance
  label: string
  children: ReactNode
  className?: string
}

/** Host-owned subtree; portals must be mounted inside this scope separately. */
export function AppearanceScope({
  appearance,
  label,
  children,
  className,
}: AppearanceScopeProps) {
  const presentation = useMemo(() => appearanceStyle(appearance), [appearance])
  return (
    <section
      aria-label={label}
      className={cn('flowtools-appearance', className)}
      data-appearance-mode={presentation.mode}
      data-theme={presentation.mode}
      data-reduced-motion={presentation.reducedMotion || undefined}
      style={presentation.style}
    >
      {children}
    </section>
  )
}
