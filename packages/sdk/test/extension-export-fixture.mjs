import assert from 'node:assert/strict'
import NodeModule, { register } from 'node:module'

// module.register is available on the existing Node 20.19 baseline.
register(new URL('./extension-dependency-guard.mjs', import.meta.url))

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
