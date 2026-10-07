import assert from 'node:assert/strict'

export const loopbackCdpCandidates = [
  'http://127.0.0.1:9224',
  'http://[::1]:9224',
] as const

/** Validate every listener before attaching to the endpoint that actually replied. */
export function assertLoopbackCdpEndpoint(
  addresses: string,
  endpoint: string
): void {
  const listeners = addresses.trim().split(/\s+/)
  assert.ok(
    listeners.every(address => address === '127.0.0.1' || address === '::1'),
    'Native CDP must listen only on literal loopback addresses'
  )
  const urls = listeners.map(address =>
    address === '::1' ? 'http://[::1]:9224' : 'http://127.0.0.1:9224'
  )
  assert.ok(
    urls.some(url => url === endpoint),
    'CDP endpoint must match a verified listener'
  )
}
