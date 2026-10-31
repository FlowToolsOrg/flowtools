import type { ErrorCode, RuntimeStatus } from '@flowtools/runtime-client'

import {
  describeRuntimeError,
  RuntimeClientError,
} from '@flowtools/runtime-client'
import { dependencyVersionSchema } from '@flowtools/sdk/dependencies'

export interface DesktopDiagnosticVersions {
  appVersion: string
  tauriVersion: string
}

export type RuntimeDiagnosticObservation =
  | { state: 'not-queried' }
  | { state: 'observed'; checkedAt: string; status: RuntimeStatus }
  | { state: 'unconfirmed'; checkedAt: string; code: ErrorCode }

const modes = new Set(['managed', 'management', 'validation'])

function timestamp(value: string) {
  if (value.length !== 24 || new Date(value).toISOString() !== value)
    throw new Error('DIAGNOSTIC_INVALID')
  return value
}

function version(value: string) {
  if (value.length > 64 || !dependencyVersionSchema.safeParse(value).success)
    throw new Error('DIAGNOSTIC_INVALID')
  return value
}

function project(observation: RuntimeDiagnosticObservation) {
  switch (observation.state) {
    case 'not-queried':
      return { state: 'not-queried' as const }
    case 'unconfirmed': {
      // Reuse the public finite error vocabulary; never copy exception text.
      const { code } = describeRuntimeError(
        new RuntimeClientError(observation.code)
      )
      return {
        state: 'unconfirmed' as const,
        checkedAt: timestamp(observation.checkedAt),
        code,
      }
    }
    case 'observed': {
      const { status } = observation
      if (
        typeof status.instanceId !== 'string' ||
        status.instanceId.length > 128 ||
        !/^[A-Za-z0-9][A-Za-z0-9-]*$/.test(status.instanceId) ||
        ![status.jobs, status.activeJobs].every(
          value => Number.isInteger(value) && value >= 0 && value <= 0xffffffff
        ) ||
        status.activeJobs > status.jobs
      )
        throw new Error('DIAGNOSTIC_INVALID')
      return {
        state: 'observed' as const,
        checkedAt: timestamp(observation.checkedAt),
        status: {
          instanceId: status.instanceId,
          mode: modes.has(status.mode) ? status.mode : 'unknown',
          jobs: status.jobs,
          activeJobs: status.activeJobs,
        },
      }
    }
    default:
      throw new Error('DIAGNOSTIC_INVALID')
  }
}

/** Host diagnostic projection only; not a wire DTO, grant or security receipt. */
export function createRuntimeStatusDiagnostic(
  versions: DesktopDiagnosticVersions,
  observation: RuntimeDiagnosticObservation,
  generatedAt: string
): string {
  try {
    const text = JSON.stringify(
      {
        formatVersion: 1,
        scope: 'desktop-runtime-status',
        maturity: 'prototype',
        generatedAt: timestamp(generatedAt),
        desktop: {
          appVersion: version(versions.appVersion),
          tauriVersion: version(versions.tauriVersion),
        },
        observation: project(observation),
      },
      null,
      2
    )
    if (new TextEncoder().encode(text).length > 4096)
      throw new Error('DIAGNOSTIC_INVALID')
    return text
  } catch {
    throw new Error('DIAGNOSTIC_INVALID')
  }
}
