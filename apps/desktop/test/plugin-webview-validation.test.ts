import { describe, expect, it } from 'bun:test'
import { once } from 'node:events'
import { readFileSync } from 'node:fs'
import { createServer } from 'node:http'

import {
  assertOwnedWebviewCdp,
  getOwnedNativeDescendants,
  isValidationProcessRoot,
  probeLoopbackCdp,
  terminateNativeProcess,
  type NativeProcess,
} from '../../ui-test/scripts/plugin-webview-cdp'

const cargo = readFileSync(
  new URL('../src-tauri/Cargo.toml', import.meta.url),
  'utf8'
)
const source = readFileSync(
  new URL('../src-tauri/src/bin/validate-plugin-webviews.rs', import.meta.url),
  'utf8'
)
const config = JSON.parse(
  readFileSync(
    new URL('../tauri.plugin-webview-validation.conf.json', import.meta.url),
    'utf8'
  )
) as {
  identifier: string
  app: {
    windows: unknown[]
    security: { csp: string; capabilities: { permissions: unknown[] }[] }
  }
  bundle: { active: boolean }
}

describe('dedicated native plugin WebView validation entry', () => {
  it('requires a nondefault feature and never calls the production launcher', () => {
    expect(cargo).toContain('required-features = ["plugin-webview-validation"]')
    expect(cargo).not.toMatch(/^default\s*=.*plugin-webview-validation/m)
    expect(source).not.toContain('desktop_lib::run(')
    expect(source).not.toContain('.plugin(')
    expect(source).toContain('.visible(false)')
    expect(source).not.toContain('.show(')
    expect(source).not.toContain('.set_focus(')
    expect(source).toContain('generate_context!(capabilities = [])')
  })
  it('has its own identity and an empty native capability surface', () => {
    expect(config.identifier).toBe(
      'com.flowtools.g5-plugin-webviews-validation-20261012'
    )
    expect(config.app.windows).toEqual([])
    expect(
      config.app.security.capabilities.flatMap(value => value.permissions)
    ).toEqual([])
    expect(config.app.security.csp).toContain("default-src 'none'")
    expect(config.app.security.csp).not.toContain("'unsafe-inline'")
    expect(config.app.security.csp).not.toContain("'unsafe-eval'")
    expect(config.bundle.active).toBe(false)
  })
})

describe('native WebView CDP ownership validation', () => {
  it('refuses a reused root PID belonging to another instance of the same image', () => {
    const executable = 'C:\\owned\\validate-plugin-webviews.exe'
    const root = {
      ProcessId: 10,
      ParentProcessId: 1,
      CreationDate: '2026-10-31T00:00:00Z',
      ExecutablePath: executable,
      CommandLine: `"${executable}" --validation-instance=flowtools-plugin-webview-first`,
    }
    expect(
      isValidationProcessRoot(
        root,
        executable,
        'flowtools-plugin-webview-first'
      )
    ).toBe(true)
    expect(
      isValidationProcessRoot(
        root,
        executable,
        'flowtools-plugin-webview-second'
      )
    ).toBe(false)
    expect(
      isValidationProcessRoot(
        { ...root, ExecutablePath: 'C:\\other.exe' },
        executable,
        'flowtools-plugin-webview-first'
      )
    ).toBe(false)
    expect(
      isValidationProcessRoot(
        {
          ...root,
          CommandLine: `"${executable}" --validation-instance=flowtools-plugin-webview-first-suffix`,
        },
        executable,
        'flowtools-plugin-webview-first'
      )
    ).toBe(false)
    expect(
      isValidationProcessRoot(
        {
          ...root,
          CommandLine: `"${executable}" "--validation-instance=flowtools-plugin-webview-first"`,
        },
        executable,
        'flowtools-plugin-webview-first'
      )
    ).toBe(true)
  })
  it('uses bounded literal-loopback HTTP without browser bad-port filtering or redirect following', async () => {
    let status = 200
    const requested: string[] = []
    const server = createServer((request, response) => {
      requested.push(request.url ?? '')
      response.writeHead(status, { Location: '/forbidden-redirect' })
      response.end('{}')
    })
    server.listen(0, '127.0.0.1')
    await once(server, 'listening')
    const address = server.address()
    expect(address && typeof address === 'object').toBe(true)
    if (!address || typeof address === 'string')
      throw new Error('Loopback fixture')
    const originalFetch = globalThis.fetch
    globalThis.fetch = new Proxy(originalFetch, {
      apply() {
        throw new Error('Browser bad port')
      },
    })
    try {
      expect(await probeLoopbackCdp(address.port, 1000)).toBe(true)
      status = 302
      expect(await probeLoopbackCdp(address.port, 1000)).toBe(false)
      expect(requested).toEqual(['/json/version', '/json/version'])
    } finally {
      globalThis.fetch = originalFetch
      await new Promise<void>(resolve => server.close(() => resolve()))
    }
  })
  const processes: NativeProcess[] = [
    {
      ProcessId: 10,
      ParentProcessId: 1,
      CommandLine: 'validation.exe',
      CreationDate: '2026-10-13T12:00:00.1234560Z',
    },
    {
      ProcessId: 20,
      ParentProcessId: 10,
      CommandLine: 'msedgewebview2.exe --user-data-dir="C:\\Temp\\fixture\\a"',
      CreationDate: '2026-10-13T12:00:00.1234570Z',
    },
    {
      ProcessId: 30,
      ParentProcessId: 1,
      CommandLine: 'msedgewebview2.exe --user-data-dir="C:\\Temp\\fixture\\a"',
      CreationDate: '2026-10-13T12:00:00.1234580Z',
    },
  ]
  it('requires a present root and a valid birth chain before accepting descendants', () => {
    const grandchild = {
      ...processes[1]!,
      ProcessId: 40,
      ParentProcessId: 20,
      CreationDate: '2026-10-13T12:00:00.1234580Z',
    }
    expect(
      getOwnedNativeDescendants([grandchild, ...processes], 10).map(
        process => process.ProcessId
      )
    ).toEqual([40, 10, 20])
    expect(getOwnedNativeDescendants(processes.slice(1), 10)).toEqual([])
    expect(() =>
      getOwnedNativeDescendants([...processes, processes[0]!], 10)
    ).toThrow('Ambiguous native process PID')
  })
  it('refuses surviving old children after a parent PID is reused', () => {
    const oldChild = {
      ...processes[1]!,
      CreationDate: '2026-10-13T12:00:00.1234550Z',
    }
    const oldGrandchild = {
      ...processes[2]!,
      ParentProcessId: oldChild.ProcessId,
    }
    const snapshot = [processes[0]!, oldChild, oldGrandchild]
    expect(getOwnedNativeDescendants(snapshot, 10)).toEqual([processes[0]!])
    expect(() =>
      assertOwnedWebviewCdp(
        snapshot,
        [{ LocalAddress: '127.0.0.1', OwningProcess: 20 }],
        10,
        'C:\\Temp\\fixture\\a'
      )
    ).toThrow('Native CDP must belong to this validation process tree')
  })
  it('preserves microsecond order and refuses invalid creation dates', () => {
    expect(
      getOwnedNativeDescendants(
        [
          processes[0]!,
          {
            ...processes[1]!,
            CreationDate: '2026-10-13T20:00:00.123456+08:00',
          },
        ],
        10
      ).length
    ).toBe(2)
    for (const CreationDate of [
      'invalid',
      '2026-02-30T12:00:00.123456Z',
      '2026-10-13T24:00:00.123456Z',
      '2026-10-13T12:00:00.123456+14:01',
    ]) {
      expect(() =>
        getOwnedNativeDescendants(
          [{ ...processes[0]!, CreationDate }, processes[1]!],
          10
        )
      ).toThrow()
      expect(() =>
        getOwnedNativeDescendants(
          [processes[0]!, { ...processes[1]!, CreationDate }],
          10
        )
      ).toThrow()
    }
  })
  it('binds an actual process handle and refuses a mismatched birth without killing it', () => {
    const record = {
      ProcessId: process.pid,
      ParentProcessId: process.ppid,
      CommandLine: null,
      CreationDate: '0001-01-01T00:00:00.0000000Z',
    }
    if (process.platform === 'win32') {
      expect(terminateNativeProcess(record)).toBe(false)
      expect(process.pid).toBe(record.ProcessId)
      expect(() =>
        terminateNativeProcess({ ...record, CreationDate: 'invalid' })
      ).toThrow()
    } else {
      expect(() => terminateNativeProcess(record)).toThrow(
        'Windows process custody only'
      )
    }
  })
  it('returns false when the recorded process has already exited', () => {
    const record = {
      ProcessId: 2147483647,
      ParentProcessId: process.ppid,
      CommandLine: null,
      CreationDate: '0001-01-01T00:00:00.0000000Z',
    }
    if (process.platform === 'win32') {
      expect(terminateNativeProcess(record)).toBe(false)
    } else {
      expect(() => terminateNativeProcess(record)).toThrow(
        'Windows process custody only'
      )
    }
  })
  it('accepts only loopback listeners in the actual native child tree and profile', () => {
    expect(() =>
      assertOwnedWebviewCdp(
        processes,
        [{ LocalAddress: '127.0.0.1', OwningProcess: 20 }],
        10,
        'C:\\Temp\\fixture\\a'
      )
    ).not.toThrow()
    for (const listener of [
      { LocalAddress: '0.0.0.0', OwningProcess: 20 },
      { LocalAddress: '127.0.0.1', OwningProcess: 30 },
      { LocalAddress: '127.0.0.1', OwningProcess: 10 },
    ])
      expect(() =>
        assertOwnedWebviewCdp(processes, [listener], 10, 'C:\\Temp\\fixture\\a')
      ).toThrow()
    expect(() =>
      assertOwnedWebviewCdp(
        processes,
        [{ LocalAddress: '127.0.0.1', OwningProcess: 20 }],
        10,
        'C:\\Temp\\fixture\\a-other'
      )
    ).toThrow()
  })
})
