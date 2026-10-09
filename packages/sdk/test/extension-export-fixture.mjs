import assert from 'node:assert/strict'
import NodeModule, * as moduleApi from 'node:module'

import { registerDependencyGuard } from './extension-dependency-guard.mjs'

registerDependencyGuard(moduleApi)

// Async ESM hooks do not see ordinary CommonJS requires. This child-process-only
// guard covers those without changing CommonJS source/named-export semantics.
const commonJsModule =
  /** @type {{ _load: (specifier: string, parent: unknown, isMain?: boolean) => unknown }} */ (
    /** @type {unknown} */ (NodeModule)
  )
const loadCommonJs = commonJsModule._load
commonJsModule._load = function (specifier, parent, isMain) {
  if (/^(react|react-dom|sucrase)(\/|$)/.test(specifier))
    throw new Error(`Unexpected extension dependency: ${specifier}`)
  return loadCommonJs.call(this, specifier, parent, isMain)
}

// Prove both guards reject imports from a different module before checking SDK.
const { importDependency, requireDependency } =
  await import('./extension-dependency-guard.mjs')
for (const specifier of [
  'react',
  'react/jsx-runtime',
  'react-dom',
  'react-dom/client',
  'sucrase',
  'sucrase/dist/index.js',
]) {
  const rejection = { message: `Unexpected extension dependency: ${specifier}` }
  await assert.rejects(importDependency(specifier), rejection)
  assert.throws(() => requireDependency(specifier), rejection)
}

const {
  ExtensionContributionError,
  ExtensionContributionRegistry,
  parseExtensionContributions,
  projectPluginContributions,
  AppearanceError,
  createAppearanceResolver,
  parseAppearanceTheme,
} = await import('@flowtools/sdk/extensions')

assert.equal(typeof projectPluginContributions, 'function')
const registry = new ExtensionContributionRegistry()
const owner = registry.createOwner('node-fixture', '1.0.0')
const parsed = parseExtensionContributions({
  formatVersion: 1,
  contributions: [{ id: 'theme', kind: 'theme', value: { radius: 8 } }],
})
const dispose = registry.replace(owner, parsed)
assert.equal(registry.getSnapshot()[0].key, 'node-fixture:theme')
assert.ok(Object.isFrozen(registry.getSnapshot()[0].value))
dispose()
assert.equal(registry.getSnapshot().length, 0)

// Exercise the compiled theme API in the same React/transpiler-free consumer.
const colors = Object.fromEntries(
  [
    'canvas',
    'text',
    'panel',
    'panelText',
    'overlay',
    'overlayText',
    'mutedText',
    'control',
    'controlText',
    'field',
    'fieldText',
    'fieldPlaceholder',
    'accent',
    'accentText',
    'border',
    'separator',
    'focus',
    'success',
    'successText',
    'warning',
    'warningText',
    'danger',
    'dangerText',
  ].map(key => [key, { space: 'srgb', red: 128, green: 128, blue: 128 }])
)
const baseline = {
  colors,
  radii: { panelRem: 0.625, controlRem: 0.625 },
  borders: { panelPx: 1, fieldPx: 1 },
  typography: { fontFamily: 'system', fontSizePx: 16, lineHeight: 1.5 },
  shadows: { panel: 'soft', overlay: 'raised', field: 'none' },
  motion: 'system',
}
const resolve = createAppearanceResolver({
  formatVersion: 1,
  title: 'Node baseline',
  modes: { light: baseline, dark: baseline },
})
const themeOwner = registry.createOwner('node-fixture', '1.0.0')
const removeTheme = registry.replace(themeOwner, {
  formatVersion: 1,
  contributions: [
    {
      id: 'theme',
      kind: 'theme',
      value: parseAppearanceTheme({
        formatVersion: 1,
        title: 'Node theme',
        common: { radii: { panelRem: 1.25 } },
      }),
    },
  ],
})
const request = {
  selectedKey: 'node-fixture:theme',
  mode: 'system',
  systemMode: 'light',
}
assert.equal(
  resolve({ ...request, contributions: registry.getSnapshot() }).tokens.radii
    .panelRem,
  1.25
)
assert.throws(
  () =>
    parseAppearanceTheme({
      formatVersion: 1,
      title: 'Unsafe',
      common: { colors: { accent: 'url(https://fixture.invalid)' } },
    }),
  AppearanceError
)
removeTheme()
const recovered = resolve({ ...request, contributions: registry.getSnapshot() })
assert.equal(recovered.fallback, 'missing-theme')
assert.equal(recovered.tokens.radii.panelRem, 0.625)

let executions = 0
class ExecutableArray extends Array {
  toJSON() {
    executions++
    return []
  }
}
const sparse = []
sparse.length = 2
sparse.x = 'lost-x'
sparse.y = 'lost-y'
for (const value of [new ExecutableArray(), sparse]) {
  assert.throws(
    () =>
      parseExtensionContributions({
        formatVersion: 1,
        contributions: [{ id: 'invalid', kind: 'theme', value }],
      }),
    error =>
      error instanceof ExtensionContributionError &&
      error.code === 'INVALID_DOCUMENT'
  )
}
assert.equal(executions, 0)
assert.throws(
  () =>
    parseExtensionContributions({
      formatVersion: 1,
      contributions: new ExecutableArray(),
    }),
  ExtensionContributionError
)
assert.equal(executions, 0)

process.stdout.write('Compiled Node extension export fixture passed\n')
