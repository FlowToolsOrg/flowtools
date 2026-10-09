import { describe, expect, test } from 'bun:test'

interface ExportTarget {
  types: string
  import: string
}

interface PackageManifest {
  name: string
  exports: Record<string, ExportTarget>
}

const expectedSymbols: Record<string, readonly string[]> = {
  './dependencies': [
    'serviceDefinitionSchema',
    'serviceDependencySchema',
    'toolDependencySchema',
    'parseDependencyDeclarations',
  ],
  './data': ['inspectLegacyTodos', 'hydratePluginData'],
  './manifest': [
    'pluginManifestSchema',
    'executeManifestCommand',
    'exportOperationSchema',
  ],
  './manifest/package': ['verifyManifestPackage', 'manifestPackageDigest'],
  './development': ['DevelopmentPluginFileLoader', 'setupImportMap'],
  './compat/catalog': ['htmlPluginCatalogSchema', 'portablePluginPathSchema'],
  '.': ['definePlugin', 'result'],
  './definePlugin': ['definePlugin'],
  './execution': [
    'executePlugin',
    'createExecutionFailure',
    'summarizeExecutionInput',
  ],
  './result': ['result'],
  './utils': ['extractMeta', 'pickCapability'],
  './utils/capability': ['pickCapability'],
  './hooks': ['useEnv'],
  './runtime': ['FlowToolRuntimeContext', 'FlowToolRuntimeProvider'],
  './types': [
    'pluginMaturitySchema',
    'resolvePluginMaturity',
    'compatibilityEvidenceStatusSchema',
  ],
}

describe('package exports', () => {
  test('execution/catalog subpaths are usable without React or source transpilers', async () => {
    for (const path of [
      'index.js',
      'execution.js',
      'dependencies.js',
      'data.js',
      'compat/catalog.js',
      'manifest.js',
      'manifest/package.js',
    ]) {
      const source = await Bun.file(
        new URL(`../dist/${path}`, import.meta.url)
      ).text()
      expect(source).not.toMatch(/(?:from|import)\s*['"]sucrase/)
      if (path !== 'index.js') {
        expect(source).not.toMatch(/(?:from|import)\s*['"]react/)
      }
      expect(source).not.toContain('__flowtools_plugin_loaded__')
    }
  })
  test('points every public subpath at consumable JS and declaration files', async () => {
    const packageRoot = new URL('../', import.meta.url)
    const manifest = (await Bun.file(
      new URL('package.json', packageRoot)
    ).json()) as PackageManifest

    expect(Object.keys(manifest.exports).sort()).toEqual(
      Object.keys(expectedSymbols).sort()
    )

    for (const [subpath, target] of Object.entries(manifest.exports)) {
      expect(target.import.startsWith('./dist/')).toBe(true)
      expect(target.types.startsWith('./dist/')).toBe(true)

      for (const filePath of [target.import, target.types]) {
        const output = Bun.file(
          new URL(filePath.replace(/^\.\//, ''), packageRoot)
        )
        expect(await output.exists()).toBe(true)
        expect(output.size).toBeGreaterThan(0)
      }

      const specifier =
        subpath === '.' ? manifest.name : `${manifest.name}/${subpath.slice(2)}`
      const loaded = (await import(specifier)) as Record<string, unknown>

      for (const symbol of expectedSymbols[subpath] ?? []) {
        expect(loaded).toHaveProperty(symbol)
      }
    }
  })
})
