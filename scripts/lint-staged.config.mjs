import { relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const generated = new Set([
  'packages/runtime-client/src/bindings.ts',
  'packages/runtime-client/src/wire-schema.json',
  'packages/runtime-client/src/wire-fixtures.json',
  'packages/runtime-client/src/wire-validators.cjs',
  'packages/runtime-client/src/wire-validators.d.cts',
  'apps/desktop/src/utils/bindings.ts',
])
/** Exclude immutable signed bytes and generated bindings before command batches.
 * @param {readonly string[]} files
 * @param {boolean} source
 * @returns {string[]}
 */
export function stagedCommands(files, source) {
  const selected = files.filter(file => {
    const path = relative(root, file).replaceAll('\\', '/')
    return (
      !path.startsWith('packages/runtime-core/assets/builtin-media/') &&
      !generated.has(path)
    )
  })
  if (!selected.length) return []
  // lint-staged executes argv directly (shell=false); quotes group spaces only.
  const args = selected
    .map(path => '"' + path.replaceAll('\\', '/') + '"')
    .join(' ')
  return source ? [`oxlint --fix ${args}`, `oxfmt ${args}`] : [`oxfmt ${args}`]
}
export default {
  '*.{js,jsx,ts,tsx,mjs,cjs,mts,cts}': files => stagedCommands(files, true),
  '*.{json,md,css,scss,html,yml,yaml}': files => stagedCommands(files, false),
}
