import type * as LaunchApi from '../src/runtime/development-html-launch'
import type * as BridgeApi from '../src/runtime/development-html-plugin-bridge'
import type { IndexedCommand } from '../src/runtime/html-command-types'

import { expect, test } from 'bun:test'
import { mkdirSync, mkdtempSync, realpathSync, rmSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const temporaryParent = resolve(import.meta.dir, '../node_modules/.tmp')
const command: IndexedCommand = {
  id: 'fixture:open',
  pluginId: 'fixture',
  pluginName: 'Controlled fixture',
  pluginType: 'app',
  title: 'Controlled fixture',
  type: 'open',
  source: 'html',
  compatibilityLevel: 'preload-bridge',
  permissions: [],
  main: 'https://fixture.invalid/index.html',
  mainAvailable: true,
  hasUi: true,
  hasPreload: true,
  requiresNative: false,
  preload: 'preload.js',
}

const modes = [
  { name: 'production', env: { DEV: false }, allowed: false },
  {
    name: 'production opt-in',
    env: { DEV: false, VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW: '1' },
    allowed: false,
  },
  { name: 'default dev', env: { DEV: true }, allowed: false },
  {
    name: 'dev disabled',
    env: { DEV: true, VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW: '0' },
    allowed: false,
  },
  {
    name: 'dev boolean',
    env: { DEV: true, VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW: true },
    allowed: false,
  },
  {
    name: 'explicit dev preview',
    env: { DEV: true, VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW: '1' },
    allowed: true,
  },
]

async function assertRejected(
  operation: Promise<unknown>,
  expected: string | { code: string }
) {
  await operation.then(
    () => {
      throw new Error('Expected HTML operation rejection')
    },
    (error: unknown) => {
      if (typeof expected === 'string') {
        expect(error).toBeInstanceOf(Error)
        expect((error as Error).message).toContain(expected)
      } else expect(error).toMatchObject(expected)
    }
  )
}

for (const mode of modes) {
  test(`actual HTML runner/bridge build: ${mode.name}`, async () => {
    mkdirSync(temporaryParent, { recursive: true })
    const temporary = mkdtempSync(join(temporaryParent, 'html-mode-'))
    try {
      const compiler = Bun.spawn(
        [
          process.execPath,
          resolve(import.meta.dir, 'html-mode-build.ts'),
          temporary,
          JSON.stringify(mode.env),
        ],
        {
          cwd: resolve(import.meta.dir, '..'),
          stdout: 'pipe',
          stderr: 'pipe',
        }
      )
      const [exitCode, stdout, stderr] = await Promise.all([
        compiler.exited,
        new Response(compiler.stdout).text(),
        new Response(compiler.stderr).text(),
      ])
      expect({ exitCode, stdout, stderr }).toEqual({
        exitCode: 0,
        stdout: '',
        stderr: '',
      })
      const launch = (await import(
        pathToFileURL(join(temporary, 'development-html-launch.js')).href
      )) as typeof LaunchApi
      const bridge = (await import(
        pathToFileURL(join(temporary, 'development-html-plugin-bridge.js')).href
      )) as typeof BridgeApi
      let requests = 0
      let payloadReads = 0
      const request: typeof fetch = Object.assign(
        async () => {
          requests++
          return new Response(
            '<html><head></head><body>controlled fixture</body></html>'
          )
        },
        { preconnect: () => {} }
      )
      const context = {
        env: {
          pluginId: 'fixture',
          pluginType: 'app' as const,
          platform: 'desktop' as const,
          mode: 'production' as const,
        },
        ui: {
          toast: () => {
            payloadReads++
          },
          openPanel: () => {},
          closePanel: () => {},
        },
        utils: { now: () => 0 },
      }
      const payload = {
        type: 'flowtools:html-plugin-call' as const,
        id: 'fixture',
        method: 'ui.toast',
        get payload() {
          return {
            title: 'fixture',
            certified: true,
            granted: true,
            mode: 'development',
          }
        },
      }
      if (!mode.allowed) {
        expect(() => launch.resolveDevelopmentHtmlTarget(command)).toThrow()
        expect(() => bridge.createHtmlPluginBridgeScript(command)).toThrow()
        expect(() =>
          bridge.injectHtmlPluginBridge('<head></head>', command)
        ).toThrow()
        await assertRejected(
          launch.loadDevelopmentHtmlFrame(command, command.main!, { request }),
          { code: 'EXTERNAL_CODE_DISABLED' }
        )
        await assertRejected(
          bridge.handleHtmlPluginBridgeRequest(context, payload),
          { code: 'EXTERNAL_CODE_DISABLED' }
        )
        expect(requests).toBe(0)
        expect(payloadReads).toBe(0)
      } else {
        expect(launch.resolveDevelopmentHtmlTarget(command)).toBe(command.main)
        const html = await launch.loadDevelopmentHtmlFrame(
          command,
          command.main!,
          { request }
        )
        expect(html).toContain('flowtools:html-plugin-call')
        expect(html).toContain('preload.js')
        expect(requests).toBe(1)
        expect(
          await bridge.handleHtmlPluginBridgeRequest(context, payload)
        ).toBe(true)
        expect(payloadReads).toBe(1)
        for (const method of [
          'native.invoke',
          'db.query',
          'fs.readFile',
          'fs.writeFile',
          'opener.openUrl',
          'opener.openPath',
          'opener.revealItemInDir',
        ]) {
          let unsafePayloadReads = 0
          await assertRejected(
            bridge.handleHtmlPluginBridgeRequest(context, {
              type: 'flowtools:html-plugin-call',
              id: 'blocked-native',
              method,
              get payload() {
                unsafePayloadReads++
                return {
                  command: 'unsafe',
                  sql: 'select 1',
                  path: '../secret',
                  url: 'https://fixture.invalid/',
                }
              },
            }),
            'Unsupported HTML development bridge method'
          )
          expect(unsafePayloadReads).toBe(0)
        }
        const failed: typeof fetch = Object.assign(
          async () => new Response('', { status: 500 }),
          { preconnect: () => {} }
        )
        await assertRejected(
          launch.loadDevelopmentHtmlFrame(command, command.main!, {
            request: failed,
          }),
          'HTML development preview could not be loaded'
        )
      }
    } finally {
      const canonical = realpathSync(temporary)
      expect(dirname(canonical)).toBe(realpathSync(temporaryParent))
      expect(canonical).toBe(temporary)
      rmSync(canonical, { recursive: true })
    }
  })
}
