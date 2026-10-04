'use client'

import {
  resolvePluginMaturity,
  type CompatibilityEvidenceStatus,
  type PluginMaturity,
} from '@flowtools/sdk/types'

import { Chip } from '@heroui/react'

const labels: Record<PluginMaturity, string> = {
  prototype: 'Prototype',
  experimental: 'Experimental',
  beta: 'Beta',
  production: 'Production',
}
const colors = {
  prototype: 'default',
  experimental: 'warning',
  beta: 'accent',
  production: 'success',
} as const

export interface PluginMaturityBadgeProps {
  maturity?: PluginMaturity
}
export function PluginMaturityBadge({ maturity }: PluginMaturityBadgeProps) {
  const value = resolvePluginMaturity(maturity)
  return (
    <Chip
      aria-label={`Plugin maturity: ${labels[value]}`}
      color={colors[value]}
      size="sm"
      variant="soft"
    >
      {labels[value]}
    </Chip>
  )
}

export interface PluginCompatibilityBadgeProps {
  evidence: CompatibilityEvidenceStatus
}
export function PluginCompatibilityBadge({
  evidence,
}: PluginCompatibilityBadgeProps) {
  return (
    <Chip
      aria-label={`Compatibility evidence: ${evidence}`}
      size="sm"
      variant="secondary"
      title="Evidence is separate from maturity and never grants execution or proves security."
    >
      {evidence}
    </Chip>
  )
}
