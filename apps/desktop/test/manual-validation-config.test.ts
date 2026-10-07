import { expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'

test('manual acceptance uses a visible test identity without production config overrides', () => {
  const manual: unknown = JSON.parse(
    readFileSync(
      new URL('../tauri.manual-validation.conf.json', import.meta.url),
      'utf8'
    )
  )
  expect(manual).toEqual({
    identifier: 'com.flowtools.manual-validation-20261004-r2',
    app: {
      windows: [
        {
          label: 'main',
          zoomHotkeysEnabled: true,
          title: 'FlowTools manual validation - test data only',
          url: '/?execution-validation=20261004-manual',
          width: 1200,
          height: 900,
          visible: true,
        },
      ],
    },
  })
})
