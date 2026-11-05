import { expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'

interface Capability {
  windows: string[]
  remote?: unknown
  permissions: (string | { identifier: string; allow?: unknown })[]
}
const read = (name: string) =>
  readFileSync(new URL(name, import.meta.url), 'utf8')

test('production capability has no default sets, remote origins or validation authority', () => {
  const cap = JSON.parse(
    read('../src-tauri/capabilities/default.json')
  ) as Capability
  expect(cap.windows).toEqual(['main'])
  expect(cap.remote).toBeUndefined()
  const permissions = cap.permissions.map(p =>
    typeof p === 'string' ? p : p.identifier
  )
  expect(permissions.length).toBeGreaterThan(0)
  expect(new Set(permissions).size).toBe(permissions.length)
  for (const permission of permissions) {
    expect(permission).not.toMatch(
      /:default$|validation|sql:|clipboard-manager:|autostart:|global-shortcut:|core:(?:menu|tray):/
    )
  }
  expect(
    cap.permissions.find(
      p => typeof p !== 'string' && p.identifier === 'opener:allow-open-url'
    )
  ).toEqual({
    identifier: 'opener:allow-open-url',
    allow: [{ url: 'https://*' }, { url: 'http://*' }],
  })
  expect(permissions).toContain('fs:deny-default')
  expect(permissions).not.toContain('fs:scope-app-recursive')
})

test('the explicit validation overlay grants no production or plugin capabilities', () => {
  const config = JSON.parse(read('../tauri.runtime-validation.conf.json')) as {
    identifier: string
    app: { security: { capabilities: (string | Capability)[] } }
  }
  expect(config.identifier).toBe('com.flowtools.g2-validation-20261004')
  expect(config.app.security.capabilities[0]).toBe('default')
  const extra = config.app.security.capabilities.slice(1) as Capability[]
  expect(extra).toHaveLength(1)
  for (const cap of extra) {
    expect(cap.windows).toEqual(['main'])
    expect(cap.remote).toBeUndefined()
    expect(cap.permissions).toEqual([
      'allow-validation-runtime',
      'allow-validation-runtime-disconnect',
    ])
  }
})
