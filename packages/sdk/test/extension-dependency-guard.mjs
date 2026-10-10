import { createRequire } from 'node:module'

/**
 * @param {Pick<typeof import('node:module'), 'register'> & Partial<Pick<typeof import('node:module'), 'registerHooks'>>} moduleApi
 */
export function registerDependencyGuard(moduleApi) {
  // Node 22.15+ supports synchronous hooks; register is deprecated in Node 26.
  if (typeof moduleApi.registerHooks === 'function') {
    moduleApi.registerHooks({ resolve })
    return
  }
  // Keep the Node 20.19 baseline's real asynchronous ESM loader.
  moduleApi.register(
    new URL('./extension-dependency-guard.mjs', import.meta.url)
  )
}

// Keep the actual ESM loader in a separate module for Node 20's async hooks.
export function resolve(specifier, context, nextResolve) {
  if (/^(react|react-dom|sucrase)(\/|$)/.test(specifier))
    throw new Error(`Unexpected extension dependency: ${specifier}`)
  return nextResolve(specifier, context)
}

// Controlled transitive probes exercise imports originating in this module.
export const importDependency = specifier => import(specifier)
const require = createRequire(import.meta.url)
export const requireDependency = specifier => require(specifier)
