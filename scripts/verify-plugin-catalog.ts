import { createHash } from 'node:crypto'
import { readFileSync, realpathSync } from 'node:fs'
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

import { htmlPluginCatalogSchema } from '../packages/sdk/src/compat/catalog'

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

/** Does not load external checkouts, URLs, scripts, or native capabilities. */
export function verifyCatalogFiles(
  desktop: unknown,
  documented: unknown
): void {
  const catalog = htmlPluginCatalogSchema.parse(desktop)
  htmlPluginCatalogSchema.parse(documented)
  if (JSON.stringify(desktop) !== JSON.stringify(documented))
    throw new Error('Catalog copies differ')
  for (const fixture of catalog.fixtures) {
    const path = realpathSync(join(repositoryRoot, fixture.path))
    const rel = relative(realpathSync(repositoryRoot), path)
    if (isAbsolute(rel) || rel === '..' || rel.startsWith(`..${sep}`))
      throw new Error('Fixture path escapes repository')
    const contents = readFileSync(path)
    if (
      createHash('sha256')
        .update(contents.toString().replaceAll('\r\n', '\n'))
        .digest('hex') !== fixture.sha256
    )
      throw new Error('Fixture digest mismatch')
    const receipt = JSON.parse(contents.toString()) as {
      id?: unknown
      scope?: unknown
    }
    if (
      receipt.id !== fixture.id ||
      receipt.scope !== 'local-file-existence-and-sha256-only'
    )
      throw new Error('Invalid fixture scope')
  }
}

if (import.meta.main) {
  const read = (path: string): unknown =>
    JSON.parse(readFileSync(join(repositoryRoot, path), 'utf8'))
  verifyCatalogFiles(
    read('apps/desktop/src/data/html-plugin-catalog.json'),
    read('docs/html-plugin-catalog.json')
  )
  process.stdout.write(
    'Portable catalog contracts passed; no execution or security certification.\n'
  )
}
