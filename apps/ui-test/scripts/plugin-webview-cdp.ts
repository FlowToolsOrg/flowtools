import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { get } from 'node:http'
import { win32 } from 'node:path'

export type NativeProcess = {
  ProcessId: number
  ParentProcessId: number
  CommandLine: string | null
  CreationDate: string
}
export type NativeListener = { LocalAddress: string; OwningProcess: number }

// CDP is a loopback control endpoint, not browser navigation. Node fetch applies
// the browser bad-port list (including OS-selected port 6697); use raw HTTP.
export function probeLoopbackCdp(
  port: number,
  timeout: number
): Promise<boolean> {
  assert.ok(Number.isInteger(port) && port > 0 && port <= 65535)
  assert.ok(Number.isInteger(timeout) && timeout > 0)
  return new Promise((resolve, reject) => {
    const request = get(
      { hostname: '127.0.0.1', port, path: '/json/version', timeout },
      response => {
        response.resume()
        resolve(response.statusCode === 200)
      }
    )
    request.on('error', reject)
    request.on('timeout', () =>
      request.destroy(new Error('CDP readiness timeout'))
    )
  })
}

function profilePath(path: string): string {
  return win32.normalize(path.replace(/^\\\\\?\\/, '')).toLowerCase()
}

// CIM timestamps have microsecond precision. Date.parse alone discards the
// sub-millisecond part and accepts some invalid calendar dates by normalizing.
function creationMicroseconds(value: string): bigint {
  const match =
    /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,7}))?(Z|[+-]\d{2}:\d{2})$/.exec(
      value
    )
  assert.ok(match, 'A native process needs an ISO creation timestamp')
  const [year, month, day, hour, minute, second] = match
    .slice(1, 7)
    .map(Number) as [number, number, number, number, number, number]
  assert.ok(year >= 1 && month >= 1 && month <= 12)
  assert.ok(day >= 1 && day <= 31 && hour < 24 && minute < 60 && second < 60)
  const calendar = new Date(0)
  calendar.setUTCFullYear(year, month - 1, day)
  calendar.setUTCHours(hour, minute, second, 0)
  assert.ok(
    calendar.getUTCFullYear() === year &&
      calendar.getUTCMonth() === month - 1 &&
      calendar.getUTCDate() === day,
    'A native process needs a valid calendar creation date'
  )
  const zone = match[8]!
  let offsetMinutes = 0
  if (zone !== 'Z') {
    const hours = Number(zone.slice(1, 3))
    const minutes = Number(zone.slice(4, 6))
    assert.ok(hours <= 14 && minutes < 60 && (hours !== 14 || minutes === 0))
    offsetMinutes = (hours * 60 + minutes) * (zone[0] === '+' ? 1 : -1)
  }
  return (
    BigInt(calendar.getTime() - offsetMinutes * 60000) * 1000n +
    BigInt((match[7] ?? '').padEnd(6, '0').slice(0, 6))
  )
}

export function getOwnedNativeDescendants(
  processes: NativeProcess[],
  nativePid: number
): NativeProcess[] {
  const byPid = new Map<number, NativeProcess>()
  for (const process of processes) {
    assert.ok(!byPid.has(process.ProcessId), 'Ambiguous native process PID')
    byPid.set(process.ProcessId, process)
  }
  const root = byPid.get(nativePid)
  if (!root) return []
  const owned = new Map([[nativePid, creationMicroseconds(root.CreationDate)]])
  for (let depth = 0; depth < processes.length; depth++) {
    for (const process of processes) {
      if (owned.has(process.ProcessId)) continue
      const parentBirth = owned.get(process.ParentProcessId)
      if (parentBirth === undefined) continue
      const birth = creationMicroseconds(process.CreationDate)
      // A surviving old child can name a PID now reused by this native parent.
      // Refuse that edge and do not infer custody of its descendants.
      if (birth >= parentBirth) owned.set(process.ProcessId, birth)
    }
  }
  return processes.filter(process => owned.has(process.ProcessId))
}

export function terminateNativeProcess(record: NativeProcess): boolean {
  assert.equal(process.platform, 'win32', 'Windows process custody only')
  assert.ok(
    Number.isInteger(record.ProcessId) &&
      record.ProcessId > 0 &&
      record.ProcessId <= 2147483647,
    'A native process needs a positive Windows PID'
  )
  const expected =
    creationMicroseconds(record.CreationDate) + 62135596800000000n
  const script = `
$ErrorActionPreference = 'Stop'
$owned = $null
try {
  try { $owned = Get-Process -Id ${record.ProcessId} -ErrorAction Stop }
  catch {
    if ($_.FullyQualifiedErrorId -eq 'NoProcessFoundForGivenId,Microsoft.PowerShell.Commands.GetProcessCommand') {
      [Console]::Out.Write('false')
      return
    }
    throw
  }
  try {
    # Cache the process handle before inspecting its birth time. Kill then uses
    # this bound Process object, never a fresh PID lookup or recursive tree kill.
    $null = $owned.Handle
    if ($owned.HasExited) { [Console]::Out.Write('false'); return }
    [long]$remainder = 0
    $actual = [Math]::DivRem([long]$owned.StartTime.ToUniversalTime().Ticks, [long]10, [ref]$remainder)
    if ($actual -ne [long]'${expected}') { [Console]::Out.Write('false'); return }
    $owned.Kill()
    [Console]::Out.Write('true')
  } catch {
    $exception = $_.Exception.GetBaseException()
    $exitRace = $exception -is [InvalidOperationException] -or
      ($exception -is [ComponentModel.Win32Exception] -and $exception.NativeErrorCode -in 5,6,87,1168)
    if ($exitRace -and $owned.HasExited) { [Console]::Out.Write('false'); return }
    throw
  }
} finally {
  if ($null -ne $owned) { $owned.Dispose() }
}
`
  const result = execFileSync(
    'pwsh',
    ['-NoProfile', '-NonInteractive', '-Command', script],
    { windowsHide: true, encoding: 'utf8', timeout: 5000 }
  ).trim()
  assert.ok(
    result === 'true' || result === 'false',
    'Native termination result'
  )
  return result === 'true'
}

export function assertOwnedWebviewCdp(
  processes: NativeProcess[],
  listeners: NativeListener[],
  nativePid: number,
  expectedProfile: string
): void {
  const descendants = new Set(
    getOwnedNativeDescendants(processes, nativePid).map(
      process => process.ProcessId
    )
  )
  assert.ok(descendants.has(nativePid), 'Native validation process is required')
  assert.ok(listeners.length > 0, 'Native CDP listener is required')
  for (const listener of listeners) {
    assert.ok(
      listener.LocalAddress === '127.0.0.1' || listener.LocalAddress === '::1',
      'Every native CDP listener must use literal loopback'
    )
    assert.ok(
      descendants.has(listener.OwningProcess) &&
        listener.OwningProcess !== nativePid,
      'Native CDP must belong to this validation process tree'
    )
    const process = processes.find(
      process => process.ProcessId === listener.OwningProcess
    )
    const profile = process?.CommandLine?.match(
      /--user-data-dir(?:=|\s+)(?:"([^"]+)"|([^\s]+))/
    )
    assert.ok(profile, 'Native browser must declare its actual profile')
    assert.equal(
      profilePath(profile[1] ?? profile[2]!),
      profilePath(expectedProfile)
    )
  }
  assert.ok(listeners.some(listener => listener.LocalAddress === '127.0.0.1'))
}
