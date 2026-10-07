import { expect, test } from 'bun:test'

import { assertLoopbackCdpEndpoint } from '../../ui-test/scripts/loopback-cdp'

test('native CDP accepts the actual IPv4, IPv6 or dual loopback listener', () => {
  expect(() =>
    assertLoopbackCdpEndpoint('127.0.0.1\r\n', 'http://127.0.0.1:9224')
  ).not.toThrow()
  expect(() =>
    assertLoopbackCdpEndpoint('::1\r\n', 'http://[::1]:9224')
  ).not.toThrow()
  for (const endpoint of ['http://127.0.0.1:9224', 'http://[::1]:9224'])
    expect(() =>
      assertLoopbackCdpEndpoint('127.0.0.1\r\n::1\r\n', endpoint)
    ).not.toThrow()
})

test('native CDP refuses public, wildcard, empty and mixed listeners', () => {
  for (const listeners of ['', '0.0.0.0', '::', '192.0.2.1', '::1\n0.0.0.0'])
    expect(() =>
      assertLoopbackCdpEndpoint(listeners, 'http://[::1]:9224')
    ).toThrow('only on literal loopback')
})

test('native CDP refuses endpoints unrelated to the observed literal listener', () => {
  for (const endpoint of [
    'http://127.0.0.1:9224',
    'http://localhost:9224',
    'http://[::1]:9225',
    'http://[::1]:9224/other',
    'http://192.0.2.1:9224',
    'http://user:pass@[::1]:9224',
  ])
    expect(() => assertLoopbackCdpEndpoint('::1', endpoint)).toThrow(
      'match a verified listener'
    )
})
