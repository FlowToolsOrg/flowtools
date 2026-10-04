import { describe, expect, test } from 'bun:test'
import { mkdirSync, mkdtempSync, realpathSync, rmSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { PluginFileLoader, PluginLoader, PluginRegistry } from '@flowtools/sdk'

async function expectDenied(action: () => unknown): Promise<void> {
  const error: unknown = await Promise.resolve()
    .then(action)
    .catch((failure: unknown) => failure)
  expect(error).toBeInstanceOf(Error)
  expect(error).toHaveProperty('code', 'EXTERNAL_CODE_DISABLED')
}

describe('public SDK external-code default denial', () => {
  test.each(['js', 'mjs', 'jsx', 'ts', 'tsx'])(
    'rejects %s before reading or importing source',
    async extension => {
      const registry = new PluginRegistry()
      const service = new PluginFileLoader(registry, new PluginLoader(registry))
      let reads = 0
      const file = new File(
        ['throw new Error("must not run")'],
        `fixture.${extension}`
      )
      file.text = () => {
        reads += 1
        return Promise.resolve('')
      }
      await expectDenied(() => service.loadFromFile(file))
      expect(reads).toBe(0)
      expect(registry.getAll()).toEqual([])
      expect(service.getExternalPluginIds()).toEqual([])
    }
  )

  test('rejects batch, resolved object and unload entry points without lifecycle effects', async () => {
    const registry = new PluginRegistry()
    const service = new PluginFileLoader(registry, new PluginLoader(registry))
    let effects = 0
    const plugin = {
      type: 'tool' as const,
      meta: {
        id: 'untrusted-fixture',
        name: 'Untrusted',
        version: '1.0.0',
        certified: true,
      },
      run: () => {
        effects += 1
      },
      lifecycle: {
        onActivate: () => {
          effects += 1
        },
      },
    }
    await expectDenied(() => service.loadFromFiles([]))
    await expectDenied(() => service.registerExternalPlugin(plugin))
    await expectDenied(() => service.unloadExternalPlugin('untrusted-fixture'))
    expect(effects).toBe(0)
    expect(registry.getAll()).toEqual([])
    expect(service.isExternal('untrusted-fixture')).toBe(false)
  })

  test('ordinary SDK exports do not expose source transpilation or import-map injection', async () => {
    const sdk = await import('@flowtools/sdk')
    for (const name of [
      'transpile',
      'needsTranspilation',
      'setupImportMap',
      'createSdkBridgeBlobUrl',
    ]) {
      expect(sdk).not.toHaveProperty(name)
    }
  })
})

describe('unsafe development subpath build policy', () => {
  test.each([
    { dev: false, flag: undefined, allowed: false },
    { dev: false, flag: '1', allowed: false },
    { dev: true, flag: undefined, allowed: false },
    { dev: true, flag: '0', allowed: false },
    { dev: true, flag: true, allowed: false },
    { dev: true, flag: '1', allowed: true },
  ])(
    'DEV=$dev opt-in=$flag allows=$allowed',
    async ({ dev, flag, allowed }) => {
      const buildRoot = fileURLToPath(
        new URL('../node_modules/.tmp', import.meta.url)
      )
      mkdirSync(buildRoot, { recursive: true })
      const outputDirectory = mkdtempSync(join(buildRoot, 'source-gate-'))
      try {
        const result = await Bun.build({
          entrypoints: [
            fileURLToPath(new URL('../src/development.ts', import.meta.url)),
          ],
          target: 'bun',
          format: 'esm',
          external: ['sucrase', 'zod'],
          outdir: outputDirectory,
          define: {
            'import.meta.env': JSON.stringify({
              DEV: dev,
              VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW: flag,
            }),
          },
        })
        expect(result.success).toBe(true)
        const output = result.outputs[0]
        if (!output)
          throw new Error('Development policy fixture produced no module')
        const loaded = (await import(
          pathToFileURL(output.path).href
        )) as typeof import('@flowtools/sdk/development')
        const registry = new PluginRegistry()
        if (allowed) {
          const service = new loaded.DevelopmentPluginFileLoader(
            registry,
            new PluginLoader(registry)
          )
          expect(service.getExternalPluginIds()).toEqual([])
          expect(() => loaded.setupImportMap({})).not.toThrow()
          const entry = await service.registerExternalPlugin({
            type: 'tool',
            meta: {
              id: 'unsigned-preview-fixture',
              name: 'Unsigned preview fixture',
              version: '1.0.0',
              maturity: 'production',
            },
            run: () => undefined,
          })
          expect(entry.manifest.maturity).toBe('prototype')
          expect(entry.state).toBe('enabled')
          await service.unloadExternalPlugin(entry.id)
        } else {
          expect(
            () =>
              new loaded.DevelopmentPluginFileLoader(
                registry,
                new PluginLoader(registry)
              )
          ).toThrow('External code execution is disabled')
          expect(() => loaded.setupImportMap({})).toThrow(
            'External code execution is disabled'
          )
          // TS private is erased. Skipping the constructor must not bypass
          // the guards on the implementation's effectful internal methods.
          const prototype = loaded.DevelopmentPluginFileLoader.prototype
          const instance: unknown = Object.create(prototype)
          for (const name of [
            'validateFile',
            'readFile',
            'importFromCode',
            'validateModule',
            'registerAndEnable',
          ]) {
            const method: unknown = Reflect.get(prototype, name)
            expect(typeof method).toBe('function')
            await expectDenied(
              () =>
                Reflect.apply(
                  method as (...args: unknown[]) => unknown,
                  instance,
                  [{}]
                ) as unknown
            )
          }
        }
        expect(registry.getAll()).toEqual([])
      } finally {
        // Only remove this test's generated, canonicalized, immediate child.
        expect(dirname(realpathSync(outputDirectory))).toBe(
          realpathSync(buildRoot)
        )
        rmSync(outputDirectory, { recursive: true })
      }
    },
    30_000
  )

  test('published development module refuses without a host build environment', async () => {
    const loaded = await import('@flowtools/sdk/development')
    const registry = new PluginRegistry()
    expect(
      () =>
        new loaded.DevelopmentPluginFileLoader(
          registry,
          new PluginLoader(registry)
        )
    ).toThrow('External code execution is disabled')
    expect(() => loaded.setupImportMap({})).toThrow(
      'External code execution is disabled'
    )
  })
})
