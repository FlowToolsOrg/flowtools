import { expect, test } from 'bun:test'
import { readFileSync, readdirSync } from 'node:fs'

const read = (path: string) =>
  readFileSync(new URL(path, import.meta.url), 'utf8')

test('shell acceptance uses a visible fixture identity without debugging or startup grants', () => {
  const config: unknown = JSON.parse(
    read('../tauri.shell-validation.conf.json')
  )
  expect(config).toEqual({
    identifier: 'com.flowtools.shell-validation-20261015',
    app: {
      windows: [
        {
          label: 'main',
          title: 'FlowTools shell validation - test data only',
          width: 1000,
          height: 720,
          visible: true,
          zoomHotkeysEnabled: true,
        },
      ],
    },
  })
  // These controls belong to native Host UI, never webview/plugin code.
  for (const file of readdirSync(
    new URL('../src-tauri/capabilities/', import.meta.url)
  )) {
    expect(read(`../src-tauri/capabilities/${file}`)).not.toMatch(
      /autostart:|global-shortcut:|core:(?:default|menu:|tray:)/
    )
  }
})
