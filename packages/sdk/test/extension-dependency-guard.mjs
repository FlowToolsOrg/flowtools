import { createRequire } from 'node:module'

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
