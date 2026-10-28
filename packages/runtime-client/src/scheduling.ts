import type { ScheduleDefinition, ScheduleRecord } from './bindings'

import { SCHEDULE_LIMITS } from './bindings'

export function validScheduleDefinition(value: ScheduleDefinition): boolean {
  const period = value.intervalMs
  return (
    Number.isSafeInteger(value.firstDueMs) &&
    value.firstDueMs >= 0 &&
    value.firstDueMs <= SCHEDULE_LIMITS.maxTimestampMs &&
    Number.isInteger(value.occurrences) &&
    value.occurrences >= 1 &&
    value.occurrences <= SCHEDULE_LIMITS.maxOccurrences &&
    Number.isInteger(value.graceMs) &&
    value.graceMs >= 0 &&
    value.graceMs <= SCHEDULE_LIMITS.maxGraceMs &&
    (value.missed === 'skip' || value.missed === 'coalesce-latest') &&
    (period === null
      ? value.occurrences === 1
      : Number.isSafeInteger(period) &&
        period >= SCHEDULE_LIMITS.minIntervalMs &&
        Number.isSafeInteger(
          value.firstDueMs + period * (value.occurrences - 1)
        ) &&
        value.firstDueMs + period * (value.occurrences - 1) <=
          SCHEDULE_LIMITS.maxTimestampMs)
  )
}

export function validScheduleRecord(value: ScheduleRecord): boolean {
  const uuid = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/
  return (
    uuid.test(value.id) &&
    value.revision > 0 &&
    validScheduleDefinition(value.definition) &&
    value.nextIndex <= value.definition.occurrences &&
    value.skipped <= value.nextIndex &&
    (value.binding.identity.caller === 'local-cli' ||
      value.binding.identity.caller === 'local-desktop') &&
    /^[a-f0-9]{64}$/.test(value.binding.inputDigest) &&
    /^[a-f0-9]{64}$/.test(value.binding.identity.packageDigest) &&
    value.binding.timeoutMs > 0 &&
    value.binding.timeoutMs <= 300000 &&
    value.binding.grantEpoch > 0 &&
    (value.lastObservedMs === null ||
      (Number.isSafeInteger(value.lastObservedMs) &&
        value.lastObservedMs >= 0 &&
        value.lastObservedMs <= SCHEDULE_LIMITS.maxTimestampMs)) &&
    (value.lastClaim === null ||
      (uuid.test(value.lastClaim.token) &&
        (value.lastClaim.result.state !== 'submitted' ||
          uuid.test(value.lastClaim.result.runId)) &&
        value.lastClaim.index < value.nextIndex &&
        value.lastClaim.dueMs ===
          value.definition.firstDueMs +
            (value.definition.intervalMs ?? 0) * value.lastClaim.index))
  )
}
