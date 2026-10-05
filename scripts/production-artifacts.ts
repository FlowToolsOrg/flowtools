import { createHash } from 'node:crypto'
import { lstatSync, readdirSync, readFileSync, realpathSync } from 'node:fs'
import { isAbsolute, join, relative, resolve, sep } from 'node:path'

import ts from 'typescript'

export interface ProductionArtifact {
  readonly path: string
  readonly size: number
  readonly sha256: string
  readonly text: string
}

const externalMarkers = [
  '__flowtools_plugin_loaded__',
  '__flowtools_importmap__',
  'https://esm.sh/',
  'flowtools:html-plugin-call',
  'flowtoolsHtmlPluginBridge',
  'development-html-plugin-surface',
  'development-html-plugin-bridge',
  'development-plugin-file-loader',
  '/@fs/',
  'Prototype · disposable Runtime profile',
  'validation-runtime-panel',
] as const
const certificationNames = new Set([
  'certified',
  'productionCertified',
  'isProductionCertified',
  'securityCertified',
])

/** Fixed build tree, byte hashes and portable paths; no import or execution. */
export function readProductionArtifacts(
  directory: string
): ProductionArtifact[] {
  const requested = resolve(directory)
  if (lstatSync(requested).isSymbolicLink())
    throw new Error('Redirected production artifact')
  const root = realpathSync(requested)
  const artifacts: ProductionArtifact[] = []
  const walk = (folder: string): void => {
    for (const name of readdirSync(folder).sort()) {
      const path = join(folder, name)
      const stat = lstatSync(path)
      if (stat.isSymbolicLink())
        throw new Error('Redirected production artifact')
      const canonical = realpathSync(path)
      const local = relative(root, canonical)
      if (isAbsolute(local) || local === '..' || local.startsWith(`..${sep}`))
        throw new Error('Redirected production artifact')
      if (stat.isDirectory()) walk(canonical)
      else if (stat.isFile()) {
        const bytes = readFileSync(canonical)
        artifacts.push({
          path: local.replaceAll('\\', '/'),
          size: bytes.length,
          sha256: createHash('sha256').update(bytes).digest('hex'),
          text: bytes.toString('utf8'),
        })
      } else throw new Error('Unsupported production artifact file')
    }
  }
  walk(root)
  return artifacts.sort((a, b) => a.path.localeCompare(b.path))
}

/** No security certification: only known fingerprints, syntax and canaries. */
export function assertProductionArtifacts(
  artifacts: readonly ProductionArtifact[],
  canaries: readonly string[] = []
): void {
  if (
    !artifacts.some(file => file.path === 'index.html' && file.size > 0) ||
    !artifacts.some(file => /\.[cm]?js$/.test(file.path) && file.size > 0)
  )
    throw new Error('Incomplete production artifacts')
  for (const file of artifacts) {
    if (externalMarkers.some(marker => file.text.includes(marker)))
      throw new Error('Forbidden external execution marker')
    if (
      canaries.some(canary => canary.length > 0 && file.text.includes(canary))
    )
      throw new Error('Production environment canary leaked')
    if (!/\.[cm]?js$/.test(file.path)) continue
    const source = ts.createSourceFile(
      file.path,
      file.text,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.JS
    )
    const inspect = (node: ts.Node): void => {
      if (
        (ts.isIdentifier(node) && certificationNames.has(node.text)) ||
        (ts.isElementAccessExpression(node) &&
          ts.isStringLiteral(node.argumentExpression) &&
          certificationNames.has(node.argumentExpression.text)) ||
        (ts.isPropertyAssignment(node) &&
          ts.isStringLiteral(node.name) &&
          certificationNames.has(node.name.text))
      )
        throw new Error('Unsupported certification switch')
      ts.forEachChild(node, inspect)
    }
    inspect(source)
  }
}

export function artifactDigests(artifacts: readonly ProductionArtifact[]) {
  return artifacts.map(({ path, size, sha256 }) => ({ path, size, sha256 }))
}
